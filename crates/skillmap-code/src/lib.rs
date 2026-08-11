#![warn(missing_docs)]

//! The code plane — tier `proven`.
//!
//! Runs compiled rules over parsed source, maps captures to findings with full
//! provenance, and establishes reachability. Everything it emits is a fact about
//! the bundle that a reviewer can check by opening the named file at the named
//! byte offset.
//!
//! Three properties this crate is responsible for:
//!
//! - **Invariant 4, provenance.** Every capability carries `{file, byte span,
//!   line, rule_id, snippet_sha256}`. The snippet hash is what lets a regression
//!   test notice a span drifting onto different bytes.
//! - **Invariant 3, no silence.** A file in a language with no grammar produces
//!   `unsupported_language`. A sink whose target is computed produces
//!   `computed_target` rather than being skipped.
//! - **Invariant 7, no language-specific code.** There are no node types and no
//!   sink names in this crate. It maps roles the rules declare.
//!
//! It does not emit `instructions` — those are tier `pattern` and belong to the
//! instruction plane (T5). Rules of that tier are loaded but not executed here,
//! which is invariant 5 enforced by construction rather than by filtering.

pub mod reach;

use skillmap_core::{
    Capability, CapabilityTerm, Detail, Digest, EvidenceStrict, LoadPhase, NonEmpty, Reachability,
    Unresolved, UnresolvedReason,
};
use skillmap_rules::{Claim, CompiledRule, LoadedLanguage, Role, RuleSet};
use std::collections::BTreeMap;
use std::num::NonZeroU64;
use streaming_iterator::StreamingIterator as _;
use tree_sitter::{Node, Parser, QueryCursor};

/// A file to analyze.
#[derive(Debug, Clone, Copy)]
pub struct SourceFile<'a> {
    /// Forward-slash path relative to the bundle root, as it appears in evidence.
    pub path: &'a str,
    /// The file's text, already LF-normalized by the parser.
    pub text: &'a str,
    /// Whether this file runs when the bundle is used.
    ///
    /// Supplied by the caller from the load phase: a file the body documents a
    /// path to is imported or invoked; an unreferenced one is not. Reachability
    /// never claims `observed` for a file that does not run.
    pub entered: bool,
}

/// Whether a file runs when the bundle is used.
///
/// The policy behind [`SourceFile::entered`], in one place so the CLI, the eval
/// harness and the corpus tool cannot each invent their own and disagree about
/// what `observed` means.
///
/// A file runs if **either**:
///
/// - the body documents a path to it — `on_trigger` is `SKILL.md` itself and
///   `reference` is everything reachable from it, which for an imported module
///   means its top level executes; or
/// - its basename is one the language's ecosystem treats as an entry point,
///   listed in `rules/languages.toml`. A `main.py` nobody links to still runs
///   when somebody runs it.
///
/// An `unreferenced` file with an unremarkable name does not run on its own, and
/// nothing in it is ever reported as `observed`.
#[must_use]
pub fn is_entered(path: &str, load_phase: LoadPhase, rules: &RuleSet) -> bool {
    if matches!(load_phase, LoadPhase::OnTrigger | LoadPhase::Reference) {
        return true;
    }
    let basename = path.rsplit('/').next().unwrap_or(path);
    extension_of(path)
        .and_then(|extension| rules.language_for_extension(&extension))
        .is_some_and(|language| {
            language
                .config
                .entry_basenames
                .iter()
                .any(|candidate| candidate == basename)
        })
}

/// What the code plane established.
#[derive(Debug, Default)]
pub struct Analysis {
    /// Tier `proven` findings, one per capability term.
    pub capabilities: Vec<Capability>,
    /// Everything the analysis could not cover.
    pub unresolved: Vec<Unresolved>,
}

