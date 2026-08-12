//! T6's first "done when" clause: **CI fails on a seeded regression.**
//!
//! A gate nobody has watched fail is a gate nobody knows works. Each test here
//! seeds a specific regression and asserts the gate rejects it — including the
//! two that would otherwise slip through, because they make the suite *greener*:
//! deleting a case, and letting a case fall back to pending.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "a failed assertion in a test is the test failing, which is the point"
)]

use skillmap_eval::metrics::{regressions, Baseline, Regression};
use skillmap_eval::{baseline_path, cases};
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

fn committed_baseline() -> Baseline {
    let path = baseline_path(&repo_root());
    let text = std::fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!(
            "cannot read {}: {error}. Bless it with \
             `cargo run -p skillmap-eval -- --bless`.",
            path.display()
        )
    });
    serde_json::from_str(&text).expect("the committed baseline must be valid JSON")
}

#[test]
fn the_repository_currently_passes_its_own_gate() {
    let report = skillmap_eval::run(&repo_root()).expect("the rule set must load");
    let baseline = committed_baseline();
    let found = regressions(&baseline, &report.metrics);
    assert!(
        found.is_empty(),
        "the working tree regresses against the committed baseline: {}",
        found
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("; ")
    );
    assert!(report.passed(), "failures: {:?}", report.failures());
}

#[test]
fn a_seeded_failure_fails_the_gate() {
    // The obvious regression: a case that used to pass now fails.
    let baseline = committed_baseline();
    let mut seeded = baseline.metrics.clone();
    seeded.adversarial_passed = seeded.adversarial_passed.saturating_sub(1);
    seeded.adversarial_failed += 1;

    let found = regressions(&baseline, &seeded);
    assert!(
        found.contains(&Regression::CasesFailing(1)),
        "a failing case must fail the build (invariant 11): {found:?}"
    );
}

#[test]
fn a_seeded_deletion_fails_the_gate() {
    // The regression that looks like an improvement. Removing a case leaves zero
    // failures and a green suite; the only thing that changed is that the tool is
    // checked less. A gate that only counted failures would wave this through.
    let baseline = committed_baseline();
    let mut seeded = baseline.metrics.clone();
    seeded.adversarial_passed = seeded.adversarial_passed.saturating_sub(2);

    let found = regressions(&baseline, &seeded);
    assert!(
        found
            .iter()
            .any(|regression| matches!(regression, Regression::CoverageShrank { .. })),
        "deleting cases must fail the build: {found:?}"
    );
}

#[test]
fn a_case_falling_back_to_pending_fails_the_gate() {
    // The subtler version of the same thing: marking a case `requires = ...`
    // silences it without deleting it. Also green, also checking less.
    let baseline = committed_baseline();
    let mut seeded = baseline.metrics.clone();
    seeded.adversarial_passed = seeded.adversarial_passed.saturating_sub(1);
    seeded.pending.insert(
        "sink-in-dead-code".to_owned(),
        "needs some future task".to_owned(),
    );

    let found = regressions(&baseline, &seeded);
    assert!(
        found.iter().any(|regression| matches!(
            regression,
            Regression::CaseBecamePending { id, .. } if id == "sink-in-dead-code"
        )),
        "a case regressing to pending must fail the build: {found:?}"
    );
}

#[test]
fn every_adversarial_case_from_the_spec_is_declared() {
    // docs/05-eval.md lists eight cases as the minimum set. All eight must be
    // present as directories — the three that cannot run yet are declared and
    // marked pending, not omitted. A suite that silently covered five of eight
    // would be the false comfort invariant 3 exists to prevent, one level up.
    let declared: Vec<String> = cases::load_cases(&repo_root())
        .into_iter()
        .map(|case| case.id)
        .collect();

    for required in [
        "capability-added-in-update",
        "computed-credential-path",
        "documented-credential-path",
        "injection-in-reference",
        "legitimate-deploy",
        "obfuscated-exec",
        "sink-in-dead-code",
        "unreferenced-payload",
    ] {
        assert!(
            declared.iter().any(|id| id == required),
            "docs/05-eval.md requires the `{required}` adversarial case; declared: {declared:?}"
        );
    }
    assert_eq!(declared.len(), 8, "declared: {declared:?}");
}

#[test]
fn pending_cases_state_a_reason_naming_what_they_wait_on() {
    // "Pending" with no reason is indistinguishable from "forgotten".
    let report = skillmap_eval::run(&repo_root()).unwrap();
    let pending = report.pending();
    assert!(
        !pending.is_empty(),
        "three cases are expected to be pending"
    );

    for outcome in pending {
        let reason = outcome.pending.as_deref().unwrap_or_default();

        // Structure, not an allowlist of task names. This was a list of
        // substrings — "T7", "T8", "rule", "corpus" — until T7 landed and left
        // `injection-in-reference` pending for a *different* reason, at which
        // point the test failed for being out of date rather than because
        // anything was wrong. A test that has to be edited every time a blocker
        // is correctly re-described is measuring the wrong thing.
        let Some(what) = reason.strip_prefix("needs ") else {
            panic!("`{}` is pending without a reason: {reason:?}", outcome.id);
        };
        assert!(
            what.len() > 10,
            "`{}` names its blocker too vaguely to act on: {reason:?}",
            outcome.id
        );
    }
}

