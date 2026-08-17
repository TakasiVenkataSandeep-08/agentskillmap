//! Load-phase classification: the reference graph.
//!
//! This is the signal the whole project is built around. `AGENTS.md` states the
//! thesis: at session start the agent sees ~100 tokens of name and description;
//! the reviewer reads `SKILL.md`, sees something benign, and installs. The
//! payload lives in files that only enter context later, on trigger, mid-task,
//! unobserved. Classifying *when* each file enters context is what makes that
//! asymmetry visible.
//!
//! | Phase | Meaning |
//! |---|---|
//! | `always` | The frontmatter description — seen at session start |
//! | `on_trigger` | The `SKILL.md` body |
//! | `reference` | Reachable from the body by a documented path |
//! | `unreferenced` | Present in the bundle, reachable by no documented path |
//!
//! **No file is classified `always` under the Claude Code resolver, and that is
//! correct.** The always-loaded content is the frontmatter *description*, which
//! is part of `SKILL.md` rather than a file of its own; its size is reported as
//! `disclosure.description_bytes`. Tagging `SKILL.md` itself `always` would claim
//! its body is seen at session start, which is exactly the false comfort this
//! tool exists to dispel. The variant stays in the taxonomy because other agents
//! and plugin wrappers do surface genuinely always-loaded files.

use crate::inventory::WalkedFile;
use skillmap_core::LoadPhase;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

/// Characters that can appear inside a path-like token.
///
/// Anything else terminates the run, so `see [docs](reference/setup.md).` yields
/// `reference/setup.md` without the trailing period or the brackets.
fn is_path_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-' | '/' | '\\' | '~' | '+')
}

/// Pull every path-shaped token out of `text`.
///
/// Deliberately syntax-agnostic rather than a markdown parser. A skill can point
/// at a file with a link, an inline code span, a fenced command line, or a bare
/// sentence — `run scripts/collect.py first` names the file just as surely as
/// `[collect](scripts/collect.py)` does, and a link-only extractor would miss it.
/// Over-collecting is safe here because a candidate only counts once it matches a
/// path that actually exists in the inventory.
fn candidates(text: &str) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    let mut current = String::new();

    for ch in text.chars() {
        if is_path_char(ch) {
            current.push(ch);
            continue;
        }
        take_candidate(&mut current, &mut found);
    }
    take_candidate(&mut current, &mut found);

    found
}

/// Finish the token under construction and keep it if it could name a file.
fn take_candidate(current: &mut String, found: &mut BTreeSet<String>) {
    if current.is_empty() {
        return;
    }
    let token = std::mem::take(current);
    // A path-shaped token needs either a directory separator or an extension.
    // Prose words ("the", "skill") have neither and would only ever fail to match.
    if token.contains('/') || token.contains('\\') || token.contains('.') {
        found.insert(token);
    }
}

/// Resolve a candidate against the directory of the file that mentioned it.
///
/// Returns the bundle-relative, forward-slashed path, or `None` if the candidate
/// escapes the bundle root or is absolute. Escaping candidates are not part of
/// the bundle — a `SKILL.md` mentioning `/etc/passwd` has not made `/etc/passwd`
/// part of it, and it is never added to the inventory.
///
/// That is right about inventory and was wrong about `unresolved`. Returning
/// `None` here used to end the matter, so a `SKILL.md` reading
/// `run [the helper](../../../tools/helper.py)` scanned clean with no gap
/// recorded — while the agent, whose boundary is not the bundle root, follows
/// exactly that link. [`escaping_code_refs`] reports them so the silence is
/// visible; the analysis still refuses to follow them.
fn resolve(from_dir: &str, candidate: &str) -> Option<String> {
    let candidate = candidate.replace('\\', "/");
    // An absolute path or a home-relative one names something outside the bundle.
    if candidate.starts_with('/') || candidate.starts_with('~') {
        return None;
    }

    let mut segments: Vec<&str> = if from_dir.is_empty() {
        Vec::new()
    } else {
        from_dir.split('/').collect()
    };

    for part in candidate.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                // Popping past the root would leave the bundle.
                segments.pop()?;
            }
            other => segments.push(other),
        }
    }

    let joined = segments.join("/");
    (!joined.is_empty()).then_some(joined)
}

