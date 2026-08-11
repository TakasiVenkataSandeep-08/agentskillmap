//! T4's acceptance criteria, against the shipped rule fixtures.
//!
//! `docs/00-tasks.md`: *"every rule's fixtures pass, `unsupported_language` is
//! emitted for everything unported, and the adversarial 'sink in dead code' case
//! reports `present` rather than `observed`."*
//!
//! Every rule in `rules/` is discovered and run against its own fixtures, so
//! adding a rule adds coverage automatically. A rule shipped without both
//! fixtures fails here rather than being quietly untested (invariant 8).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "a failed assertion in a test is the test failing, which is the point"
)]

use skillmap_code::{analyze, SourceFile};
use skillmap_core::{Reachability, UnresolvedReason};
use skillmap_rules::{load, RuleSet};
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

/// Every fixture directory: `fixtures/<lang>/<rule>/`.
fn fixture_dirs() -> Vec<PathBuf> {
    let mut found = Vec::new();
    let root = repo_root().join("fixtures");
    for language in std::fs::read_dir(&root).unwrap().flatten() {
        // `fixtures/bundles/` is T2's whole-bundle corpus, not a rule fixture.
        if !language.path().is_dir() || language.file_name() == "bundles" {
            continue;
        }
        for rule in std::fs::read_dir(language.path()).unwrap().flatten() {
            if rule.path().is_dir() {
                found.push(rule.path());
            }
        }
    }
    found.sort();
    found
}

/// The single file in `dir` whose stem is `stem`.
fn fixture_file(dir: &Path, stem: &str) -> Option<PathBuf> {
    std::fs::read_dir(dir).ok()?.flatten().find_map(|entry| {
        let path = entry.path();
        (path.file_stem().is_some_and(|found| found == stem)).then_some(path)
    })
}

#[test]
fn every_rule_ships_both_fixtures() {
    // Invariant 8: a rule with no negative fixture is an untested false-positive
    // generator. This is what makes that statement enforced rather than hoped for.
    let dirs = fixture_dirs();
    assert!(!dirs.is_empty(), "there must be at least one rule fixture");

    for dir in dirs {
        assert!(
            fixture_file(&dir, "positive").is_some(),
            "{dir:?} has no positive fixture"
        );
        assert!(
            fixture_file(&dir, "negative").is_some(),
            "{dir:?} has no negative fixture: a rule proved only by a positive \
             example has, in practice, been tested against nothing"
        );
    }
}

#[test]
fn positive_fixtures_fire_and_negative_fixtures_do_not() {
    let rules = rules();

    for dir in fixture_dirs() {
        let positive = fixture_file(&dir, "positive").unwrap();
        let negative = fixture_file(&dir, "negative").unwrap();

        let positive_text = std::fs::read_to_string(&positive).unwrap();
        let negative_text = std::fs::read_to_string(&negative).unwrap();

        let positive_name = positive.file_name().unwrap().to_string_lossy().into_owned();
        let negative_name = negative.file_name().unwrap().to_string_lossy().into_owned();

        let fired = analyze(
            &[SourceFile {
                path: &positive_name,
                text: &positive_text,
                entered: true,
            }],
            &rules,
        );
        assert!(
            !fired.capabilities.is_empty() || !fired.unresolved.is_empty(),
            "{dir:?}: the positive fixture must produce a finding"
        );

        let quiet = analyze(
            &[SourceFile {
                path: &negative_name,
                text: &negative_text,
                entered: true,
            }],
            &rules,
        );
        assert!(
            quiet.capabilities.is_empty(),
            "{dir:?}: the negative fixture must not fire, but produced {:?}",
            quiet.capabilities
        );
    }
}

/// Render an analysis as the `expected.json` fragment shape.
///
/// This is what `skillmap rules bless` will write once the CLI exists (T9); the
/// logic lives here so the promise in `docs/03-rules-authoring.md` is backed by
/// something that runs, rather than by a command that does not exist.
fn fragment(analysis: &skillmap_code::Analysis) -> serde_json::Value {
    serde_json::json!({
        "capabilities": analysis.capabilities.iter().map(|capability| {
            serde_json::json!({
                "capability": capability.capability.as_str(),
                "reachability": capability.reachability.as_str(),
                "detail": capability.detail.as_ref().map(|detail| serde_json::json!({
                    "paths": detail.paths.clone().unwrap_or_default(),
                })),
                "evidence": capability.evidence.iter().map(|evidence| serde_json::json!({
                    "file": evidence.file,
                    "start_byte": evidence.start_byte,
                    "end_byte": evidence.end_byte,
                    "start_line": evidence.start_line.get(),
                    "rule_id": evidence.rule_id,
                    "snippet_sha256": evidence.snippet_sha256.to_wire(),
                })).collect::<Vec<_>>(),
            })
        }).collect::<Vec<_>>(),
        "unresolved": analysis.unresolved.iter().map(|entry| serde_json::json!({
            "reason": entry.reason.as_str(),
            "file": entry.file,
            "start_byte": entry.start_byte,
            "end_byte": entry.end_byte,
            "start_line": entry.start_line.map(std::num::NonZeroU64::get),
        })).collect::<Vec<_>>(),
    })
}

