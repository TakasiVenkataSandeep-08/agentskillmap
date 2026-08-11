#![warn(missing_docs)]

//! The quarantined semantic pass — tier `advisory`.
//!
//! One question, from `docs/04-semantic-layer.md`:
//!
//! > Do the deep-loaded files instruct the agent to do things the ~100-token
//! > description does not disclose?
//!
//! That is the **disclosure delta**. Not "is this skill bad", not "is this
//! dangerous" — the moment this component grades anything it has become a risk
//! scorer and broken invariant 1.
//!
//! # The quarantine is structural, in three places
//!
//! `docs/04-semantic-layer.md` is explicit that this must be enforced by crate
//! boundaries and not by review discipline. Three things enforce it:
//!
//! 1. **The dependency graph.** `Cargo.toml` has no `skillmap-code` and no
//!    `skillmap-instr`. The types needed to touch a `Capability` or an
//!    `Instruction` are not in scope here.
//! 2. **The input type.** [`BundleView`] carries a description and file text.
//!    It has no field for capabilities, instructions, or unresolved entries, so
//!    this pass cannot read a deterministic finding, let alone reprioritize one.
//! 3. **The output type.** [`Outcome`] hands back an `Advisory`, a list of
//!    `Unresolved` for content it could not cover, and diagnostics. There is no
//!    variant by which it could return a capability.
//!
//! `tests/quarantine.rs` proves the consequence a consumer actually cares
//! about: the deterministic branches of a manifest are byte-identical whether
//! this pass ran, ran and found nothing, or ran and returned something maximally
//! hostile.
//!
//! # Off by default
//!
//! Invariant 9. A default build does not contain an HTTP client — see
//! [`provider`] — and even a build that does makes no call until asked. The
//! semantic pass is the single network path in the scan flow.
//!
//! # What this crate does not have
//!
//! Published precision and recall. `docs/04-semantic-layer.md` requires them
//! against the held-out split of the labeled corpus, and **the corpus is
//! harvested but not labelled**, so there is no ground truth to score against.
//! [`variance`] is built and runnable and has not been run against a live model.
//! Numbers would have to be invented to fill that gap, which is the one thing
//! this project is defined against. See `docs/00-tasks.md`, T7.

pub mod prompt;
pub mod provider;
pub mod validate;
pub mod variance;

pub use provider::{Provider, ProviderError};

use skillmap_core::{
    Advisory, AdvisoryRun, Diagnostic, DiagnosticCode, Unresolved, UnresolvedReason,
};

/// One file, as this pass sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileView {
    /// Forward-slash path relative to the bundle root.
    pub path: String,
    /// The file's text.
    pub text: String,
}

impl FileView {
    /// Number of 1-indexed lines a citation could name.
    ///
    /// Counts the lines a human would see in an editor: a trailing newline does
    /// not create an extra empty one, and a file with no trailing newline still
    /// has its last line.
    #[must_use]
    pub fn line_count(&self) -> u64 {
        if self.text.is_empty() {
            return 0;
        }
        let counted = self.text.lines().count();
        u64::try_from(counted).unwrap_or(u64::MAX)
    }
}

/// Everything the semantic pass is allowed to know about a bundle.
///
/// Deliberately anaemic. The absence of a `capabilities` field here is not an
/// oversight to be corrected later — it is the quarantine, expressed as a type.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BundleView {
    /// The description the agent sees at session start. The thing the deep
    /// files are compared against; the delta is meaningless without it.
    pub description: String,
    /// Deep-loaded files, in inventory order.
    pub files: Vec<FileView>,
}

/// Bounds on how much content one run may send.
#[derive(Debug, Clone, Copy)]
pub struct Limits {
    /// Largest prompt, in bytes of untrusted content.
    pub max_content_bytes: usize,
    /// Largest single chunk, in bytes.
    pub max_chunk_bytes: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            // Roughly 60k tokens of content, well inside a modern context and
            // far outside any bundle in the 2026-08 corpus. A bound that is
            // never hit is still worth having: the failure it prevents is a
            // hostile bundle padding itself until the call fails or costs.
            max_content_bytes: 240_000,
            max_chunk_bytes: 24_000,
        }
    }
}

/// What one run produced.
#[derive(Debug)]
pub struct Outcome {
    /// The advisory branch, ready to drop into a manifest.
    pub advisory: Advisory,
    /// Content this pass could not cover. Invariant 3: a partially analysed
    /// bundle says so.
    pub unresolved: Vec<Unresolved>,
    /// Problems with the run.
    pub diagnostics: Vec<Diagnostic>,
}

impl Outcome {
    /// A run that did not happen.
    #[must_use]
    pub fn disabled() -> Self {
        Self {
            advisory: Advisory::Disabled,
            unresolved: Vec::new(),
            diagnostics: Vec::new(),
        }
    }
}

/// One piece of a bundle, quoted and ready to send.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chunk {
    /// Which file it came from.
    pub file: String,
    /// 1-indexed line the chunk starts at, so the model can cite absolute lines.
    pub start_line: u64,
    /// The text.
    pub text: String,
}

