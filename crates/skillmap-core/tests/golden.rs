//! The schema-drift gate.
//!
//! T1 requires that the Rust types and `schema/manifest-v1.schema.json` cannot
//! drift apart. The check is split across two runners on purpose:
//!
//! - **This test** proves the Rust types still render
//!   `tests/golden/manifest-maximal.json` byte for byte. Add a field, rename one,
//!   change a sort order, and it fails here.
//! - **`scripts/verify_spec.py --only golden-manifest`** proves that same file
//!   validates against the JSON Schema. Since the schema sets
//!   `additionalProperties: false`, a field added on the Rust side that the
//!   schema does not declare fails there.
//!
//! Together they close the loop in both directions. Doing the schema half in
//! Python is a deliberate dependency decision: the `jsonschema` crate pulls
//! `tokio`, `hyper`, and `reqwest` into the tree — an async HTTP stack, in the
//! dev-dependencies of a supply-chain auditor whose `SECURITY.md` promises a
//! minimal tree and no network. Python already owns the schema checks, and
//! already had the dependency.
//!
//! To re-bless after an intentional change: `SKILLMAP_BLESS=1 cargo test -p
//! skillmap-core --test golden`, then read the diff before committing it.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "a failed unwrap or panic in a test is the test failing. Invariant 10 \
              bans these in library code, where hostile input is the normal case; \
              a test binary has no CI to take down but its own."
)]

mod common;

use std::path::PathBuf;

fn golden_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/golden/manifest-maximal.json")
}

#[test]
fn maximal_manifest_matches_golden() {
    let rendered = common::maximal().to_canonical_json().unwrap();
    let path = golden_path();

    if std::env::var_os("SKILLMAP_BLESS").is_some() {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&path, rendered.as_bytes()).unwrap();
        return;
    }

    let expected = std::fs::read_to_string(&path).unwrap_or_else(|err| {
        panic!(
            "cannot read {}: {err}. Re-bless with SKILLMAP_BLESS=1.",
            path.display()
        )
    });

    assert_eq!(
        rendered, expected,
        "the canonical rendering changed. If that was intended, re-bless with \
         SKILLMAP_BLESS=1 and update schema/manifest-v1.schema.json in the same commit \
         — a manifest shape change is a schema-version event."
    );
}

#[test]
fn golden_file_is_stored_with_lf_endings() {
    // Read as bytes: a CRLF checkout would change this file's own SHA-256, which
    // is precisely the failure .gitattributes exists to prevent.
    let bytes = std::fs::read(golden_path()).unwrap_or_default();
    assert!(
        !bytes.windows(2).any(|w| w == b"\r\n"),
        "golden manifest must be stored with LF endings"
    );
}
