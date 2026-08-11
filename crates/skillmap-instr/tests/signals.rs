//! The instruction plane, against its fixtures and against real prose.
//!
//! T5's stated "done when" — *"false-positive rate on the benign stratum is
//! measured and published, per signal"* — is **not met and cannot be**, because
//! the benign stratum is a T3 output and the harvest has not run. What is here
//! instead is the strongest false-positive check available without a corpus: run
//! every instruction rule over this repository's **own documentation**, which
//! discusses exfiltration, prompt injection and agent-config edits at length
//! without ever instructing any of them.
//!
//! That is a genuinely adversarial negative set — prose *about* the exact
//! behaviour each rule detects is the hardest case a lexical rule faces — and it
//! is real text nobody wrote to make a rule look good. It is still not the
//! measured base rate T5 asks for, and it is not a substitute for one.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "a failed assertion in a test is the test failing, which is the point"
)]

use skillmap_core::InstructionSignal;
use skillmap_instr::{analyze, ProseFile};
use skillmap_rules::{load, Claim, RuleSet};
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

fn rules() -> RuleSet {
    let set = load(&repo_root());
    assert!(
        set.diagnostics.is_empty(),
        "the shipped rules must load cleanly: {:?}",
        set.diagnostics
    );
    set
}

/// Every `fixtures/markdown/<rule>/` directory, sorted.
fn fixture_dirs() -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = std::fs::read_dir(repo_root().join("fixtures").join("markdown"))
        .unwrap()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect();
    found.sort();
    found
}

#[test]
fn every_instruction_rule_ships_both_fixtures() {
    let rules = rules();
    let shipped: Vec<&str> = rules
        .rules
        .iter()
        .filter(|rule| matches!(rule.claim, Claim::Instruction(_)))
        .map(|rule| rule.id.as_str())
        .collect();
    assert!(
        !shipped.is_empty(),
        "at least one instruction rule must ship"
    );

    for dir in fixture_dirs() {
        assert!(dir.join("positive.md").is_file(), "{dir:?} has no positive");
        assert!(
            dir.join("negative.md").is_file(),
            "{dir:?} has no negative: invariant 8"
        );
    }
    assert_eq!(
        fixture_dirs().len(),
        shipped.len(),
        "every instruction rule needs its own fixture directory"
    );
}

#[test]
fn positive_fixtures_fire() {
    let rules = rules();
    for dir in fixture_dirs() {
        let text = std::fs::read_to_string(dir.join("positive.md")).unwrap();
        let found = analyze(
            &[ProseFile {
                path: "positive.md",
                text: &text,
            }],
            &rules,
        );
        assert!(
            !found.is_empty(),
            "{dir:?}: the positive fixture must produce an instruction finding"
        );
    }
}

#[test]
fn negative_fixtures_do_not_fire() {
    let rules = rules();
    for dir in fixture_dirs() {
        let text = std::fs::read_to_string(dir.join("negative.md")).unwrap();
        let found = analyze(
            &[ProseFile {
                path: "negative.md",
                text: &text,
            }],
            &rules,
        );
        assert!(
            found.is_empty(),
            "{dir:?}: the negative fixture must stay quiet, but produced {:?}. \
             These negatives are prose *about* the detected behaviour — if a rule \
             cannot tell description from instruction it is not shippable.",
            found.iter().map(|f| f.signal.as_str()).collect::<Vec<_>>()
        );
    }
}

/// Every markdown document this repository ships, excluding rule fixtures.
fn repository_prose() -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut work = vec![repo_root()];
    while let Some(dir) = work.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name();
            if path.is_dir() {
                // Skip build output, git internals, and the fixture corpora —
                // fixtures are *supposed* to fire.
                if !matches!(
                    name.to_str(),
                    Some(".git" | "target" | "fixtures" | "corpus" | "__pycache__")
                ) {
                    work.push(path);
                }
            } else if path.extension().is_some_and(|ext| ext == "md") {
                found.push(path);
            }
        }
    }
    found.sort();
    found
}