/// Split a bundle into chunks, and say what did not fit.
///
/// By file, then by section — `docs/04-semantic-layer.md`'s order. Sections are
/// markdown headings, because that is what the deep files overwhelmingly are;
/// a file with no headings is one chunk, split on lines if it must be.
///
/// Anything dropped for size becomes an [`UnresolvedReason::SizeLimit`] entry.
/// A bundle that was too big to read in full and said nothing about it would be
/// reporting a partial analysis as a complete one.
#[must_use]
pub fn chunk(bundle: &BundleView, limits: &Limits) -> (Vec<Chunk>, Vec<Unresolved>) {
    let mut chunks = Vec::new();
    let mut unresolved = Vec::new();
    let mut budget = limits.max_content_bytes;

    for file in &bundle.files {
        let mut covered = true;

        for section in sections(file, limits.max_chunk_bytes) {
            if section.text.len() > budget {
                covered = false;
                break;
            }
            budget -= section.text.len();
            chunks.push(section);
        }

        if !covered {
            unresolved.push(Unresolved {
                reason: UnresolvedReason::SizeLimit,
                file: file.path.clone(),
                start_byte: None,
                end_byte: None,
                start_line: None,
                note: Some(
                    "not sent to the semantic pass: the bundle exceeded the content \
                     budget for one call"
                        .to_owned(),
                ),
            });
        }
    }

    (chunks, unresolved)
}

/// Split one file at markdown headings, then hard-split anything still too big.
fn sections(file: &FileView, max_chunk: usize) -> Vec<Chunk> {
    let mut sections: Vec<Chunk> = Vec::new();
    let mut current = String::new();
    let mut start_line: u64 = 1;
    let mut line_number: u64 = 1;

    for line in file.text.lines() {
        let is_heading = line.starts_with('#');
        if is_heading && !current.is_empty() {
            sections.push(Chunk {
                file: file.path.clone(),
                start_line,
                text: std::mem::take(&mut current),
            });
            start_line = line_number;
        }
        current.push_str(line);
        current.push('\n');
        line_number = line_number.saturating_add(1);
    }
    if !current.is_empty() {
        sections.push(Chunk {
            file: file.path.clone(),
            start_line,
            text: current,
        });
    }

    // A single section can still be enormous — one heading over a whole file, or
    // a file with no headings at all. Split on line boundaries so a citation
    // still lands on a real line.
    let mut out = Vec::new();
    for section in sections {
        if section.text.len() <= max_chunk {
            out.push(section);
            continue;
        }
        let mut buffer = String::new();
        let mut offset = section.start_line;
        let mut consumed: u64 = 0;
        for line in section.text.lines() {
            if buffer.len().saturating_add(line.len()) > max_chunk && !buffer.is_empty() {
                out.push(Chunk {
                    file: section.file.clone(),
                    start_line: offset,
                    text: std::mem::take(&mut buffer),
                });
                offset = offset.saturating_add(consumed);
                consumed = 0;
            }
            buffer.push_str(line);
            buffer.push('\n');
            consumed = consumed.saturating_add(1);
        }
        if !buffer.is_empty() {
            out.push(Chunk {
                file: section.file,
                start_line: offset,
                text: buffer,
            });
        }
    }

    out
}

/// Render every chunk into the untrusted channel, with its own header.
///
/// The header sits **outside** the quoted block. A path or line number inside
/// the channel would be third-party text claiming to be a path, and the model
/// would have no way to tell the difference.
#[must_use]
pub fn render_chunks(chunks: &[Chunk]) -> (String, usize) {
    let mut rendered = String::new();
    let mut neutralized = 0;

    for piece in chunks {
        let (quoted, count) = prompt::quote(&piece.text);
        neutralized += count;
        rendered.push_str(&format!(
            "\n### `{}`, from line {}\n\n{quoted}\n",
            piece.file, piece.start_line
        ));
    }

    (rendered, neutralized)
}

/// Run the pass.
///
/// One model call. The provider's surface is a string in and a string out
/// ([`Provider`]), so there is no tool the model could be given and no second
/// round trip it could steer.
///
/// A failed call reports [`Advisory::Disabled`] plus a `semantic_call_failed`
/// diagnostic — never "ran, found nothing". Those are different claims, and
/// conflating them is the failure invariant 3 exists to prevent.
#[must_use]
pub fn analyze(bundle: &BundleView, provider: &dyn Provider, limits: &Limits) -> Outcome {
    let phrases = match prompt::auditor_directed_phrases() {
        Ok(phrases) => phrases,
        Err(error) => {
            // The list is compiled in, so this is a build-time mistake. Running
            // without the net would be worse than not running.
            return Outcome {
                advisory: Advisory::Disabled,
                unresolved: Vec::new(),
                diagnostics: vec![Diagnostic {
                    code: DiagnosticCode::SemanticCallFailed,
                    file: Some("prompts/auditor-directed.toml".to_owned()),
                    note: Some(format!("the pinned phrase list does not parse: {error}")),
                }],
            };
        }
    };

    let (chunks, unresolved) = chunk(bundle, limits);
    let (rendered, _neutralized) = render_chunks(&chunks);
    let filled = prompt::render(&bundle.description, &rendered);

    let raw = match provider.complete(&filled) {
        Ok(raw) => raw,
        Err(error) => {
            return Outcome {
                advisory: Advisory::Disabled,
                unresolved,
                diagnostics: vec![Diagnostic {
                    code: DiagnosticCode::SemanticCallFailed,
                    file: None,
                    note: Some(error.to_string()),
                }],
            };
        }
    };

    let checked = validate::response(&raw, bundle, &phrases);

    Outcome {
        advisory: Advisory::Enabled(AdvisoryRun {
            model: provider.model().to_owned(),
            prompt_sha256: prompt::digest(),
            findings: checked.findings,
        }),
        unresolved,
        diagnostics: checked.diagnostics,
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "a failed assertion in a test is the test failing"
)]
mod tests {
    use super::*;
    use provider::Replay;

