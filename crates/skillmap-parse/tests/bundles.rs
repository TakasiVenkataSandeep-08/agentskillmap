//! T2's acceptance criteria, against the fixture bundle corpus.
//!
//! `docs/00-tasks.md` states it as: a valid manifest per bundle with an empty
//! `capabilities` array and a fully populated inventory, byte-identical across
//! two runs on two platforms. The platform half is CI's `rust` matrix; everything
//! else is here.
//!
//! Golden manifests live in `fixtures/bundles/expected/`. Re-bless after an
//! intentional change with `SKILLMAP_BLESS=1 cargo test -p skillmap-parse`, and
//! read the diff before committing it — a change there means the parser now says
//! something different about the same bytes.
//!
//! The corpus is stored as plain directories rather than under a real
//! `.claude/skills/` tree, deliberately. A committed `.claude/skills/` is a *live*
//! skill directory: Claude Code and every other agent that reads this repository
//! would load these fixtures as installed skills, and one of them is deliberately
//! shaped like an exfiltration payload. `discovers_bundles_under_a_real_claude_tree`
//! covers the real convention by building that layout in a scratch directory
//! instead, so nothing is lost by keeping it out of the repository.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "a failed assertion in a test is the test failing, which is the point. \
              Invariant 10 bans these in library code, where hostile input is the \
              normal case and a crash is a DoS on somebody's CI."
)]

use skillmap_core::{LoadPhase, UnresolvedReason};
use skillmap_parse::{parse_bundle, Limits};
use skillmap_resolve::{discover, ClaudeCode, Resolver, Scope};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// The fixture corpus root — the directory a project checkout would be.
fn corpus_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("fixtures")
        .join("bundles")
}

fn expected_dir() -> PathBuf {
    corpus_root().join("expected")
}

/// Every fixture bundle directory, sorted.
fn fixture_bundles() -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = std::fs::read_dir(corpus_root())
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.is_dir() && path.file_name().is_some_and(|n| n != "expected"))
        .collect();
    found.sort();
    found
}

/// Parse every fixture bundle, keyed by name.
fn parse_all() -> BTreeMap<String, String> {
    let bundles = fixture_bundles();
    assert!(!bundles.is_empty(), "fixture corpus must contain bundles");

    bundles
        .iter()
        .map(|path| {
            let manifest = skillmap_parse::parse_path(path, &ClaudeCode, &Limits::default())
                .unwrap_or_else(|e| panic!("parsing {} failed: {e}", path.display()));
            (
                manifest.target.root.clone(),
                manifest.to_canonical_json().unwrap(),
            )
        })
        .collect()
}

#[test]
fn the_corpus_holds_the_bundles_the_suite_expects() {
    let names: Vec<String> = fixture_bundles()
        .iter()
        .filter_map(|path| Some(path.file_name()?.to_str()?.to_owned()))
        .collect();
    assert_eq!(
        names,
        [
            "malformed-frontmatter",
            "minimal",
            "no-frontmatter",
            "pdf-formatter"
        ]
    );
}

#[test]
fn discovers_bundles_under_a_real_claude_tree() {
    // The committed corpus is flat, so this is what actually exercises the
    // `.claude/skills` convention end to end: copy the fixtures into that layout
    // in a scratch directory and discover them there.
    let temp = TempDir::new("discover");
    let skills = temp.path().join(".claude").join("skills");
    for bundle in fixture_bundles() {
        let name = bundle.file_name().expect("fixture dir has a name");
        copy_tree(&bundle, &skills.join(name));
    }

    let bundles = discover(&ClaudeCode, temp.path(), Scope::Project)
        .unwrap()
        .bundles;
    let names: Vec<&str> = bundles.iter().map(|b| b.name.as_str()).collect();
    assert_eq!(
        names,
        [
            "malformed-frontmatter",
            "minimal",
            "no-frontmatter",
            "pdf-formatter"
        ],
        "discovery must be sorted and complete"
    );

    for bundle in &bundles {
        assert_eq!(bundle.resolver, "claude-code");
        assert_eq!(
            bundle.root, bundle.name,
            "target.root must be the bundle directory relative to the discovery root"
        );
        // Discovering a bundle and naming its directory outright must produce the
        // same manifest: discovery decides where to look, never what is reported.
        let discovered = parse_bundle(bundle, &ClaudeCode, &Limits::default()).unwrap();
        let direct = skillmap_parse::parse_path(
            &corpus_root().join(&bundle.name),
            &ClaudeCode,
            &Limits::default(),
        )
        .unwrap();
        assert_eq!(
            discovered.to_canonical_json().unwrap(),
            direct.to_canonical_json().unwrap(),
            "{}: discovery must not change what the parser reports",
            bundle.name
        );
    }
}