/// Run every `proven` rule over every file.
///
/// Files whose extension maps to no grammar are reported, not skipped: a scanner
/// that reports nothing because it understood nothing has to look different from
/// one that reports nothing because there was nothing there.
#[must_use]
pub fn analyze(files: &[SourceFile<'_>], rules: &RuleSet) -> Analysis {
    // Keyed by the capability's *wire name*, not by the enum: `CapabilityTerm`
    // deliberately has no `Ord`, because deriving one would make the order of
    // findings depend on the declaration order of its variants — the same hazard
    // the manifest canonicalizer avoids. Merging by term is what makes
    // reachability a per-capability property rather than a per-evidence one.
    let mut findings: BTreeMap<&'static str, Finding> = BTreeMap::new();
    let mut unresolved: Vec<Unresolved> = Vec::new();

    for file in files {
        let Some(language) =
            extension_of(file.path).and_then(|extension| rules.language_for_extension(&extension))
        else {
            unresolved.push(Unresolved {
                reason: UnresolvedReason::UnsupportedLanguage,
                file: file.path.to_owned(),
                start_byte: None,
                end_byte: None,
                start_line: None,
                note: Some(
                    "no grammar is registered for this file type; nothing was \
                     analyzed in it"
                        .to_owned(),
                ),
            });
            continue;
        };

        analyze_file(file, language, rules, &mut findings, &mut unresolved);
    }

    let mut capabilities: Vec<Capability> = findings
        .into_values()
        .filter_map(Finding::into_capability)
        .collect();
    capabilities.sort_by(|a, b| a.capability.as_str().cmp(b.capability.as_str()));

    Analysis {
        capabilities,
        unresolved,
    }
}

/// Accumulated evidence for one capability term.
struct Finding {
    term: CapabilityTerm,
    evidence: Vec<EvidenceStrict>,
    paths: Vec<String>,
    hosts: Vec<String>,
    reachability: Option<Reachability>,
}

impl Finding {
    /// Merge one more sighting.
    ///
    /// Reachability precedence, and the reasoning for it:
    ///
    /// - Any `observed` wins. A path was established somewhere; the capability
    ///   is reachable, whatever else is true.
    /// - Otherwise any `unresolved` wins over `present`. `present` asserts the
    ///   analysis looked and found no caller; `unresolved` says it could not see.
    ///   Reporting `present` while some evidence was unanalyzable would claim
    ///   more than was established (invariant 4).
    /// - Otherwise `present`.
    fn merge(&mut self, reachability: Reachability) {
        self.reachability = Some(match (self.reachability, reachability) {
            (Some(Reachability::Observed), _) | (_, Reachability::Observed) => {
                Reachability::Observed
            }
            (Some(Reachability::Unresolved), _) | (_, Reachability::Unresolved) => {
                Reachability::Unresolved
            }
            _ => Reachability::Present,
        });
    }

    /// A fresh accumulator for one capability term.
    fn new(term: CapabilityTerm) -> Self {
        Self {
            term,
            evidence: Vec::new(),
            paths: Vec::new(),
            hosts: Vec::new(),
            reachability: None,
        }
    }

    /// Turn into a manifest capability, or nothing if no evidence survived.
    fn into_capability(mut self) -> Option<Capability> {
        self.paths.sort();
        self.paths.dedup();
        self.hosts.sort();
        self.hosts.dedup();

        let detail = (!self.paths.is_empty() || !self.hosts.is_empty()).then(|| Detail {
            paths: (!self.paths.is_empty()).then_some(self.paths),
            hosts: (!self.hosts.is_empty()).then_some(self.hosts),
        });

        Some(Capability {
            capability: self.term,
            reachability: self.reachability.unwrap_or(Reachability::Present),
            detail,
            // A capability with no evidence is unrepresentable, so a finding that
            // somehow accumulated none is dropped rather than forced.
            evidence: NonEmpty::new(self.evidence)?,
        })
    }
}

/// Run the rules for one file.
fn analyze_file(
    file: &SourceFile<'_>,
    language: &LoadedLanguage,
    rules: &RuleSet,
    findings: &mut BTreeMap<&'static str, Finding>,
    unresolved: &mut Vec<Unresolved>,
) {
    let mut parser = Parser::new();
    if parser.set_language(&language.grammar).is_err() {
        unresolved.push(Unresolved {
            reason: UnresolvedReason::ParseError,
            file: file.path.to_owned(),
            start_byte: None,
            end_byte: None,
            start_line: None,
            note: Some(format!(
                "grammar for `{}` could not be loaded",
                language.name
            )),
        });
        return;
    }

    let Some(tree) = parser.parse(file.text, None) else {
        unresolved.push(Unresolved {
            reason: UnresolvedReason::ParseError,
            file: file.path.to_owned(),
            start_byte: None,
            end_byte: None,
            start_line: None,
            note: Some("the grammar could not produce a tree".to_owned()),
        });
        return;
    };

    let reachability = reach::analyze(language, &tree, file.text, file.entered);
    let bytes = file.text.as_bytes();

    for rule in rules.rules_for(&language.name) {
        // Tier `pattern` rules belong to the instruction plane. Executing one
        // here would put a lexical finding in `capabilities`, which is exactly
        // the tier blending invariant 5 forbids.
        let Claim::Capability(term) = rule.claim else {
            continue;
        };

        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(&rule.query, tree.root_node(), bytes);
        while let Some(matched) = matches.next() {
            let mut site: Option<Node<'_>> = None;
            let mut paths: Vec<String> = Vec::new();
            let mut hosts: Vec<String> = Vec::new();
            let mut dynamic = false;

            for capture in matched.captures {
                let index = Some(capture.index);
                if index == rule.capture_index(Role::Site) {
                    site = Some(capture.node);
                } else if index == rule.capture_index(Role::Path) {
                    paths.push(literal(capture.node, file.text));
                } else if index == rule.capture_index(Role::Host) {
                    hosts.push(literal(capture.node, file.text));
                } else if index == rule.capture_index(Role::Dynamic) {
                    dynamic = true;
                }
            }

            let Some(site) = site else {
                // Unreachable: validation rejects a rule with no `site` role.
                continue;
            };

            if dynamic {
                // The target is computed. Reporting a capability would claim a
                // path we cannot name; reporting nothing would be silence.
                unresolved.push(Unresolved {
                    reason: UnresolvedReason::ComputedTarget,
                    file: file.path.to_owned(),
                    start_byte: Some(site.start_byte() as u64),
                    end_byte: Some(site.end_byte() as u64),
                    start_line: line_of(site),
                    note: Some(format!(
                        "`{}` matched but the target is computed, so it could not be \
                         resolved to a literal",
                        rule.id
                    )),
                });
                continue;
            }

            // Filter literals through the rule's data. A rule that declares
            // prefixes and matches nothing is a rule that did not fire — this is
            // what makes the reference negative fixture pass, since it opens a
            // real file that simply is not a credential path.
            let paths = retain_matching(paths, &rule.match_data.path_prefixes, |value, pattern| {
                value.starts_with(pattern)
            });
            let hosts = retain_matching(hosts, &rule.match_data.host_suffixes, |value, pattern| {
                value.ends_with(pattern)
            });

            if declares_filter(rule) && paths.is_empty() && hosts.is_empty() {
                continue;
            }

            let Some(evidence) = evidence_for(rule, file, site) else {
                continue;
            };

            let finding = findings
                .entry(term.as_str())
                .or_insert_with(|| Finding::new(term));
            finding.evidence.push(evidence);
            finding.paths.extend(paths);
            finding.hosts.extend(hosts);
            finding.merge(reachability.classify(site.start_byte()));
        }
    }
}

/// Whether a rule filters its literals at all.
fn declares_filter(rule: &CompiledRule) -> bool {
    !rule.match_data.path_prefixes.is_empty() || !rule.match_data.host_suffixes.is_empty()
}

/// Keep only literals satisfying at least one pattern.
fn retain_matching(
    values: Vec<String>,
    patterns: &[String],
    test: impl Fn(&str, &str) -> bool,
) -> Vec<String> {
    if patterns.is_empty() {
        return Vec::new();
    }
    values
        .into_iter()
        .filter(|value| patterns.iter().any(|pattern| test(value, pattern)))
        .collect()
}

/// Build evidence with full provenance for one match.
fn evidence_for(
    rule: &CompiledRule,
    file: &SourceFile<'_>,
    site: Node<'_>,
) -> Option<EvidenceStrict> {
    let snippet = file.text.get(site.start_byte()..site.end_byte())?;
    Some(EvidenceStrict {
        file: file.path.to_owned(),
        start_byte: site.start_byte() as u64,
        end_byte: site.end_byte() as u64,
        start_line: line_of(site)?,
        rule_id: rule.id.clone(),
        // Hashing the captured bytes is what turns a span into something a
        // regression test can check: if the span later covers different text,
        // this changes even when the offsets happen not to.
        snippet_sha256: Digest::of(snippet.as_bytes()),
    })
}

/// A node's 1-indexed start line.
fn line_of(node: Node<'_>) -> Option<NonZeroU64> {
    NonZeroU64::new(node.start_position().row as u64 + 1)
}

/// The extension of a forward-slash path, lowercased.
fn extension_of(path: &str) -> Option<String> {
    let name = path.rsplit('/').next().unwrap_or(path);
    name.rsplit_once('.')
        .map(|(_, extension)| extension.to_ascii_lowercase())
}

/// The value of a string literal node, without quotes or prefixes.
///
/// Deliberately conservative and language-agnostic: strip any leading letters
/// (Python's `r`, `b`, `f`, `u`; JavaScript has none), then one matching pair of
/// quotes, longest first so triple quotes are handled before single ones. Escape
/// sequences are **not** interpreted — a literal containing `\x2e` is reported as
/// written rather than as what it would evaluate to, because interpreting escapes
/// is the beginning of an evaluator and this is a matcher.
fn literal(node: Node<'_>, source: &str) -> String {
    let raw = source
        .get(node.start_byte()..node.end_byte())
        .unwrap_or_default();
    let trimmed = raw.trim_start_matches(|ch: char| ch.is_ascii_alphabetic());

    for quote in ["\"\"\"", "'''", "\"", "'"] {
        if let Some(inner) = trimmed
            .strip_prefix(quote)
            .and_then(|rest| rest.strip_suffix(quote))
        {
            return inner.to_owned();
        }
    }
    trimmed.to_owned()
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    reason = "a failed unwrap in a test is the test failing"
)]
mod tests {
    use super::*;