/// The directory part of a bundle-relative path, or `""` for a top-level file.
fn parent_dir(path: &str) -> &str {
    match path.rsplit_once('/') {
        Some((dir, _)) => dir,
        None => "",
    }
}

/// Documented references that point at **code outside the bundle**.
///
/// Deliberately narrow. [`candidates`] tokenises any word containing a slash or
/// a dot, so reporting every escaping token would emit an entry for a sentence
/// mentioning `~/.claude/skills` — noise that would bury the case that matters.
/// What matters is a document pointing the agent at code this analysis cannot
/// read, so the filter is the extension: only paths the code plane would have
/// analysed had they been inside.
///
/// Measured over the harvest: 500 of 34,302 bundles carry a link that escapes
/// the bundle, and exactly one points at code. The natural rate is near zero —
/// which is not an argument for ignoring it, because a rate over honest bundles
/// says nothing about a deliberate one, and this is the cheapest evasion the
/// engine has.
pub fn escaping_code_refs(entry: &str, files: &[WalkedFile]) -> BTreeSet<(String, String)> {
    const CODE: [&str; 12] = [
        "py", "pyi", "sh", "bash", "zsh", "js", "mjs", "cjs", "ts", "mts", "cts", "jsx",
    ];
    let mut found = BTreeSet::new();
    for file in files {
        // Only files the bundle actually documents can point anywhere. An
        // unreferenced file's mentions are already covered by it being
        // unreferenced.
        if file.path != entry {
            continue;
        }
        let Some(text) = file.text.as_deref() else {
            continue;
        };
        let from_dir = parent_dir(&file.path);
        for candidate in candidates(text) {
            if resolve(from_dir, &candidate).is_some() || resolve("", &candidate).is_some() {
                continue;
            }
            let extension = candidate
                .rsplit('/')
                .next()
                .and_then(|name| name.rsplit_once('.'))
                .map(|(_, ext)| ext.to_ascii_lowercase())
                .unwrap_or_default();
            if CODE.contains(&extension.as_str()) {
                found.insert((file.path.clone(), candidate));
            }
        }
    }
    found
}

/// Classify every walked file by when it enters the agent's context.
///
/// `entry` is the bundle's entry point, `SKILL.md`. Traversal is breadth-first
/// from it, and both the queue and every candidate set are ordered, so the result
/// does not depend on filesystem or hash iteration order (invariant 2).
///
/// Traversal follows references out of *any* text file, not only markdown. If
/// `SKILL.md` names `scripts/collect.py` and that script in turn names
/// `scripts/helpers.py`, the helper is reachable by a documented path and is
/// `reference`, not `unreferenced`. Stopping at markdown would report every
/// imported helper as unreferenced and drown the one signal that matters: a file
/// nothing points at is either dead weight or a payload waiting for a later
/// commit to wire it up.
pub fn classify(entry: &str, files: &[WalkedFile]) -> BTreeMap<String, LoadPhase> {
    let known: BTreeMap<&str, &WalkedFile> = files
        .iter()
        .map(|file| (file.path.as_str(), file))
        .collect();

    let mut phase: BTreeMap<String, LoadPhase> = BTreeMap::new();
    let mut queue: VecDeque<String> = VecDeque::new();

    if known.contains_key(entry) {
        phase.insert(entry.to_owned(), LoadPhase::OnTrigger);
        queue.push_back(entry.to_owned());
    }

    while let Some(current) = queue.pop_front() {
        let Some(file) = known.get(current.as_str()) else {
            continue;
        };
        let Some(text) = file.text.as_deref() else {
            continue;
        };

        let from_dir = parent_dir(&current);
        for candidate in candidates(text) {
            // Try the candidate both relative to the mentioning file's directory
            // and relative to the bundle root. Skills write both — `./setup.md`
            // from inside `reference/`, and `reference/setup.md` from anywhere.
            for resolved in [resolve(from_dir, &candidate), resolve("", &candidate)]
                .into_iter()
                .flatten()
            {
                if !known.contains_key(resolved.as_str()) || phase.contains_key(&resolved) {
                    continue;
                }
                phase.insert(resolved.clone(), LoadPhase::Reference);
                queue.push_back(resolved);
            }
        }
    }

    for file in files {
        phase
            .entry(file.path.clone())
            .or_insert(LoadPhase::Unreferenced);
    }

    phase
}