/// Recursively copy a directory tree.
fn copy_tree(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.path().is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), &target).unwrap();
        }
    }
}

#[test]
fn every_bundle_matches_its_golden_manifest() {
    let parsed = parse_all();

    if std::env::var_os("SKILLMAP_BLESS").is_some() {
        std::fs::create_dir_all(expected_dir()).unwrap();
        for (name, json) in &parsed {
            std::fs::write(expected_dir().join(format!("{name}.json")), json.as_bytes()).unwrap();
        }
        return;
    }

    for (name, json) in &parsed {
        let path = expected_dir().join(format!("{name}.json"));
        let expected = std::fs::read_to_string(&path).unwrap_or_else(|err| {
            panic!(
                "cannot read {}: {err}. Re-bless with SKILLMAP_BLESS=1.",
                path.display()
            )
        });
        assert_eq!(
            *json, expected,
            "the manifest for `{name}` changed. If that was intended, re-bless with \
             SKILLMAP_BLESS=1 and read the diff."
        );
    }
}

#[test]
fn two_runs_produce_identical_bytes() {
    // Invariant 2, the local half. The cross-platform half is CI's matrix.
    assert_eq!(parse_all(), parse_all());
}

#[test]
fn no_manifest_carries_a_machine_specific_path() {
    // The single easiest way to break invariant 2 is to let an absolute path,
    // a username, or a backslash reach the artifact.
    let machine_markers = [
        corpus_root().to_string_lossy().into_owned(),
        std::env::var("USERNAME").unwrap_or_else(|_| "\0no-username".to_owned()),
        std::env::var("USER").unwrap_or_else(|_| "\0no-user".to_owned()),
    ];

    for (name, json) in parse_all() {
        for marker in &machine_markers {
            assert!(
                !json.contains(marker.as_str()),
                "{name} leaks the machine-specific string {marker:?} into the manifest"
            );
        }
        assert!(
            !json.contains('\\'),
            "{name} contains a backslash; paths must be forward-slashed on every platform"
        );
    }
}

#[test]
fn capabilities_are_empty_and_inventory_is_populated() {
    // T2's stated acceptance criterion, literally.
    for path in fixture_bundles() {
        let manifest = skillmap_parse::parse_path(&path, &ClaudeCode, &Limits::default()).unwrap();
        let name = manifest.target.root.clone();
        assert!(
            manifest.capabilities.is_empty(),
            "{name}: no rule engine exists yet (T4); capabilities must be empty"
        );
        assert!(manifest.instructions.is_empty(), "{name}");
        assert!(
            !manifest.inventory.is_empty(),
            "{name}: inventory must be populated"
        );
        assert!(
            !manifest.advisory.is_enabled(),
            "{name}: the semantic pass is opt-in and lives in T7"
        );
    }
}

/// The parsed manifest for one fixture bundle.
fn manifest_for(name: &str) -> skillmap_core::Manifest {
    skillmap_parse::parse_path(&corpus_root().join(name), &ClaudeCode, &Limits::default())
        .unwrap_or_else(|e| panic!("fixture bundle `{name}`: {e}"))
}

