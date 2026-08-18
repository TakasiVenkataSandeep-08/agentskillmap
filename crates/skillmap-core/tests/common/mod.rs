//! A maximal manifest, shared by the golden and determinism suites.
//!
//! "Maximal" is the point: it exercises every optional field, every array that
//! has a declared sort order, and at least one variant of every closed enum. A
//! fixture that only covers the common path would let a new field reach the
//! schema without anything noticing it never got serialized.
//!
//! It also carries **deliberate ties** — two evidence entries at one site, two
//! `unresolved` entries agreeing on `(file, reason)`, two `diagnostics` agreeing
//! on `code`, two advisory findings agreeing on `(kind, first evidence)`. Without
//! them the declared sort keys are already total on this input, and the entire
//! tiebreak mechanism could be deleted with every test still passing. Each tie is
//! marked below; `the_fixture_actually_contains_ties` fails if one is removed.

#![allow(
    dead_code,
    reason = "each integration test binary uses a different subset"
)]
#![allow(
    clippy::expect_used,
    reason = "a 0 line number here is an authoring mistake in the fixture, and \
              failing loudly at test time is the right response. Invariant 10 \
              bans this in library code, where input is hostile by assumption."
)]

use skillmap_core::{
    Advisory, AdvisoryFinding, AdvisoryKind, AdvisoryRun, BundleKind, Capability, CapabilityTerm,
    Detail, Diagnostic, DiagnosticCode, Digest, Disclosure, EvidenceAdvisory, EvidenceStrict,
    Instruction, InstructionSignal, InventoryEntry, LoadPhase, Manifest, NonEmpty, ParseStatus,
    Reachability, Target, Tool, Unresolved, UnresolvedReason, SCHEMA_VERSION,
};
use std::num::NonZeroU64;

/// 1-indexed line number. Panics on 0, which is a test authoring error.
pub fn line(n: u64) -> NonZeroU64 {
    NonZeroU64::new(n).expect("line numbers in fixtures are 1-indexed")
}

/// A single-element advisory evidence list.
fn cite(file: &str, start_line: u64) -> EvidenceAdvisory {
    EvidenceAdvisory {
        file: file.to_owned(),
        start_line: line(start_line),
    }
}

fn evidence(file: &str, start_byte: u64, start_line: u64, rule_id: &str) -> EvidenceStrict {
    EvidenceStrict {
        file: file.to_owned(),
        start_byte,
        end_byte: start_byte + 36,
        start_line: line(start_line),
        rule_id: rule_id.to_owned(),
        snippet_sha256: Digest::of(format!("{file}:{start_byte}-{rule_id}").as_bytes()),
    }
}