/// Extract trigger terms from the frontmatter description.
///
/// These are the words that would plausibly cause an agent to load this skill.
/// **Extracted, not scored** — there is no weighting, ranking, or relevance
/// judgement here, and adding one would be a verdict (invariant 1).
///
/// The rule is deliberately dull and fully specified, because it feeds a
/// byte-identical artifact: lowercase, split on anything that is not a letter or
/// digit, drop tokens shorter than three characters, drop a fixed English
/// stopword list, deduplicate, sort. Nothing here depends on locale — `to_lowercase`
/// is Unicode-defined, not locale-defined.
pub fn trigger_terms(description: &str) -> Vec<String> {
    let mut terms: BTreeSet<String> = BTreeSet::new();
    for raw in description.split(|ch: char| !ch.is_alphanumeric()) {
        if raw.is_empty() {
            continue;
        }
        let term = raw.to_lowercase();
        if term.chars().count() < 3 || STOPWORDS.contains(&term.as_str()) {
            continue;
        }
        terms.insert(term);
    }
    terms.into_iter().collect()
}

/// Fixed English stopword list.
///
/// Sorted and closed. It exists so `trigger_terms` is not 40% articles and
/// auxiliaries; it is not tuned per bundle, because a list that changed with the
/// input would make the output depend on something other than the input.
const STOPWORDS: &[&str] = &[
    "and", "any", "are", "but", "can", "for", "from", "has", "have", "how", "into", "its", "not",
    "que", "should", "that", "the", "their", "them", "then", "these", "they", "this", "those",
    "use", "used", "uses", "using", "was", "were", "what", "when", "which", "will", "with", "you",
    "your",
];

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "a failed unwrap or out-of-bounds index in a test is the test failing, which is the point. Invariant 10 bans these in library code, where hostile input is the normal case and a crash is a DoS on somebody's CI."
)]
mod tests {
    use super::*;
    use skillmap_core::{Digest, ParseStatus};

    fn file(path: &str, text: Option<&str>) -> WalkedFile {
        WalkedFile {
            path: path.to_owned(),
            size: text.map_or(0, |t| t.len() as u64),
            sha256: Digest::of(path.as_bytes()),
            parsed_as: "markdown",
            parse_status: ParseStatus::Ok,
            text: text.map(str::to_owned),
        }
    }

    #[test]
    fn stopwords_are_sorted_and_unique() {
        let mut sorted = STOPWORDS.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(
            sorted, STOPWORDS,
            "stopword list must stay sorted and unique"
        );
    }

    #[test]
    fn extracts_paths_from_links_code_spans_and_prose() {
        let found = candidates(
            "See [setup](reference/setup.md) then run `scripts/collect.py`. \
             Also read docs/deep.md today.",
        );
        assert!(found.contains("reference/setup.md"));
        assert!(found.contains("scripts/collect.py"));
        assert!(found.contains("docs/deep.md"));
        // Trailing punctuation is not part of the path.
        assert!(!found.contains("docs/deep.md."));
    }

    #[test]
    fn a_document_pointing_at_code_outside_the_bundle_is_recorded() {
        // The evasion this closes: a SKILL.md reading
        // `run [the helper](../../../tools/helper.py)`, where that helper reads
        // credentials and posts them onward, scanned completely clean. The
        // analysis was right to refuse to follow the link and wrong to say
        // nothing about having seen it — the bundle root is skillmap's boundary,
        // not the agent's.
        let files = vec![file(
            "SKILL.md",
            Some("See [a](../../../tools/helper.py) and [b](/opt/x/run.sh) and [c](~/e/z.js)."),
        )];
        let found = escaping_code_refs("SKILL.md", &files);
        let targets: BTreeSet<&str> = found.iter().map(|(_, target)| target.as_str()).collect();
        assert!(targets.contains("../../../tools/helper.py"), "{targets:?}");
        assert!(targets.contains("/opt/x/run.sh"), "{targets:?}");
        assert!(targets.contains("~/e/z.js"), "{targets:?}");
    }