#[test]
fn no_instruction_rule_fires_on_this_repositorys_own_documentation() {
    // The adversarial false-positive check, and the reason it is worth running:
    // AGENTS.md, SECURITY.md and docs/ describe indirect prompt injection,
    // exfiltration, and agent-config writes in detail — in some places quoting
    // the exact phrasing an attacker would use — without instructing any of them.
    //
    // Prose about a behaviour is the hardest false positive a lexical rule faces.
    // A rule that cannot survive this one would drown every security-conscious
    // skill in the ecosystem in noise, and a tier that cries wolf gets ignored,
    // which is worse than not shipping it.
    let rules = rules();
    let documents = repository_prose();
    assert!(
        documents.len() > 10,
        "expected this repository to have real prose to test against, found {}",
        documents.len()
    );

    let mut offences: Vec<String> = Vec::new();
    for path in &documents {
        let Ok(text) = std::fs::read_to_string(path) else {
            continue;
        };
        let relative = path
            .strip_prefix(repo_root())
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/");

        for finding in analyze(
            &[ProseFile {
                path: &relative,
                text: &text,
            }],
            &rules,
        ) {
            for evidence in finding.evidence.iter() {
                let snippet = text
                    .get(evidence.start_byte as usize..evidence.end_byte as usize)
                    .unwrap_or_default();
                offences.push(format!(
                    "{}:{} [{}] {}",
                    relative,
                    evidence.start_line,
                    finding.signal.as_str(),
                    snippet.chars().take(160).collect::<String>()
                ));
            }
        }
    }

    assert!(
        offences.is_empty(),
        "instruction rules fired on this repository's own documentation, which \
         describes these behaviours without instructing them:\n  {}",
        offences.join("\n  ")
    );
}

#[test]
fn findings_carry_full_provenance() {
    // Invariant 4 is explicit that the weak tier gets no discount: "No
    // exceptions, including for instruction-plane findings."
    let rules = rules();
    let text =
        std::fs::read_to_string(repo_root().join("fixtures/markdown/exfil/positive.md")).unwrap();
    let found = analyze(
        &[ProseFile {
            path: "positive.md",
            text: &text,
        }],
        &rules,
    );

    let evidence = found[0].evidence.first().unwrap();
    assert_eq!(evidence.file, "positive.md");
    assert!(!evidence.rule_id.is_empty());
    assert!(evidence.end_byte > evidence.start_byte);
    let snippet = &text[evidence.start_byte as usize..evidence.end_byte as usize];
    assert_eq!(
        evidence.snippet_sha256,
        skillmap_core::Digest::of(snippet.as_bytes())
    );
}

#[test]
fn the_two_riskiest_signals_are_deliberately_not_shipped() {
    // docs/00-tasks.md requires three negative fixtures drawn from *real corpus
    // bundles* before `instruction.silence` and `instruction.privilege_claim` are
    // written. T3 has not run, so those fixtures do not exist, so the queries do
    // not exist. This test exists so that shipping them without corpus negatives
    // is a deliberate act that has to delete an assertion, rather than something
    // that quietly happens.
    let rules = rules();
    let shipped: Vec<InstructionSignal> = rules
        .rules
        .iter()
        .filter_map(|rule| match rule.claim {
            Claim::Instruction(signal) => Some(signal),
            Claim::Capability(_) => None,
        })
        .collect();

    for withheld in [
        InstructionSignal::Silence,
        InstructionSignal::PrivilegeClaim,
    ] {
        assert!(
            !shipped.contains(&withheld),
            "`{}` is shipped, but docs/00-tasks.md requires three negative \
             fixtures drawn from real corpus bundles before its query is written. \
             If T3 has now run and those fixtures exist, update this test in the \
             same commit that adds them.",
            withheld.as_str()
        );
    }
}

#[test]
fn the_instruction_plane_cannot_produce_a_capability() {
    // Invariant 5 by construction: `analyze` returns `Vec<Instruction>`, so there
    // is no code path from this crate to `capabilities`. This asserts the other
    // half — that no shipped `pattern` rule claims a capability term, which the
    // loader rejects but which is worth pinning where a reader will see it.
    let rules = rules();
    for rule in &rules.rules {
        if let Claim::Instruction(signal) = rule.claim {
            assert!(
                signal.as_str().starts_with("instruction."),
                "{} claims `{}`, which is not in the instruction namespace",
                rule.id,
                signal.as_str()
            );
        }
    }
}

#[test]
fn analysis_is_deterministic() {
    let rules = rules();
    let text =
        std::fs::read_to_string(repo_root().join("fixtures/markdown/exfil/positive.md")).unwrap();
    let file = ProseFile {
        path: "positive.md",
        text: &text,
    };
    assert_eq!(
        format!("{:?}", analyze(&[file], &rules)),
        format!("{:?}", analyze(&[file], &rules))
    );
}
