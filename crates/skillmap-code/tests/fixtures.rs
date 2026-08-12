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

/// Every code-plane fixture directory: `fixtures/<lang>/<rule>/`.
///
/// Derived from the rules themselves, not from a listing of `fixtures/`. This
/// was a blocklist — skip `bundles/`, `markdown/`, `adversarial/` — until T8
/// added `fixtures/projects/`, whose two version directories were promptly read
/// as languages and failed for having no rule fixtures. A blocklist has to be
/// updated by whoever adds the next directory; asking the ruleset which
/// languages carry `proven` rules cannot fall out of date.
///
/// `markdown` drops out for free and for the right reason: its rules are tier
/// `pattern`, this crate only executes tier `proven`, and asserting here that a
/// markdown positive produces a finding would be asserting that the code plane
/// does something invariant 5 forbids. `skillmap-instr` covers those.
fn fixture_dirs(rules: &RuleSet) -> Vec<PathBuf> {
    let root = repo_root().join("fixtures");
    let mut found = Vec::new();

    let mut languages: Vec<&str> = rules
        .rules
        .iter()
        .filter(|rule| matches!(rule.claim, skillmap_rules::Claim::Capability(_)))
        .map(|rule| rule.language.as_str())
        .collect();
    languages.sort_unstable();
    languages.dedup();

    for language in languages {
        let dir = root.join(language);
        assert!(
            dir.is_dir(),
            "rules exist for `{language}` but {dir:?} does not: invariant 8"
        );
        for rule in std::fs::read_dir(dir).unwrap().flatten() {
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
    let dirs = fixture_dirs(&rules());
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

    for dir in fixture_dirs(&rules) {
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
fn a_language_with_a_grammar_but_no_proven_rules_is_not_called_unsupported() {
    // Markdown has a grammar (the instruction plane uses it) but no `proven`
    // rules. Saying nothing about a .md file is therefore correct and is *not* a
    // gap: `unsupported_language` would claim the analysis could not read it,
    // which is a different and false statement.
    let rules = rules();
    let analysis = analyze(
        &[SourceFile {
            path: "notes.md",
            text: "# notes

Read ~/.aws/credentials.
",
            entered: true,
        }],
        &rules,
    );
    assert!(analysis.capabilities.is_empty());
    assert!(
        analysis.unresolved.is_empty(),
        "a grammar exists, so nothing is unresolved: {:?}",
        analysis.unresolved
    );
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
                path: "build.go",
                text: "package main\n\nfunc main() {}\n",
                entered: true,
            },
        ],
        &rules,
    );

    assert!(analysis.capabilities.is_empty());
    // Sorted here rather than relied upon: `Analysis` is emitted in input order,
    // and it is the manifest canonicalizer that imposes an order later. A test
    // that depended on the intermediate ordering would be asserting something
    // this type deliberately does not promise.
    let mut reported: Vec<&str> = analysis
        .unresolved
        .iter()
        .filter(|entry| entry.reason == UnresolvedReason::UnsupportedLanguage)
        .map(|entry| entry.file.as_str())
        .collect();
    reported.sort_unstable();
    assert_eq!(
        reported,
        ["build.go", "helper.rb"],
        "ruby and go have no grammar. Naming a `.sh` here would test nothing now \
         that shell is ported — and that this test had to change is the proof \
         the grammar landed."
    );
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

#[test]
fn a_shell_redirect_fires_on_input_and_not_on_output() {
    // Found by the T3 labelling pass, on a real bundle: `sh.credential-read.dotfile`
    // reported a *write* as a read, because tree-sitter-bash parses `<`, `>` and
    // `>>` to one `file_redirect` node and the query did not say which it wanted.
    //
    // The negative fixture covers the write. This covers the read, and the two
    // together are what make the `"<"` in the query load-bearing: without this
    // test, deleting that whole pattern would leave the suite green.
    let rules = rules();

    let fires = |source: &str| {
        !analyze(
            &[SourceFile {
                path: "run.sh",
                text: source,
                entered: true,
            }],
            &rules,
        )
        .capabilities
        .is_empty()
    };

    // `~/.aws/credentials`, not `~/.netrc`: the prefix list carries `.netrc`
    // without a tilde, so `~/.netrc` matches no prefix. The first version of
    // this test used it and failed for a reason that had nothing to do with
    // redirects — a test can fail for the wrong reason and still look like it
    // proved something.
    assert!(
        fires("cat < ~/.aws/credentials\n"),
        "an input redirect from a credential path must be reported"
    );
    assert!(
        !fires("cat > ~/.aws/credentials\n"),
        "an output redirect writes the file; `credential-read` must not claim it"
    );
    assert!(
        !fires("printf x >> ~/.aws/credentials\n"),
        "an appending redirect is also a write"
    );
}

#[test]
fn a_python_read_text_on_a_variable_is_never_silent() {
    // Originally this asserted the case produced `unresolved: computed_target` —
    // the best available answer when `env_file.read_text()` could not be resolved
    // at all. Constant folding now resolves `base / ".env"` to a file *named*
    // `.env`, so the same input produces a capability instead. The assertion
    // moved with the behaviour rather than being deleted: what must hold in both
    // worlds is that the read is never passed over in silence.
    let rules = rules();

    let analyse = |source: &str| {
        analyze(
            &[SourceFile {
                path: "loader.py",
                text: source,
                entered: true,
            }],
            &rules,
        )
    };

    let folded = analyse(
        "def load(base):
    env_file = base / '.env'
    return env_file.read_text()
",
    );
    assert!(
        !folded.capabilities.is_empty(),
        "a join with an unknown head still ends in a file called .env"
    );

    // Genuinely unfoldable: the path comes out of a call this does not model, so
    // there is no tail to claim. It must say so rather than stay quiet.
    let opaque = analyse(
        "def load():
    p = compute_path()
    return p.read_text()
",
    );
    assert!(
        opaque.capabilities.is_empty(),
        "nothing is known about this path"
    );
    assert!(
        opaque
            .unresolved
            .iter()
            .any(|entry| entry.reason == UnresolvedReason::ComputedTarget),
        "and the read must still be reported as unresolved, not skipped"
    );
}

/// The path shapes real bundles use, from the T3 labelling pass.
///
/// Every credential read in the labelled corpus reaches its path by computation;
/// not one uses a string literal. These are the actual shapes, transcribed.
#[test]
fn constant_folding_resolves_the_shapes_the_corpus_actually_uses() {
    let rules = rules();

    let fires = |name: &str, source: &str| {
        !analyze(
            &[SourceFile {
                path: name,
                text: source,
                entered: true,
            }],
            &rules,
        )
        .capabilities
        .is_empty()
    };

    // A single-assignment constant composed from home(), then joined.
    assert!(
        fires(
            "a.py",
            "from pathlib import Path\nBASE = Path.home() / '.myapp'\n\
             def load():\n    return (BASE / '.env').read_text()\n"
        ),
        "constant propagation through one assignment"
    );

    // An unknowable head with a knowable tail: matched by name, not location.
    assert!(
        fires(
            "b.py",
            "import os\ndef load(root):\n    return open(os.path.join(root, '.env')).read()\n"
        ),
        "unknown head, known filename"
    );

    // `os.path.dirname(__file__)` is a real directory this cannot name.
    assert!(
        fires(
            "c.py",
            "import os\nP = os.path.join(os.path.dirname(os.path.abspath(__file__)), '.token')\n\
             def load():\n    return open(P).read()\n"
        ),
        "dirname is an unknown head, and the tail still identifies the file"
    );

    // JavaScript: homedir() and join.
    assert!(
        fires(
            "d.js",
            "const os = require('os');\nconst path = require('path');\n\
             const P = path.join(os.homedir(), '.myapp', '.env');\n\
             function load() { return require('fs').readFileSync(P, 'utf8'); }\n"
        ),
        "homedir and join in javascript"
    );
}

#[test]
fn constant_folding_does_not_invent_credentials() {
    // The other half, and the one that matters more: 0 false positives across 92
    // labelled corpus bundles is the claim this test defends locally.
    let rules = rules();

    let fires = |name: &str, source: &str| {
        !analyze(
            &[SourceFile {
                path: name,
                text: source,
                entered: true,
            }],
            &rules,
        )
        .capabilities
        .is_empty()
    };

    assert!(
        !fires(
            "a.py",
            "from pathlib import Path\nBASE = Path.home() / '.myapp'\n\
             def load():\n    return (BASE / 'settings.json').read_text()\n"
        ),
        "a resolved path that is not a credential path must stay quiet"
    );
    assert!(
        !fires(
            "b.py",
            "def load(root):\n    return open(root + 'production.env').read()\n"
        ),
        "component-boundary matching: production.env is not .env"
    );
    assert!(
        !fires(
            "c.py",
            "BASE = compute()\nBASE = other()\n\
             def load():\n    return open(BASE).read()\n"
        ),
        "a name assigned twice has no single value; folding must not pick one"
    );
}

#[test]
fn a_credential_directory_is_matched_by_the_directory_and_not_by_the_filename() {
    // The third matching mode, and the miss that motivated it. A real bundle
    // reads `~/.clawdbot/credentials/homebridge.json`: the filename is named
    // after the integration, so no list of filenames reaches it, and it sits
    // under a home directory too broad for any prefix list to claim. The
    // directory is the only knowable part.
    //
    // `path_suffixes` already carried `credentials`, which is why this needed a
    // new mode rather than a new entry: that matches a path *ending* in
    // `/credentials`, not a file filed inside one.
    let rules = rules();

    let fires = |source: &str| {
        !analyze(
            &[SourceFile {
                path: "load.js",
                text: source,
                entered: true,
            }],
            &rules,
        )
        .capabilities
        .is_empty()
    };

    let read = |expr: &str| {
        format!(
            "const fs = require('fs');\n\
             const os = require('os');\n\
             const path = require('path');\n\
             const p = {expr};\n\
             fs.readFileSync(p, 'utf8');\n"
        )
    };

    assert!(
        fires(&read(
            "path.join(os.homedir(), '.clawdbot', 'credentials', 'homebridge.json')"
        )),
        "a file inside a `credentials/` directory must be reported"
    );

    // Both ends bounded. An unbounded substring match would fire on all three
    // of these, and the first is the one a real home directory would contain.
    assert!(
        !fires(&read(
            "path.join(os.homedir(), 'my-credentials', 'notes.txt')"
        )),
        "`my-credentials` is not the component `credentials`"
    );
    assert!(
        !fires(&read(
            "path.join(os.homedir(), 'credentialsx', 'notes.txt')"
        )),
        "`credentialsx` is not the component `credentials`"
    );

    // And the mode must not resurrect what the prefix list deliberately does
    // not claim: a path that resolves completely and matches nothing stays
    // silent rather than becoming an unresolved entry.
    let manifest = analyze(
        &[SourceFile {
            path: "load.js",
            text: &read("path.join(os.homedir(), 'my-credentials', 'notes.txt')"),
            entered: true,
        }],
        &rules,
    );
    assert!(
        manifest.unresolved.is_empty(),
        "a fully resolved non-match is not an analysis gap: {:?}",
        manifest.unresolved
    );
}

#[test]
fn the_two_exec_terms_are_disjoint_and_an_fstring_is_never_called_static() {
    // A rule declares exactly one capability, and the engine cannot choose a
    // term based on how a capture folded. So `process.exec` and
    // `process.exec.dynamic` are two queries, and the whole design rests on
    // their being structurally unable to match the same site. If they overlap,
    // one call reports both terms and the manifest contradicts itself.
    //
    // The load-bearing half is the f-string. An interpolated string parses as
    // `(string)` exactly like a plain one, so the static query excludes it with
    // `#not-match?` while the dynamic query matches its `(interpolation)` child.
    // That only works if tree-sitter actually evaluates the negated predicate —
    // if it silently ignored it, `subprocess.run(f"rm {path}")` would be
    // reported as a statically known program, which is the opposite of true and
    // the more dangerous direction to be wrong in. This asserts it rather than
    // trusting the version.
    let rules = rules();

    let terms = |source: &str| -> Vec<String> {
        let mut found: Vec<String> = analyze(
            &[SourceFile {
                path: "run.py",
                text: source,
                entered: true,
            }],
            &rules,
        )
        .capabilities
        .iter()
        .map(|c| c.capability.as_str().to_owned())
        .collect();
        found.sort_unstable();
        found.dedup();
        found
    };

    let cases: [(&str, &str); 5] = [
        ("subprocess.run([\"git\", \"status\"])\n", "process.exec"),
        ("subprocess.check_output(\"git status\")\n", "process.exec"),
        ("os.system(\"df -h\")\n", "process.exec"),
        // `subprocess.run(cmd)` is deliberately absent: it reports nothing, and
        // why is asserted in the test below.
        (
            "subprocess.run([sys.executable, \"x.py\"])\n",
            "process.exec.dynamic",
        ),
        (
            "subprocess.run(f\"rm -rf {path}\", shell=True)\n",
            "process.exec.dynamic",
        ),
    ];

    for (source, expected) in cases {
        let found = terms(source);
        assert_eq!(
            found,
            vec![expected.to_owned()],
            "`{source_trimmed}` must report exactly [{expected}], got {found:?}",
            source_trimmed = source.trim_end()
        );
    }
}

#[test]
fn a_bare_identifier_argv_is_not_claimed_to_be_dynamic() {
    // The correction the corpus forced. `subprocess.run(cmd)` was reported as
    // `process.exec.dynamic` until three labelled bundles showed why that is
    // wrong: in each, `cmd` was a single-assignment literal list one line up —
    // `cmd = ["ffmpeg", "-i", src]` — so argv[0] is `ffmpeg` and perfectly
    // knowable. The engine can fold that; a tree-sitter query cannot, and it has
    // no way to tell a name that folds from one that does not.
    //
    // So the rule stops claiming. Reporting `dynamic` for a foldable name is a
    // false statement about how much a reader can know, and it was wrong in
    // three of the four corpus sites that reached it.
    //
    // The cost is a real miss: a genuinely computed `cmd` now reports nothing.
    // What makes that the better trade is that the shapes which CANNOT fold are
    // still matched — an attribute, a call, a subscript and an interpolated
    // command line all still report. Closing the rest needs the engine to fold
    // argv[0] and choose a term from the result, which it cannot do today
    // because a rule declares exactly one capability.
    let rules = rules();

    let manifest = analyze(
        &[SourceFile {
            path: "run.py",
            text: "cmd = [\"ffmpeg\", \"-i\", src]\nsubprocess.run(cmd)\n",
            entered: true,
        }],
        &rules,
    );
    assert!(
        manifest.capabilities.is_empty(),
        "a foldable argv[0] must not be claimed unknowable: {:?}",
        manifest.capabilities
    );

    // The shapes that genuinely cannot fold still report.
    for source in [
        "subprocess.run([sys.executable, \"x.py\"])\n",
        "subprocess.run(config.command)\n",
        "subprocess.run(build_argv())\n",
    ] {
        let found = analyze(
            &[SourceFile {
                path: "run.py",
                text: source,
                entered: true,
            }],
            &rules,
        );
        assert!(
            found
                .capabilities
                .iter()
                .any(|c| c.capability.as_str() == "process.exec.dynamic"),
            "`{}` must still report dynamic",
            source.trim_end()
        );
    }
}

#[test]
fn an_env_secret_fires_on_a_read_and_never_on_a_write() {
    // The direct counterpart of `a_shell_redirect_fires_on_input_and_not_on_output`,
    // and it exists because this repository has already shipped that defect once
    // in the other direction: `sh.credential-read.dotfile` reported `cat > .env`
    // — a write — as a credential read, because tree-sitter-bash uses one node
    // for every redirect direction and the query never said which it wanted.
    //
    // Here the grammar offers no operator to match on at all. A read
    // `process.env.OPENAI_API_KEY` and a write `process.env.OPENAI_API_KEY = v`
    // are the same node, differing only in which field of the parent they
    // occupy, and tree-sitter has no negation. So the query anchors on read
    // CONTEXTS instead. Deleting those anchors would make the rule report a
    // hand-rolled dotenv loader — which SETS credentials — as a reader of them,
    // and two bundles in the labelled corpus load .env exactly that way.
    let rules = rules();

    let fires = |path: &str, source: &str| {
        analyze(
            &[SourceFile {
                path,
                text: source,
                entered: true,
            }],
            &rules,
        )
        .capabilities
        .iter()
        .any(|c| c.capability.as_str() == "env.read.secret")
    };

    // Reads, in each context the corpus actually uses.
    assert!(fires("a.js", "const k = process.env.OPENAI_API_KEY;\n"));
    assert!(fires("a.js", "f({ key: process.env.STRIPE_SECRET });\n"));
    assert!(fires(
        "a.js",
        "const k = process.env.APIFY_TOKEN || null;\n"
    ));
    assert!(fires("a.py", "k = os.getenv(\"OPENAI_API_KEY\")\n"));
    assert!(fires(
        "a.py",
        "k = os.environ.get(\"DEPLOYER_PRIVATE_KEY\")\n"
    ));

    // Writes. Setting a credential is not reading one, and calling it a read
    // inverts the direction the reader cares about.
    assert!(
        !fires("a.js", "process.env.OPENAI_API_KEY = \"x\";\n"),
        "assigning to the environment is a write"
    );
    assert!(
        !fires("a.js", "if (!process.env[k]) process.env[k] = v;\n"),
        "the hand-rolled dotenv shape is a write"
    );
    assert!(
        !fires("a.py", "os.environ[\"OPENAI_API_KEY\"] = value\n"),
        "assigning to os.environ is a write"
    );

    // And the names a looser regex would catch. Every one is real.
    for name in [
        "CACHE_KEY",
        "PRIMARY_KEY",
        "MAX_TOKENS",
        "TOKENIZER",
        "CLIENT_ID",
    ] {
        assert!(
            !fires("a.js", &format!("const v = process.env.{name};\n")),
            "`{name}` is not a secret name"
        );
    }
}