#[test]
fn load_phases_are_classified_correctly() {
    // The core signal. `pdf-formatter` is built so that every phase is reachable
    // and one file is deliberately reachable by no documented path.
    let manifest = manifest_for("pdf-formatter");
    let phase: BTreeMap<&str, LoadPhase> = manifest
        .inventory
        .iter()
        .map(|entry| (entry.path.as_str(), entry.load_phase))
        .collect();

    assert_eq!(phase["SKILL.md"], LoadPhase::OnTrigger);

    // Linked from the body, and named in an inline code span.
    assert_eq!(phase["reference/setup.md"], LoadPhase::Reference);
    assert_eq!(phase["scripts/fill.py"], LoadPhase::Reference);
    // Reached transitively through a script's own comment, not through markdown.
    assert_eq!(phase["scripts/helpers.py"], LoadPhase::Reference);
    // Reached through a `../`-relative mention inside a reference file.
    assert_eq!(phase["scripts/fetch.py"], LoadPhase::Reference);

    // The signal that matters: nothing in the bundle points at these.
    assert_eq!(phase["scripts/exfil.py"], LoadPhase::Unreferenced);
    assert_eq!(phase["assets/seal.bin"], LoadPhase::Unreferenced);

    // No file is `always`: the always-loaded content is the frontmatter
    // description, which is reported as disclosure.description_bytes.
    assert!(
        manifest
            .inventory
            .iter()
            .all(|e| e.load_phase != LoadPhase::Always),
        "the claude-code resolver has no always-loaded file; see refgraph docs"
    );

    assert_eq!(manifest.disclosure.unreferenced_files, 2);
    assert_eq!(
        manifest.disclosure.reference_files,
        u64::try_from(
            manifest
                .inventory
                .iter()
                .filter(|e| e.load_phase == LoadPhase::Reference)
                .count()
        )
        .unwrap()
    );
}

#[test]
fn disclosure_reports_the_frontmatter_honestly() {
    let manifest = manifest_for("pdf-formatter");
    assert_eq!(manifest.target.name, "pdf-formatter");
    assert!(manifest.disclosure.description_bytes > 0);
    assert!(
        manifest
            .disclosure
            .trigger_terms
            .contains(&"forms".to_owned()),
        "trigger terms: {:?}",
        manifest.disclosure.trigger_terms
    );
    // `allowed-tools` is not a declared-capability key for this resolver, so
    // nothing is claimed. Reading it into the taxonomy would be the lossy
    // mapping step the schema deliberately keeps separate.
    assert!(ClaudeCode.declared_capability_keys().is_empty());
    assert!(manifest.disclosure.declared_capabilities.is_empty());
}

#[test]
fn a_binary_file_is_inventoried_and_reported_unresolved() {
    let manifest = manifest_for("pdf-formatter");
    let entry = manifest
        .inventory
        .iter()
        .find(|e| e.path == "assets/seal.bin")
        .expect("the binary fixture must still be inventoried");
    assert_eq!(entry.parsed_as, "binary");
    assert_eq!(entry.parse_status, skillmap_core::ParseStatus::Unsupported);

    assert!(
        manifest
            .unresolved
            .iter()
            .any(|u| { u.file == "assets/seal.bin" && u.reason == UnresolvedReason::BinaryFile }),
        "a file that could not be analyzed must say so; silence is invariant 3's failure mode"
    );
}

#[test]
fn malformed_frontmatter_is_reported_rather_than_guessed_at() {
    let manifest = manifest_for("malformed-frontmatter");
    let entry = manifest
        .unresolved
        .iter()
        .find(|u| u.file == "SKILL.md")
        .expect("a duplicate key must produce an unresolved entry");
    assert_eq!(entry.reason, UnresolvedReason::ParseError);
    assert!(entry
        .note
        .as_deref()
        .unwrap_or_default()
        .contains("duplicate key"));
    assert!(entry.start_line.is_some(), "the author needs a line number");

    // Falls back to the directory name rather than inventing one.
    assert_eq!(manifest.target.name, "malformed-frontmatter");
    assert_eq!(manifest.disclosure.description_bytes, 0);
}