#[test]
fn the_baseline_does_not_claim_a_corpus_the_eval_did_not_use() {
    // A corpus now exists — snapshot `2026-08`, 34,284 bundles, published in the
    // README. This field still has to stay empty, and the reason has changed:
    // the corpus is measured but **not labelled**, so the eval has no ground
    // truth to score against and has never been run over it. Naming a snapshot
    // here would attach real-looking provenance to numbers that came from the
    // fixture and adversarial suites alone.
    //
    // Fill this in when a labelled split exists and the eval actually consumes
    // it — not when a corpus merely exists.
    let baseline = committed_baseline();
    assert!(
        baseline.corpus_snapshot.is_none(),
        "the baseline names corpus snapshot {:?}, but the eval has not been run          against a labelled corpus — the harvest alone does not license this field",
        baseline.corpus_snapshot
    );
    assert!(
        baseline.note.contains("NOT the published numbers"),
        "the baseline must say what it is not"
    );
}

#[test]
fn every_term_a_rule_detects_is_either_scored_or_a_declared_gap() {
    // Invariant 11 says precision and recall are published. It does not say what
    // happens to a term that ships a rule and has no ground truth — and the
    // honest answer, today, is *nothing*: `corpus::run` iterates
    // `terms_labelled`, so an unlabelled term gets no row at all. Not a zero,
    // not an error. Absent.
    //
    // That alone would be survivable if the false-positive rate covered it, but
    // it does not either: the per-stratum rate counts only scored terms, so a
    // new rule could fire on every benign bundle and `code_clean 0/36` would
    // still print. The README's headline would silently narrow from a claim
    // about the tool to a claim about credential rules.
    //
    // So the gap has to be declared. A term may be measured
    // (`terms_labelled`) or admitted as unmeasured (`terms_detected_unscored`),
    // and shipping a rule for a term in neither list fails here.
    //
    // This test needs `rules/` and `corpus/labels.toml` only — never
    // `corpus/raw/`, which is gitignored and makes the corpus suite itself
    // inert in CI. That is deliberate: it is the one part of the corpus
    // machinery that can actually gate a build on every platform.
    let rules = skillmap_rules::load(&repo_root());
    assert!(rules.diagnostics.is_empty(), "{:?}", rules.diagnostics);

    let labels = match skillmap_eval::corpus::Labels::load(&repo_root().join("corpus/labels.toml"))
    {
        Ok(labels) => labels,
        // A fresh clone has the file, since it is committed. If it is genuinely
        // absent there is nothing to check and nothing to claim.
        Err(skillmap_eval::corpus::Error::Absent(_)) => return,
        Err(error) => panic!("corpus/labels.toml is present and unusable: {error}"),
    };

    let mut undeclared: Vec<String> = Vec::new();
    for rule in &rules.rules {
        let skillmap_rules::Claim::Capability(term) = rule.claim else {
            // Instruction signals are a separate plane with a separate
            // vocabulary; the corpus labels capabilities.
            continue;
        };
        let term = term.as_str();
        let scored = labels.terms_labelled.iter().any(|have| have == term);
        let declared = labels
            .terms_detected_unscored
            .iter()
            .any(|have| have == term);
        if !scored && !declared {
            undeclared.push(format!("{} detects `{term}`", rule.id));
        }
    }
    undeclared.sort_unstable();
    undeclared.dedup();

    assert!(
        undeclared.is_empty(),
        "these rules detect terms the corpus neither scores nor declares as a gap:\n  {}\n\n\
         Add each term to `terms_labelled` in corpus/labels.toml if a labelling pass \
         covered it exhaustively, or to `terms_detected_unscored` if it did not. Widening \
         `terms_labelled` without relabelling is the worse of the two mistakes: every \
         genuine detection would score as a false positive, because a label's empty \
         `capabilities` array means \"not looked for\", not \"not present\".",
        undeclared.join("\n  ")
    );
}

#[test]
fn the_baseline_is_canonical_json() {
    // It is diffed on every change, so it gets the same framing as the manifest.
    let text = std::fs::read_to_string(baseline_path(&repo_root())).unwrap();
    assert!(text.ends_with("}\n"));
    assert!(!text.contains('\r'));

    let value: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(
        text,
        serde_json::to_string_pretty(&value).unwrap() + "\n",
        "sorted keys, two-space indent, one trailing newline"
    );
}

#[test]
fn the_quiet_cases_are_genuinely_quiet() {
    // docs/05-eval.md: "The last two are as important as the rest. A scanner that
    // cannot stay quiet on legitimate behaviour will not survive contact with
    // users." Asserted directly rather than inferred from the aggregate.
    let rules = skillmap_rules::load(&repo_root());
    assert!(rules.diagnostics.is_empty());

    let documented = repo_root().join("fixtures/adversarial/documented-credential-path/bundle");
    let manifest = skillmap_eval::pipeline::analyze(&documented, &rules).unwrap();
    assert!(
        manifest.capabilities.is_empty() && manifest.instructions.is_empty(),
        "documentation that merely names credential paths must produce nothing: {:?} {:?}",
        manifest.capabilities,
        manifest.instructions
    );

    let deploy = repo_root().join("fixtures/adversarial/legitimate-deploy/bundle");
    let manifest = skillmap_eval::pipeline::analyze(&deploy, &rules).unwrap();
    let json = manifest.to_canonical_json().unwrap().to_lowercase();
    for word in ["malicious", "suspicious", "risk_score", "severity"] {
        assert!(
            !json.contains(word),
            "a legitimate deploy skill must be reported without {word:?}"
        );
    }
}