    fn bundle() -> BundleView {
        BundleView {
            description: "Summarises meeting notes into action items.".to_owned(),
            files: vec![
                FileView {
                    path: "SKILL.md".to_owned(),
                    text: "# notes\n\nFormatting rules live in `reference/style.md`.\n".to_owned(),
                },
                FileView {
                    path: "reference/style.md".to_owned(),
                    text: "# Style\n\nUse sentence case.\n\n# Tone\n\nBe brief.\n".to_owned(),
                },
            ],
        }
    }

    #[test]
    fn a_failed_call_reports_disabled_not_empty() {
        // The distinction the whole project turns on. "Could not check" must
        // never serialize as "checked and found nothing".
        let outcome = analyze(&bundle(), &provider::Unavailable, &Limits::default());
        assert_eq!(outcome.advisory, Advisory::Disabled);
        assert_eq!(outcome.diagnostics.len(), 1);
        assert_eq!(
            outcome.diagnostics[0].code,
            DiagnosticCode::SemanticCallFailed
        );
    }

    #[test]
    fn a_run_that_found_nothing_is_enabled_and_empty() {
        let outcome = analyze(&bundle(), &Replay::silent(), &Limits::default());
        match &outcome.advisory {
            Advisory::Enabled(run) => {
                assert!(run.findings.is_empty());
                assert_eq!(run.model, "replay/silent");
                assert_eq!(run.prompt_sha256.to_wire(), prompt::digest().to_wire());
            }
            Advisory::Disabled => panic!("the pass ran; it must not report disabled"),
        }
    }

    #[test]
    fn chunking_splits_at_headings_and_keeps_absolute_lines() {
        let (chunks, unresolved) = chunk(&bundle(), &Limits::default());
        assert!(unresolved.is_empty());

        let style: Vec<&Chunk> = chunks
            .iter()
            .filter(|piece| piece.file == "reference/style.md")
            .collect();
        assert_eq!(style.len(), 2, "two headings, two sections");
        assert_eq!(style[0].start_line, 1);
        assert_eq!(
            style[1].start_line, 5,
            "the second section must cite the line it really starts on"
        );
    }

    #[test]
    fn a_file_that_does_not_fit_becomes_unresolved_rather_than_vanishing() {
        // Invariant 3. A bundle too big to read in full, reported as a complete
        // analysis, is the exact shape of the lie this project exists to catch.
        let big = BundleView {
            description: "x".to_owned(),
            files: vec![FileView {
                path: "huge.md".to_owned(),
                text: "line\n".repeat(10_000),
            }],
        };
        let limits = Limits {
            max_content_bytes: 100,
            max_chunk_bytes: 50,
        };
        let (_, unresolved) = chunk(&big, &limits);
        assert_eq!(unresolved.len(), 1);
        assert_eq!(unresolved[0].reason, UnresolvedReason::SizeLimit);
        assert_eq!(unresolved[0].file, "huge.md");
    }

    #[test]
    fn every_chunk_is_quoted_and_its_header_is_outside_the_quote() {
        // A path inside the channel is third-party text claiming to be a path.
        let (chunks, _) = chunk(&bundle(), &Limits::default());
        let (rendered, _) = render_chunks(&chunks);

        for piece in &chunks {
            let header = format!("### `{}`, from line {}", piece.file, piece.start_line);
            let position = rendered.find(&header).expect("header missing");
            let quote_after = rendered[position..].find(prompt::OPEN);
            assert!(
                quote_after.is_some(),
                "each header must be followed by its quoted block"
            );
        }
        assert_eq!(
            rendered.matches(prompt::OPEN).count(),
            chunks.len(),
            "one channel per chunk"
        );
    }

    #[test]
    fn line_counts_match_what_a_human_would_see() {
        let view = |text: &str| FileView {
            path: "f".to_owned(),
            text: text.to_owned(),
        };
        assert_eq!(view("").line_count(), 0);
        assert_eq!(view("a\n").line_count(), 1);
        assert_eq!(view("a").line_count(), 1, "no trailing newline");
        assert_eq!(view("a\nb\n").line_count(), 2);
        assert_eq!(view("a\n\n").line_count(), 2);
    }
}