#[test]
fn a_missing_frontmatter_block_is_reported() {
    let manifest = manifest_for("no-frontmatter");
    assert!(
        manifest
            .unresolved
            .iter()
            .any(|u| u.file == "SKILL.md" && u.reason == UnresolvedReason::ParseError),
        "a SKILL.md with no frontmatter has no session-start description, which is \
         a fact about the bundle worth stating"
    );
    assert_eq!(manifest.disclosure.description_bytes, 0);
    assert!(manifest.disclosure.trigger_terms.is_empty());
}

#[test]
fn content_digest_ignores_load_phase_but_tracks_bytes() {
    // The digest means "these bytes", nothing more. Two parses of unchanged
    // files must agree, and it must not be a function of classification.
    let first = manifest_for("minimal");
    let second = manifest_for("minimal");
    assert_eq!(first.target.content_digest, second.target.content_digest);

    let other = manifest_for("pdf-formatter");
    assert_ne!(first.target.content_digest, other.target.content_digest);
}

// ---------------------------------------------------------------------------
// Cases that need a scratch tree rather than a committed fixture
// ---------------------------------------------------------------------------

/// A scratch directory that cleans itself up.
struct TempDir(PathBuf);

impl TempDir {
    fn new(tag: &str) -> Self {
        let mut path = std::env::temp_dir();
        path.push(format!("skillmap-parse-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn write(path: &Path, contents: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(path, contents).unwrap();
}

/// A bundle directory with a valid SKILL.md, ready for extra files.
fn scratch_bundle(temp: &TempDir) -> PathBuf {
    let bundle = temp.path().join(".claude").join("skills").join("scratch");
    write(
        &bundle.join("SKILL.md"),
        "---\nname: scratch\ndescription: Scratch bundle.\n---\n\n# Scratch\n",
    );
    bundle
}

fn parse_scratch(temp: &TempDir, limits: &Limits) -> skillmap_core::Manifest {
    let bundles = discover(&ClaudeCode, temp.path(), Scope::Project)
        .unwrap()
        .bundles;
    let bundle = bundles.first().expect("scratch bundle must be discovered");
    parse_bundle(bundle, &ClaudeCode, limits).unwrap()
}

#[test]
fn an_oversized_file_is_hashed_but_not_analyzed() {
    let temp = TempDir::new("size");
    let bundle = scratch_bundle(&temp);
    write(&bundle.join("big.txt"), &"x".repeat(4096));

    let limits = Limits {
        max_file_bytes: 1024,
    };
    let manifest = parse_scratch(&temp, &limits);

    let entry = manifest
        .inventory
        .iter()
        .find(|e| e.path == "big.txt")
        .expect(
            "an oversized file must still be inventoried: omitting it would \
                 change the bundle's identity",
        );
    assert_eq!(entry.size, 4096);
    assert_eq!(entry.parse_status, skillmap_core::ParseStatus::Unsupported);

    assert!(
        manifest
            .unresolved
            .iter()
            .any(|u| u.file == "big.txt" && u.reason == UnresolvedReason::SizeLimit),
        "exceeding the limit must be stated, not silently applied"
    );
}

/// Create a symlink, or return `false` if this platform/user cannot.
///
/// Windows needs Developer Mode or elevation for symlink creation, so the test
/// that uses this skips rather than failing on a machine where the operation is
/// simply unavailable.
fn try_symlink(target: &Path, link: &Path) -> bool {
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(target, link).is_ok()
    }
    #[cfg(windows)]
    {
        if target.is_dir() {
            std::os::windows::fs::symlink_dir(target, link).is_ok()
        } else {
            std::os::windows::fs::symlink_file(target, link).is_ok()
        }
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = (target, link);
        false
    }
}

#[test]
fn a_symlink_escaping_the_bundle_is_reported_and_not_hashed() {
    let temp = TempDir::new("symlink");
    let bundle = scratch_bundle(&temp);

    // The target lives outside the bundle root entirely.
    let outside = temp.path().join("outside-secret.txt");
    write(&outside, "not part of this bundle\n");

    if !try_symlink(&outside, &bundle.join("linked.txt")) {
        eprintln!("skipping: this platform or user cannot create symlinks");
        return;
    }

    let manifest = parse_scratch(&temp, &Limits::default());

    assert!(
        !manifest.inventory.iter().any(|e| e.path == "linked.txt"),
        "a symlink out of the bundle must not be inventoried: hashing it would fold \
         somebody else's bytes into this bundle's identity"
    );
    assert!(
        manifest
            .unresolved
            .iter()
            .any(|u| u.file == "linked.txt" && u.reason == UnresolvedReason::SymlinkEscape),
        "the escape must be reported; unresolved was {:?}",
        manifest.unresolved
    );
}

#[test]
fn a_deeply_nested_tree_does_not_exhaust_the_stack() {
    // Directory nesting is attacker controlled, and a stack overflow aborts the
    // process rather than unwinding — it cannot be caught, so it is worse than a
    // panic, and invariant 10 exists because a crash on malformed input is a
    // denial of service on somebody's CI. The walk uses an explicit heap worklist
    // so depth costs heap, not stack.
    //
    // Honest about what this proves: it exercises the deep path and would fail on
    // a recursive walk given enough depth, but the depth at which recursion
    // actually overflows depends on the platform's thread stack size (2 MiB for
    // Rust test threads, 8 MiB for a Unix main thread) and on frame size. It is a
    // regression guard, not a demonstration of the exact limit. It is also the
    // slowest test here, and the cost is filesystem calls, not the walk.
    let temp = TempDir::new("deep");
    let bundle = scratch_bundle(&temp);

    let mut deep = bundle.clone();
    for level in 0..2_000 {
        deep = deep.join(format!("d{level}"));
    }
    if std::fs::create_dir_all(&deep).is_err() {
        // Some filesystems cap total path length well below this. The bug this
        // guards is real regardless; skipping beats a false failure.
        eprintln!("skipping: this filesystem will not create a 2000-deep path");
        return;
    }
    write(&deep.join("buried.txt"), "at the bottom\n");

    let manifest = parse_scratch(&temp, &Limits::default());
    assert!(
        manifest
            .inventory
            .iter()
            .any(|entry| entry.path.ends_with("buried.txt")),
        "the deepest file must still be inventoried"
    );
}

#[test]
fn a_bundle_with_no_skill_md_still_parses() {
    // `parse_path` scans a directory the user named, which may not be a bundle at
    // all. It must describe what it found rather than refusing or panicking.
    let temp = TempDir::new("noskill");
    let dir = temp.path().join("plain");
    write(&dir.join("notes.txt"), "just a directory\n");

    let manifest = skillmap_parse::parse_path(&dir, &ClaudeCode, &Limits::default()).unwrap();
    assert_eq!(manifest.target.name, "plain");
    assert!(manifest
        .unresolved
        .iter()
        .any(|u| u.file == "SKILL.md" && u.reason == UnresolvedReason::ParseError));
    assert_eq!(manifest.inventory.len(), 1);
}

#[test]
fn crlf_and_lf_checkouts_produce_the_same_digest() {
    // The specific failure .gitattributes exists to prevent, asserted directly:
    // the same logical bundle checked out with Windows line endings must hash
    // identically to one checked out with Unix line endings.
    let lf = TempDir::new("lf");
    let crlf = TempDir::new("crlf");

    let body = "---\nname: eol\ndescription: Line ending test.\n---\n\n# Body\n\nRun `run.sh`.\n";
    let script = "#!/bin/sh\necho one\necho two\n";

    for (temp, newline) in [(&lf, "\n"), (&crlf, "\r\n")] {
        let bundle = temp.path().join(".claude").join("skills").join("eol");
        write(&bundle.join("SKILL.md"), &body.replace('\n', newline));
        write(&bundle.join("run.sh"), &script.replace('\n', newline));
    }

    let lf_manifest = parse_scratch(&lf, &Limits::default());
    let crlf_manifest = parse_scratch(&crlf, &Limits::default());

    assert_eq!(
        lf_manifest.target.content_digest, crlf_manifest.target.content_digest,
        "a CRLF checkout must not change a bundle's identity"
    );
    assert_eq!(
        lf_manifest.to_canonical_json().unwrap(),
        crlf_manifest.to_canonical_json().unwrap()
    );
}
