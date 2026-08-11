#![warn(missing_docs)]

//! The instruction plane — tier `pattern`.
//!
//! The code plane asks what a bundle's *scripts* can do. This asks what its
//! *prose* tells the agent to do, which is a different question with a different
//! and much weaker kind of answer.
//!
//! # This tier is deliberately weak, and says so
//!
//! Findings here are lexical. A sentence matched a pattern. That is all that has
//! been established — there is no execution, no reachability, no proof anything
//! ever acts on the sentence. Invariant 5 keeps them in `instructions`, never in
//! `capabilities`, and **a `pattern` finding is never promoted to `proven`, under
//! any circumstance, including "we are now very confident about it"**. The
//! separation is enforced three times over: the rule loader rejects a `pattern`
//! rule that names a capability term, [`analyze`] only ever constructs
//! [`Instruction`] values, and the manifest keeps the two in separate arrays.
//!
//! # Why it exists anyway
//!
//! `AGENTS.md`: the structural hole is progressive disclosure. A reviewer reads
//! `SKILL.md`, sees something benign, installs; the payload lives in prose that
//! only enters context on trigger. Prose *is* the attack surface for an agent, so
//! a scanner that only reads scripts is reading the wrong half of the bundle.
//!
//! # What is not here yet, and why
//!
//! `instruction.silence` and `instruction.privilege_claim` are **not shipped**.
//! `docs/00-tasks.md` is explicit that their negative fixtures must be drawn from
//! real corpus bundles *before* the queries are written, because they are the two
//! signals most likely to earn this project attention and most likely to
//! false-positive on ordinary skills that discuss logging verbosity or
//! permissions. T3 has not been run, so those fixtures do not exist, so the
//! queries are not written. Shipping them on invented negatives would be exactly
//! the untested false-positive generator invariant 8 describes.

use skillmap_core::{Digest, EvidenceStrict, Instruction, InstructionSignal, NonEmpty};
use skillmap_rules::{Claim, Role, RuleSet};
use std::collections::BTreeMap;
use std::num::NonZeroU64;
use streaming_iterator::StreamingIterator as _;
use tree_sitter::{Node, Parser, QueryCursor};

/// A prose file to scan.
#[derive(Debug, Clone, Copy)]
pub struct ProseFile<'a> {
    /// Forward-slash path relative to the bundle root, as it appears in evidence.
    pub path: &'a str,
    /// The file's text, already LF-normalized by the parser.
    pub text: &'a str,
}

/// Run every `pattern` rule over every prose file.
///
/// Returns only [`Instruction`] values. There is no code path here that can
/// produce a `Capability`, which is what makes the tier separation structural
/// rather than a filtering step somebody has to remember.
#[must_use]
pub fn analyze(files: &[ProseFile<'_>], rules: &RuleSet) -> Vec<Instruction> {
    // Keyed on the signal's wire name rather than the enum, for the same reason
    // the code plane does: `InstructionSignal` has no `Ord`, deliberately, so
    // that reordering its variants cannot reorder a manifest.
    let mut findings: BTreeMap<&'static str, (InstructionSignal, Vec<EvidenceStrict>)> =
        BTreeMap::new();

    for file in files {
        let Some(language) =
            extension_of(file.path).and_then(|extension| rules.language_for_extension(&extension))
        else {
            // Not an omission worth reporting: a file with no prose grammar is
            // simply not prose. The code plane already accounts for files it
            // cannot read, and duplicating that here would double-count.
            continue;
        };

        let mut parser = Parser::new();
        if parser.set_language(&language.grammar).is_err() {
            continue;
        }
        let Some(tree) = parser.parse(file.text, None) else {
            continue;
        };
        let bytes = file.text.as_bytes();

        for rule in rules.rules_for(&language.name) {
            let Claim::Instruction(signal) = rule.claim else {
                continue;
            };
            let Some(site_index) = rule.capture_index(Role::Site) else {
                continue;
            };

            let mut cursor = QueryCursor::new();
            let mut matches = cursor.matches(&rule.query, tree.root_node(), bytes);
            while let Some(matched) = matches.next() {
                for capture in matched.captures {
                    if capture.index != site_index {
                        continue;
                    }
                    let Some(evidence) = evidence_for(&rule.id, file, capture.node) else {
                        continue;
                    };
                    findings
                        .entry(signal.as_str())
                        .or_insert_with(|| (signal, Vec::new()))
                        .1
                        .push(evidence);
                }
            }
        }
    }

    let mut instructions: Vec<Instruction> = findings
        .into_values()
        .filter_map(|(signal, evidence)| {
            Some(Instruction {
                signal,
                // Invariant 4: a finding nobody can point at is not a finding.
                evidence: NonEmpty::new(evidence)?,
            })
        })
        .collect();
    instructions.sort_by(|a, b| a.signal.as_str().cmp(b.signal.as_str()));
    instructions
}

/// Build evidence with full provenance.
///
/// Note that this is `EvidenceStrict`, the same type the code plane uses, with
/// every field required. Invariant 4 says so explicitly: *"No exceptions,
/// including for instruction-plane findings."* A weak tier is not a licence for
/// weak provenance — a lexical finding still fired at an exact byte range, and a
/// reviewer must be able to open the file and read the sentence.
fn evidence_for(rule_id: &str, file: &ProseFile<'_>, site: Node<'_>) -> Option<EvidenceStrict> {
    let snippet = file.text.get(site.start_byte()..site.end_byte())?;
    Some(EvidenceStrict {
        file: file.path.to_owned(),
        start_byte: site.start_byte() as u64,
        end_byte: site.end_byte() as u64,
        start_line: NonZeroU64::new(site.start_position().row as u64 + 1)?,
        rule_id: rule_id.to_owned(),
        snippet_sha256: Digest::of(snippet.as_bytes()),
    })
}

/// The extension of a forward-slash path, lowercased.
fn extension_of(path: &str) -> Option<String> {
    let name = path.rsplit('/').next().unwrap_or(path);
    name.rsplit_once('.')
        .map(|(_, extension)| extension.to_ascii_lowercase())
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    reason = "a failed unwrap in a test is the test failing"
)]
mod tests {
    use super::*;

    #[test]
    fn extensions_are_lowercased() {
        assert_eq!(extension_of("docs/SETUP.MD").as_deref(), Some("md"));
        assert_eq!(extension_of("README"), None);
    }
}