#[test]
fn the_reference_rule_matches_its_expected_fragment() {
    // fixtures/python/credential-read/expected.json is the contract every other
    // rule is copied from, and docs/03-rules-authoring.md points contributors at
    // it. Comparing against the file itself — rather than against values
    // duplicated into this test — is what keeps that document honest: if the
    // engine's output shape drifts, the committed contract fails, not a copy of it.
    //
    // Re-bless with `SKILLMAP_BLESS=1 cargo test -p skillmap-code`, then read the
    // diff. The byte offsets are generated, never hand-maintained.
    let rules = rules();
    let dir = repo_root()
        .join("fixtures")
        .join("python")
        .join("credential-read");

    let mut produced = serde_json::Map::new();
    produced.insert(
        "note".to_owned(),
        serde_json::Value::String(
            "Generated. Re-bless with `SKILLMAP_BLESS=1 cargo test -p skillmap-code` \
             (this becomes `skillmap rules bless` at T9) and read the diff. Byte \
             offsets are never hand-maintained."
                .to_owned(),
        ),
    );

    for stem in ["positive", "negative"] {
        let path = fixture_file(&dir, stem).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        let analysis = analyze(
            &[SourceFile {
                path: &name,
                text: &text,
                entered: true,
            }],
            &rules,
        );
        produced.insert(stem.to_owned(), fragment(&analysis));
    }

    let rendered =
        serde_json::to_string_pretty(&serde_json::Value::Object(produced)).unwrap() + "\n";
    let expected_path = dir.join("expected.json");

    if std::env::var_os("SKILLMAP_BLESS").is_some() {
        std::fs::write(&expected_path, rendered.as_bytes()).unwrap();
        return;
    }

    let expected = std::fs::read_to_string(&expected_path).unwrap();
    assert_eq!(
        rendered, expected,
        "the reference fixture's expected fragment changed. If that was intended, \
         re-bless with SKILLMAP_BLESS=1 and read the diff."
    );
}

#[test]
fn the_reference_fixture_still_encodes_what_the_docs_claim() {
    // Guards the guard: blessing would happily record a wrong answer. These are
    // the specific claims docs/03-rules-authoring.md and the rule's own comments
    // make about this fixture, asserted independently of the blessed file.
    let rules = rules();
    let text = std::fs::read_to_string(
        repo_root()
            .join("fixtures")
            .join("python")
            .join("credential-read")
            .join("positive.py"),
    )
    .unwrap();
    let analysis = analyze(
        &[SourceFile {
            path: "positive.py",
            text: &text,
            entered: true,
        }],
        &rules,
    );

    assert_eq!(analysis.capabilities.len(), 1);
    let capability = &analysis.capabilities[0];
    assert_eq!(capability.capability.as_str(), "fs.read.credential");
    assert_eq!(
        capability
            .detail
            .as_ref()
            .and_then(|detail| detail.paths.as_ref())
            .map(Vec::as_slice),
        Some(["~/.aws/credentials".to_owned()].as_slice()),
        "only the credential path survives the [match] filter"
    );

    // Full provenance, per invariant 4.
    let evidence = capability.evidence.first().unwrap();
    assert_eq!(evidence.file, "positive.py");
    assert_eq!(evidence.rule_id, "py.credential-read.dotfile");
    assert!(evidence.end_byte > evidence.start_byte);
    assert!(text
        .get(evidence.start_byte as usize..evidence.end_byte as usize)
        .is_some_and(|snippet| snippet.contains("open")));

    // The computed-target branch: reported, never silent (invariant 3).
    assert_eq!(analysis.unresolved.len(), 1);
    assert_eq!(
        analysis.unresolved[0].reason,
        UnresolvedReason::ComputedTarget
    );
    assert!(analysis.unresolved[0].start_line.is_some());
}

#[test]
fn a_sink_in_dead_code_reports_present_not_observed() {
    // The adversarial case T4's "done when" names explicitly, and the one the
    // reference fixture already encodes: `collect()` is never called, so the
    // credential read inside it exists but was never shown to run.
    let rules = rules();
    let text = std::fs::read_to_string(
        repo_root()
            .join("fixtures")
            .join("python")
            .join("credential-read")
            .join("positive.py"),
    )
    .unwrap();

    let analysis = analyze(
        &[SourceFile {
            path: "positive.py",
            text: &text,
            entered: true,
        }],
        &rules,
    );
    assert_eq!(
        analysis.capabilities[0].reachability,
        Reachability::Present,
        "a sink inside a function nothing calls must not be reported as observed"
    );
}