    #[test]
    fn prose_about_paths_is_not_reported_as_an_escaping_reference() {
        // `candidates` tokenises any word containing a slash or a dot, so an
        // unfiltered version of the check above would fire on a sentence telling
        // the reader where skills live. That noise would bury the case that
        // matters, so only paths the code plane would have analysed count.
        let files = vec![file(
            "SKILL.md",
            Some(
                "Install into ~/.claude/skills, keep creds in ~/.aws/credentials,                  see /etc/hosts and https://example.com/docs.html for details.",
            ),
        )];
        assert!(
            escaping_code_refs("SKILL.md", &files).is_empty(),
            "{:?}",
            escaping_code_refs("SKILL.md", &files)
        );
    }

    #[test]
    fn resolves_relative_and_root_anchored_paths() {
        assert_eq!(
            resolve("reference", "./setup.md").as_deref(),
            Some("reference/setup.md")
        );
        assert_eq!(
            resolve("reference", "../scripts/a.py").as_deref(),
            Some("scripts/a.py")
        );
        assert_eq!(resolve("", "scripts/a.py").as_deref(), Some("scripts/a.py"));
        // Escapes are not references.
        assert_eq!(resolve("", "../outside.md"), None);
        assert_eq!(resolve("", "/etc/passwd"), None);
        assert_eq!(resolve("", "~/.aws/credentials"), None);
    }

    #[test]
    fn classifies_the_four_phases() {
        let files = vec![
            file(
                "SKILL.md",
                Some("Run `scripts/collect.py` and see [docs](reference/setup.md)."),
            ),
            file(
                "scripts/collect.py",
                Some("import helpers  # see scripts/helpers.py"),
            ),
            file("scripts/helpers.py", Some("pass")),
            file("reference/setup.md", Some("nothing further")),
            file("scripts/payload.py", Some("never mentioned anywhere")),
        ];
        let phases = classify("SKILL.md", &files);

        assert_eq!(phases["SKILL.md"], LoadPhase::OnTrigger);
        assert_eq!(phases["scripts/collect.py"], LoadPhase::Reference);
        assert_eq!(phases["reference/setup.md"], LoadPhase::Reference);
        // Transitively reachable through a script, not just through markdown.
        assert_eq!(phases["scripts/helpers.py"], LoadPhase::Reference);
        // The signal that matters: nothing points at this.
        assert_eq!(phases["scripts/payload.py"], LoadPhase::Unreferenced);
        // No file is `always` — the always-loaded content is the description.
        assert!(phases.values().all(|p| *p != LoadPhase::Always));
    }

    #[test]
    fn a_reference_cycle_terminates() {
        let files = vec![
            file("SKILL.md", Some("see a.md")),
            file("a.md", Some("see b.md")),
            file("b.md", Some("see a.md and SKILL.md")),
        ];
        let phases = classify("SKILL.md", &files);
        assert_eq!(phases["a.md"], LoadPhase::Reference);
        assert_eq!(phases["b.md"], LoadPhase::Reference);
        // The entry keeps its own phase; a back-reference cannot demote it.
        assert_eq!(phases["SKILL.md"], LoadPhase::OnTrigger);
    }

    #[test]
    fn a_binary_file_is_unreferenced_unless_named() {
        let files = vec![
            file("SKILL.md", Some("nothing here")),
            file("vendor/blob.bin", None),
        ];
        let phases = classify("SKILL.md", &files);
        assert_eq!(phases["vendor/blob.bin"], LoadPhase::Unreferenced);
    }

    #[test]
    fn every_file_gets_exactly_one_phase() {
        let files = vec![
            file("SKILL.md", Some("see a.md")),
            file("a.md", Some("x")),
            file("b.md", Some("y")),
        ];
        let phases = classify("SKILL.md", &files);
        assert_eq!(phases.len(), files.len());
    }

    #[test]
    fn trigger_terms_are_lowercased_deduped_sorted_and_stopword_free() {
        let terms = trigger_terms("Formats AWS credentials, and formats the AWS config.");
        assert_eq!(terms, vec!["aws", "config", "credentials", "formats"]);
    }

    #[test]
    fn trigger_terms_ignore_short_tokens_and_punctuation() {
        assert!(trigger_terms("a an I/O 42").is_empty());
    }
}