    #[test]
    fn extensions_are_taken_from_the_basename() {
        assert_eq!(extension_of("scripts/run.PY").as_deref(), Some("py"));
        assert_eq!(extension_of("a.b/c.py").as_deref(), Some("py"));
        assert_eq!(extension_of("Makefile"), None);
    }

    #[test]
    fn filters_reject_everything_when_no_pattern_matches() {
        let values = vec!["templates/default.toml".to_owned()];
        let patterns = vec!["~/.aws/".to_owned()];
        assert!(retain_matching(values, &patterns, |value, pattern| value
            .starts_with(pattern))
        .is_empty());
    }

    #[test]
    fn entry_policy_covers_documented_and_conventional_files() {
        let rules = skillmap_rules::load(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("..")
                .join(".."),
        );

        // Documented by the body: imported or invoked, so its top level runs.
        assert!(is_entered(
            "scripts/helper.py",
            LoadPhase::Reference,
            &rules
        ));
        assert!(is_entered("SKILL.md", LoadPhase::OnTrigger, &rules));

        // Unreferenced, but conventionally an entry point.
        assert!(is_entered(
            "scripts/main.py",
            LoadPhase::Unreferenced,
            &rules
        ));

        // Unreferenced and unremarkable: nothing established that it runs.
        assert!(!is_entered(
            "scripts/exfil.py",
            LoadPhase::Unreferenced,
            &rules
        ));
        // A language with no grammar has no conventions to consult.
        assert!(!is_entered(
            "scripts/main.rb",
            LoadPhase::Unreferenced,
            &rules
        ));
    }

    #[test]
    fn a_rule_with_no_patterns_filters_nothing_through() {
        // An empty pattern list means the rule does not filter; the caller checks
        // `declares_filter` before treating an empty result as "did not fire".
        assert!(
            retain_matching(vec!["x".to_owned()], &[], |value, pattern| value
                .starts_with(pattern))
            .is_empty()
        );
    }
}