/// A manifest touching every field the schema declares.
///
/// Built deliberately **out of order** — unsorted inventory, unsorted evidence,
/// duplicate trigger terms, an empty `detail` — so that anything comparing
/// against it is comparing against canonicalization having actually run.
#[must_use]
pub fn maximal() -> Manifest {
    let inventory = vec![
        InventoryEntry {
            path: "scripts/collect.py".to_owned(),
            size: 902,
            sha256: Digest::of(b"collect"),
            load_phase: LoadPhase::Reference,
            parsed_as: "python".to_owned(),
            parse_status: ParseStatus::Ok,
        },
        InventoryEntry {
            path: "SKILL.md".to_owned(),
            size: 1834,
            sha256: Digest::of(b"skill"),
            load_phase: LoadPhase::OnTrigger,
            parsed_as: "markdown".to_owned(),
            parse_status: ParseStatus::Ok,
        },
        InventoryEntry {
            path: "vendor/blob.bin".to_owned(),
            size: 4096,
            sha256: Digest::of(b"blob"),
            load_phase: LoadPhase::Unreferenced,
            parsed_as: "binary".to_owned(),
            parse_status: ParseStatus::Unsupported,
        },
        InventoryEntry {
            path: "reference/setup.md".to_owned(),
            size: 640,
            sha256: Digest::of(b"setup"),
            load_phase: LoadPhase::Always,
            parsed_as: "markdown".to_owned(),
            parse_status: ParseStatus::Error,
        },
    ];

    let content_digest = skillmap_core::content_digest(
        &inventory
            .iter()
            .map(|e| (e.path.clone(), e.sha256))
            .collect::<Vec<_>>(),
    );

    Manifest {
        schema_version: SCHEMA_VERSION.to_owned(),
        tool: Tool {
            name: "skillmap".to_owned(),
            // Tracks the crate rather than restating it. A hardcoded string
            // here meant the blessed golden advertised a tool version that had
            // not existed since the last bump — the same drift class as a
            // README total nothing recomputes.
            version: env!("CARGO_PKG_VERSION").to_owned(),
        },
        target: Target {
            kind: BundleKind::Skill,
            name: "example-skill".to_owned(),
            resolver: "claude-code".to_owned(),
            root: "example-skill".to_owned(),
            content_digest,
        },
        inventory,
        disclosure: Disclosure {
            description_bytes: 412,
            // Free-form, verbatim from third-party frontmatter — deliberately not
            // taxonomy terms, and deliberately unsorted with a duplicate.
            declared_capabilities: vec![
                "pdf-generation".to_owned(),
                "aws-config".to_owned(),
                "pdf-generation".to_owned(),
            ],
            trigger_terms: vec![
                "format".to_owned(),
                "aws".to_owned(),
                "credentials".to_owned(),
                "aws".to_owned(),
            ],
            reference_files: 4,
            unreferenced_files: 1,
        },
        capabilities: vec![
            Capability {
                capability: CapabilityTerm::NetEgress,
                reachability: Reachability::Present,
                detail: Some(Detail {
                    paths: None,
                    hosts: Some(vec![
                        "upload.example.com".to_owned(),
                        "api.example.com".to_owned(),
                        "upload.example.com".to_owned(),
                    ]),
                }),
                evidence: NonEmpty::of(
                    evidence("scripts/collect.py", 980, 41, "py.net.requests-post"),
                    [
                        evidence("scripts/collect.py", 610, 24, "py.net.requests-post"),
                        // TIE with the entry above on (file, start_byte): two rules
                        // firing at one site is ordinary, and the declared evidence
                        // order cannot separate them. Only the tiebreak can.
                        evidence("scripts/collect.py", 610, 24, "py.net.urllib-open"),
                    ],
                ),
            },
            Capability {
                capability: CapabilityTerm::FsReadCredential,
                reachability: Reachability::Observed,
                detail: Some(Detail {
                    paths: Some(vec!["~/.ssh/id_rsa".to_owned(), "~/.aws/credentials".to_owned()]),
                    hosts: None,
                }),
                evidence: NonEmpty::of(
                    evidence("scripts/collect.py", 412, 17, "py.credential-read.dotfile"),
                    [],
                ),
            },
            Capability {
                capability: CapabilityTerm::CodeDynamicEval,
                reachability: Reachability::Unresolved,
                // An empty detail must be dropped, not rendered as `{}`.
                detail: Some(Detail::default()),
                evidence: NonEmpty::of(evidence("scripts/run.sh", 12, 2, "sh.eval.dynamic"), []),
            },
        ],
        instructions: vec![
            Instruction {
                signal: InstructionSignal::ConfigMutation,
                evidence: NonEmpty::of(evidence("reference/setup.md", 300, 22, "instr.config-mutation"), []),
            },
            Instruction {
                signal: InstructionSignal::FetchAsInstruction,
                evidence: NonEmpty::of(
                    evidence("reference/setup.md", 88, 6, "instr.fetch-as-instruction"),
                    [],
                ),
            },
        ],
        unresolved: vec![
            Unresolved {
                reason: UnresolvedReason::UnsupportedLanguage,
                file: "vendor/blob.bin".to_owned(),
                start_byte: None,
                end_byte: None,
                start_line: None,
                note: None,
            },
            Unresolved {
                reason: UnresolvedReason::DynamicDispatch,
                file: "scripts/run.sh".to_owned(),
                start_byte: Some(90),
                end_byte: Some(118),
                start_line: Some(line(4)),
                note: Some("exec target is a shell variable".to_owned()),
            },
            // TIE with the entry above on (file, reason), with start_byte ABSENT.
            // docs/02-manifest-schema.md says an absent start_byte sorts BEFORE any
            // present one; without an entry like this, nothing in the suite ever
            // evaluates that rule.
            Unresolved {
                reason: UnresolvedReason::DynamicDispatch,
                file: "scripts/run.sh".to_owned(),
                start_byte: None,
                end_byte: None,
                start_line: None,
                note: Some("call graph never reaches a statically-known target".to_owned()),
            },
        ],
        advisory: Advisory::Enabled(AdvisoryRun {
            model: "claude-sonnet-5".to_owned(),
            prompt_sha256: Digest::of(b"prompt-template-v1"),
            findings: vec![
                AdvisoryFinding {
                    kind: AdvisoryKind::InjectionAttempt,
                    claim: "reference/setup.md addresses the reading agent directly".to_owned(),
                    evidence: NonEmpty::of(cite("reference/setup.md", 31), []),
                },
                // TIE with the finding above on (kind, first evidence file, first
                // evidence start_line). Only `claim` — the last declared key —
                // separates them.
                AdvisoryFinding {
                    kind: AdvisoryKind::InjectionAttempt,
                    claim: "reference/setup.md asserts prior authorization".to_owned(),
                    evidence: NonEmpty::of(cite("reference/setup.md", 31), []),
                },
                AdvisoryFinding {
                    kind: AdvisoryKind::DisclosureDelta,
                    claim: "reference/setup.md instructs credential upload; description mentions only formatting".to_owned(),
                    evidence: NonEmpty::of(
                        cite("reference/setup.md", 12),
                        [cite("SKILL.md", 3)],
                    ),
                },
            ],
        }),
        diagnostics: vec![
            Diagnostic {
                code: DiagnosticCode::RuleValidationError,
                file: Some("rules/ruby/exec.toml".to_owned()),
                note: Some("query references capture @target, not declared in [captures]".to_owned()),
            },
            Diagnostic {
                code: DiagnosticCode::PolicyLoadError,
                file: None,
                note: None,
            },
            // TIE with the entry above on `code`, but carrying a file.
            // docs/02-manifest-schema.md says an absent file sorts BEFORE any
            // present one; this pair is what actually evaluates that rule.
            Diagnostic {
                code: DiagnosticCode::PolicyLoadError,
                file: Some("policy.toml".to_owned()),
                note: Some("unexpected key `severity`".to_owned()),
            },
        ],
    }
}