#[test]
fn a_sink_on_the_entry_path_reports_observed() {
    // The other half of the claim: if `present` were returned unconditionally the
    // dead-code test above would pass for the wrong reason.
    let rules = rules();
    let text = "\
import os

def collect():
    with open(\"~/.aws/credentials\") as handle:
        return handle.read()

collect()
";
    let analysis = analyze(
        &[SourceFile {
            path: "entry.py",
            text,
            entered: true,
        }],
        &rules,
    );
    assert_eq!(
        analysis.capabilities[0].reachability,
        Reachability::Observed,
        "a function called from module level runs when the file does"
    );
}

#[test]
fn a_module_level_sink_reports_observed() {
    let rules = rules();
    let analysis = analyze(
        &[SourceFile {
            path: "top.py",
            text: "creds = open(\"~/.aws/credentials\").read()\n",
            entered: true,
        }],
        &rules,
    );
    assert_eq!(
        analysis.capabilities[0].reachability,
        Reachability::Observed
    );
}

#[test]
fn a_computed_callee_blocks_the_analysis_rather_than_clearing_it() {
    // `present` asserts the analysis looked and found no caller. When a computed
    // callee is in play it cannot assert that, and saying so is the difference
    // between a scanner that is careful and one that is merely quiet.
    let rules = rules();
    let text = "\
import os

def collect():
    with open(\"~/.aws/credentials\") as handle:
        return handle.read()

globals()[os.environ[\"WHICH\"]]()
";
    let analysis = analyze(
        &[SourceFile {
            path: "dispatch.py",
            text,
            entered: true,
        }],
        &rules,
    );
    assert_eq!(
        analysis.capabilities[0].reachability,
        Reachability::Unresolved,
        "a computed callee at module level could reach anything in the file"
    );
}

#[test]
fn a_file_nothing_documents_never_reports_observed() {
    // An unreferenced file does not run on its own, so even a module-level sink
    // in it is `present`: nothing established that those bytes execute.
    let rules = rules();
    let analysis = analyze(
        &[SourceFile {
            path: "orphan.py",
            text: "creds = open(\"~/.aws/credentials\").read()\n",
            entered: false,
        }],
        &rules,
    );
    assert_eq!(analysis.capabilities[0].reachability, Reachability::Present);
}

#[test]
fn an_unported_language_is_reported_not_skipped() {
    // T4's "done when", second clause. Silence here would be indistinguishable
    // from a clean scan (invariant 3).
    let rules = rules();
    let analysis = analyze(
        &[
            SourceFile {
                path: "helper.rb",
                text: "File.read(File.expand_path(\"~/.aws/credentials\"))\n",
                entered: true,
            },
            SourceFile {
                path: "notes.md",
                text: "# notes\n",
                entered: true,
            },
        ],
        &rules,
    );

    assert!(analysis.capabilities.is_empty());
    let reported: Vec<&str> = analysis
        .unresolved
        .iter()
        .filter(|entry| entry.reason == UnresolvedReason::UnsupportedLanguage)
        .map(|entry| entry.file.as_str())
        .collect();
    assert_eq!(reported, ["helper.rb", "notes.md"]);
}

#[test]
fn evidence_snippet_hashes_cover_the_reported_span() {
    // The snippet hash is what turns a byte span into something regression-
    // testable: if the span later covers different text, this changes even when
    // the offsets happen not to.
    let rules = rules();
    let text = "creds = open(\"~/.aws/credentials\").read()\n";
    let analysis = analyze(
        &[SourceFile {
            path: "top.py",
            text,
            entered: true,
        }],
        &rules,
    );

    let evidence = analysis.capabilities[0].evidence.first().unwrap();
    let snippet = &text[evidence.start_byte as usize..evidence.end_byte as usize];
    assert_eq!(
        evidence.snippet_sha256,
        skillmap_core::Digest::of(snippet.as_bytes())
    );
}

#[test]
fn analysis_is_deterministic_across_runs() {
    let rules = rules();
    let text = std::fs::read_to_string(
        repo_root()
            .join("fixtures")
            .join("python")
            .join("credential-read")
            .join("positive.py"),
    )
    .unwrap();
    let file = SourceFile {
        path: "positive.py",
        text: &text,
        entered: true,
    };

    let render = |analysis: skillmap_code::Analysis| {
        format!("{:?}{:?}", analysis.capabilities, analysis.unresolved)
    };
    assert_eq!(
        render(analyze(&[file], &rules)),
        render(analyze(&[file], &rules))
    );
}
