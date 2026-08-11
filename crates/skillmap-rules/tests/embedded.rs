//! The embedded ruleset must be the ruleset contributors test against.
//!
//! T9 bakes `rules/` and `queries/` into the binary so a release can run without
//! a checkout beside it. That creates a way for the shipped tool to detect less
//! than the repository's own test suite does — a stale build, a walker that
//! misses a directory, a file that failed to embed — and the failure is silent
//! in the worst possible direction: fewer rules means a cleaner report.
//!
//! So the two sources are compared directly, on every build.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "a failed assertion in a test is the test failing, which is the point"
)]

use skillmap_rules::{embedded, load, Dir, Embedded, Source};
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

#[test]
fn the_embedded_rules_are_the_rules_on_disk() {
    let from_disk = load(&repo_root());
    let baked = embedded();

    let disk_ids: Vec<&str> = from_disk
        .rules
        .iter()
        .map(|rule| rule.id.as_str())
        .collect();
    let baked_ids: Vec<&str> = baked.rules.iter().map(|rule| rule.id.as_str()).collect();

    assert_eq!(
        disk_ids, baked_ids,
        "the binary would ship a different set of rules than this repository tests. \
         Fewer rules is a quieter report, which is why this is checked rather than assumed."
    );

    let disk_languages: Vec<&String> = from_disk.languages.keys().collect();
    let baked_languages: Vec<&String> = baked.languages.keys().collect();
    assert_eq!(disk_languages, baked_languages);
}

#[test]
fn both_sources_load_without_diagnostics() {
    // A diagnostic here means a rule did not load. On the embedded side that
    // would ship silently; `skillmap` prints diagnostics but nobody reads
    // stderr on a green build.
    assert!(
        load(&repo_root()).diagnostics.is_empty(),
        "{:?}",
        load(&repo_root()).diagnostics
    );
    assert!(
        embedded().diagnostics.is_empty(),
        "{:?}",
        embedded().diagnostics
    );
}

#[test]
fn every_file_the_disk_source_offers_is_embedded_byte_for_byte() {
    // Comparing loaded rule ids proves the two agree about which rules exist.
    // This proves they agree about what those rules *say* — a query that
    // embedded as an older revision would still compile, still load, and still
    // report a different set of findings.
    let disk = Dir::new(&repo_root());

    for path in disk.rule_files() {
        assert_eq!(
            disk.read(&path).unwrap(),
            Embedded
                .read(&path)
                .unwrap_or_else(|error| panic!("{path} is on disk but not embedded: {error}")),
            "{path} differs between the checkout and the binary"
        );
    }

    // And the query files, which `rule_files` does not enumerate because they
    // are referenced by rules rather than discovered.
    for query in query_paths() {
        assert_eq!(
            disk.read(&query).unwrap(),
            Embedded
                .read(&query)
                .unwrap_or_else(|error| panic!("{query} is not embedded: {error}")),
            "{query} differs between the checkout and the binary"
        );
    }
}

/// Every `.scm` under `queries/`, forward-slashed and repo-relative.
fn query_paths() -> Vec<String> {
    let root = repo_root();
    let mut found = Vec::new();
    let mut work = vec![root.join("queries")];

    while let Some(dir) = work.pop() {
        for entry in std::fs::read_dir(&dir).unwrap().flatten() {
            let path = entry.path();
            if path.is_dir() {
                work.push(path);
            } else if path.extension().is_some_and(|extension| extension == "scm") {
                found.push(
                    path.strip_prefix(&root)
                        .unwrap()
                        .to_str()
                        .unwrap()
                        .replace('\\', "/"),
                );
            }
        }
    }

    found.sort();
    assert!(!found.is_empty(), "there must be query files to compare");
    found
}
