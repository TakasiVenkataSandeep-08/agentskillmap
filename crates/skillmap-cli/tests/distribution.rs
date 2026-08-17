//! T9's acceptance criteria that a test can hold.
//!
//! The headline one — *"two builds of the same tag from clean checkouts are
//! byte-identical"* — is a property of the linker and lives in
//! `.github/workflows/release.yml`, because it needs two checkouts and two full
//! release builds. What belongs here is everything about the shipped binary
//! being *self-contained*: a release has no repository beside it, and until T9
//! this tool could not run without one.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "a failed unwrap in a test is the test failing"
)]

use std::path::PathBuf;
use std::process::{Command, Output};

/// Run the binary from a directory that is **not** the repository.
///
/// The whole point. Running from the checkout would pass even if `--rules` were
/// still required, because the rules would be underfoot.
fn skillmap_from_elsewhere(args: &[&str]) -> Output {
    let elsewhere = std::env::temp_dir().join(format!("skillmap-t9-{}", std::process::id()));
    std::fs::create_dir_all(&elsewhere).unwrap();

    Command::new(env!("CARGO_BIN_EXE_skillmap"))
        .args(args)
        .current_dir(&elsewhere)
        .output()
        .unwrap()
}

fn fixture(version: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("fixtures")
        .join("projects")
        .join(version)
        .canonicalize()
        .unwrap()
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

#[test]
fn the_binary_detects_without_a_checkout_beside_it() {
    let output =
        skillmap_from_elsewhere(&["scan", "--project", &fixture("v1.1").display().to_string()]);

    assert!(
        output.status.success(),
        "scan failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let manifest = stdout(&output);
    assert!(
        manifest.contains("fs.read.credential"),
        "the binary found nothing, which is what a binary with no rules looks \
         like — and it looks like good news:\n{manifest}"
    );
    assert!(
        manifest.contains("py.credential-read.dotfile"),
        "{manifest}"
    );
}

#[test]
fn scan_emits_canonical_json() {
    // `scan` exists so the manifest is inspectable, and a manifest that is not
    // canonical is not comparable between two machines (invariant 2). Checking
    // the framing here is cheap; the ordering rules have their own tests in
    // skillmap-core.
    let output =
        skillmap_from_elsewhere(&["scan", "--project", &fixture("v1.0").display().to_string()]);
    let json = stdout(&output);

    assert!(json.ends_with('\n'), "trailing newline");
    assert!(!json.contains('\r'), "LF only");
    // An ARRAY, not a bare object. `scan` used to print one object per bundle,
    // concatenated, which parses for one skill and is two top-level objects for
    // two — so the only machine-readable output the tool has stopped being
    // machine-readable at exactly the point a project became real.
    assert!(
        json.starts_with("[\n  {\n"),
        "an array, two-space indent: {json}"
    );
    let document: serde_json::Value = serde_json::from_str(&json).unwrap();
    let manifests = document.as_array().expect("scan emits an array");
    assert_eq!(manifests.len(), 1, "this fixture has one skill");
    let parsed = manifests.first().expect("one manifest");
    // Compared against the constant rather than a literal. This carried "1.0.0"
    // and had to be edited when the taxonomy shrank — which is a test failing
    // because it duplicated a value, not because anything was wrong. The point
    // here is that the shipped binary stamps the version this crate declares.
    assert_eq!(
        parsed
            .get("schema_version")
            .and_then(serde_json::Value::as_str),
        Some(skillmap_core::SCHEMA_VERSION)
    );
}

#[test]
fn rules_lists_what_this_build_can_detect() {
    // The rules are no longer on disk beside the tool, so the binary has to be
    // able to answer "which rule produced this finding, and what does it claim".
    let output = skillmap_from_elsewhere(&["rules"]);
    let listing = stdout(&output);

    assert!(output.status.success(), "{listing}");
    assert!(listing.contains("py.credential-read.dotfile"), "{listing}");
    assert!(listing.contains("fs.read.credential"), "{listing}");
    assert!(
        listing.contains(env!("CARGO_PKG_VERSION")),
        "the listing must say which build it describes:\n{listing}"
    );
}

#[test]
fn version_is_the_crate_version() {
    // The release workflow refuses to publish when the tag and Cargo.toml
    // disagree; this is the other end of that check.
    let output = skillmap_from_elsewhere(&["version"]);
    assert_eq!(
        stdout(&output).trim(),
        format!("skillmap {}", env!("CARGO_PKG_VERSION"))
    );
}

#[test]
fn an_unknown_subcommand_exits_four_rather_than_doing_nothing_quietly() {
    let output = skillmap_from_elsewhere(&["scna", "--project", "."]);
    assert_eq!(output.status.code(), Some(4));
}
