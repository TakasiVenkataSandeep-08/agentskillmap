//! The harvest pipeline, end to end, without a network.
//!
//! Everything except the two GitHub adapters is exercised here: fetch caching,
//! bundle discovery inside a repository, deduplication by content digest,
//! measurement, the index, and the report. The [`Fetcher`] trait exists so this
//! is possible — the local implementation below stands in for `git clone`, and
//! nothing downstream of it knows the difference.
//!
//! What is deliberately *not* covered: `github.rs`. Testing it would mean either
//! hitting the real API (non-deterministic, rate-limited, and a network call in
//! `cargo test`) or building a mock HTTP server, which tests the mock. Its
//! surface is two GETs and four `git` invocations, and it is the one part a
//! reviewer must read rather than trust.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "a failed assertion in a test is the test failing, which is the point"
)]

use skillmap_corpus::{archive::Archive, report, Error, Fetcher, Provenance, RepoRef};
use std::cell::RefCell;
use std::path::{Path, PathBuf};

/// A scratch directory that cleans itself up.
struct TempDir(PathBuf);

impl TempDir {
    fn new(tag: &str) -> Self {
        let mut path = std::env::temp_dir();
        path.push(format!("skillmap-corpus-{tag}-{}", std::process::id()));
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

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("fixtures")
        .join("bundles")
}

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

/// Stands in for `git clone`, laying out fixture bundles as a repository would.
///
/// Counts its calls, which is how the caching test proves a re-run never reaches
/// for the network.
struct LocalFetcher {
    /// Which fixture bundles each repo slug contains, and at what path.
    layout: Vec<(&'static str, &'static str)>,
    calls: RefCell<usize>,
}

impl Fetcher for LocalFetcher {
    fn fetch(&self, _repo: &RepoRef, into: &Path) -> Result<(), Error> {
        *self.calls.borrow_mut() += 1;
        std::fs::create_dir_all(into).unwrap();
        // A repository is not a bundle: bundles sit somewhere inside it, which is
        // what `find_bundles` has to cope with.
        for (fixture, at) in &self.layout {
            let mut target = into.to_path_buf();
            for segment in at.split('/') {
                target.push(segment);
            }
            copy_tree(&fixtures().join(fixture), &target);
        }
        // Repositories carry a .git directory; the archive must not.
        std::fs::create_dir_all(into.join(".git")).unwrap();
        std::fs::write(into.join(".git").join("HEAD"), "ref: refs/heads/main").unwrap();
        Ok(())
    }
}

fn repo(owner: &str, name: &str, provenance: Provenance) -> RepoRef {
    RepoRef {
        owner: owner.to_owned(),
        name: name.to_owned(),
        commit: "0123456789abcdef0123456789abcdef01234567".to_owned(),
        provenance,
        stars: Some(42),
    }
}

fn fetcher() -> LocalFetcher {
    LocalFetcher {
        layout: vec![
            ("pdf-formatter", ".claude/skills/pdf-formatter"),
            ("minimal", ".claude/skills/minimal"),
        ],
        calls: RefCell::new(0),
    }
}

#[test]
fn harvests_measures_and_indexes() {
    let temp = TempDir::new("harvest");
    let archive = Archive::open(temp.path()).unwrap();
    let repos = vec![repo("acme", "skills", Provenance::CuratedList)];

    let index = report::harvest(&repos, &fetcher(), &archive, "test").unwrap();

    assert_eq!(
        index.records.len(),
        2,
        "both bundles in the repo are indexed"
    );
    assert_eq!(index.snapshot, "test");

    let pdf = index
        .records
        .iter()
        .find(|record| record.bundle_root.ends_with("pdf-formatter"))
        .expect("the pdf-formatter bundle must be indexed");

    assert_eq!(pdf.repo, "acme/skills");
    assert!(pdf.bundle_root.starts_with(".claude/skills/"));
    assert!(pdf.digest.starts_with("sha256:"));

    // Structural facts are exact and must match what the parser said.
    assert!(pdf.measurements.structure.has_scripts);
    assert!(pdf.measurements.structure.has_unreferenced);
    assert!(pdf.measurements.governance.frontmatter_parsed);

    // Lexical: exfil.py mentions ~/.aws and urllib, and nothing points at it.
    assert!(pdf.measurements.lexical.credential_paths);
    assert!(pdf.measurements.lexical.network);
    assert!(
        pdf.measurements
            .lexical
            .only_in_unreferenced
            .contains(&"credential_paths".to_owned()),
        "the credential mention exists only in a file nothing references: {:?}",
        pdf.measurements.lexical.only_in_unreferenced
    );
}

#[test]
fn a_rerun_does_not_refetch() {
    // `docs/01-corpus-scan.md`: "a re-run must not re-fetch". The cache key is
    // the pinned commit, so this only holds because the commit is pinned.
    let temp = TempDir::new("cache");
    let archive = Archive::open(temp.path()).unwrap();
    let repos = vec![repo("acme", "skills", Provenance::CuratedList)];

    let first = fetcher();
    let index_a = report::harvest(&repos, &first, &archive, "test").unwrap();
    assert_eq!(*first.calls.borrow(), 1);

    let second = fetcher();
    let index_b = report::harvest(&repos, &second, &archive, "test").unwrap();
    assert_eq!(
        *second.calls.borrow(),
        0,
        "the second run must be served entirely from the ledger"
    );

    assert_eq!(
        report::index_json(&index_a).unwrap(),
        report::index_json(&index_b).unwrap(),
        "a cached re-run must produce an identical index"
    );
}

#[test]
fn a_moved_commit_is_a_cache_miss() {
    let temp = TempDir::new("moved");
    let archive = Archive::open(temp.path()).unwrap();

    let first = fetcher();
    report::harvest(
        &[repo("acme", "skills", Provenance::CuratedList)],
        &first,
        &archive,
        "test",
    )
    .unwrap();
    assert_eq!(*first.calls.borrow(), 1);

    let mut moved = repo("acme", "skills", Provenance::CuratedList);
    moved.commit = "ffffffffffffffffffffffffffffffffffffffff".to_owned();
    let second = fetcher();
    report::harvest(&[moved], &second, &archive, "test").unwrap();
    assert_eq!(
        *second.calls.borrow(),
        1,
        "a different commit is different content and must be fetched"
    );
}

#[test]
fn the_same_bundle_in_two_repos_is_counted_once() {
    // Otherwise the base rates over-count whatever is most vendored, which is
    // exactly the popular material — the bias hardest to notice.
    let temp = TempDir::new("dedup");
    let archive = Archive::open(temp.path()).unwrap();
    let repos = vec![
        repo("acme", "skills", Provenance::CuratedList),
        repo("other", "skills", Provenance::CodeSearch),
    ];

    let index = report::harvest(&repos, &fetcher(), &archive, "test").unwrap();
    assert_eq!(
        index.records.len(),
        2,
        "two identical repos contribute two distinct bundles, not four"
    );

    let mut digests: Vec<&str> = index
        .records
        .iter()
        .map(|record| record.digest.as_str())
        .collect();
    let before = digests.len();
    digests.sort_unstable();
    digests.dedup();
    assert_eq!(digests.len(), before, "digests must already be unique");
}

#[test]
fn the_archive_excludes_git_and_is_content_addressed() {
    let temp = TempDir::new("archive");
    let archive = Archive::open(temp.path()).unwrap();
    let index = report::harvest(
        &[repo("acme", "skills", Provenance::CuratedList)],
        &fetcher(),
        &archive,
        "test",
    )
    .unwrap();

    for record in &index.records {
        let dir = archive.bundle_dir(&record.digest);
        assert!(
            dir.join("SKILL.md").is_file(),
            "{dir:?} must hold the bundle"
        );
        assert!(
            !dir.join(".git").exists(),
            "the archive must not carry .git"
        );
        assert!(
            !dir.to_string_lossy().contains("sha256:"),
            "a colon in a path is not portable to Windows"
        );
    }
}

#[test]
fn a_repository_with_no_bundle_is_recorded_not_dropped() {
    let temp = TempDir::new("empty");
    let archive = Archive::open(temp.path()).unwrap();

    struct EmptyRepo;
    impl Fetcher for EmptyRepo {
        fn fetch(&self, _repo: &RepoRef, into: &Path) -> Result<(), Error> {
            std::fs::create_dir_all(into.join("src")).unwrap();
            std::fs::write(into.join("README.md"), "no skills here\n").unwrap();
            Ok(())
        }
    }

    let index = report::harvest(
        &[repo("acme", "not-skills", Provenance::CodeSearch)],
        &EmptyRepo,
        &archive,
        "test",
    )
    .unwrap();

    assert!(index.records.is_empty());
    assert_eq!(index.skipped.len(), 1);
    assert!(index.skipped[0].reason.contains("no SKILL.md"));
}

#[test]
fn a_failed_fetch_does_not_abort_the_harvest() {
    // A run can cost thousands of API calls. One unreachable repository must not
    // discard the rest of it.
    let temp = TempDir::new("partial");
    let archive = Archive::open(temp.path()).unwrap();

    struct FailsOne;
    impl Fetcher for FailsOne {
        fn fetch(&self, repo: &RepoRef, into: &Path) -> Result<(), Error> {
            if repo.name == "broken" {
                return Err(Error::Git {
                    context: repo.slug(),
                    message: "simulated clone failure".to_owned(),
                });
            }
            fetcher().fetch(repo, into)
        }
    }

    let index = report::harvest(
        &[
            repo("acme", "broken", Provenance::CodeSearch),
            repo("acme", "skills", Provenance::CuratedList),
        ],
        &FailsOne,
        &archive,
        "test",
    )
    .unwrap();

    assert_eq!(
        index.records.len(),
        2,
        "the good repository is still harvested"
    );
    assert_eq!(index.skipped.len(), 1);
    assert!(index.skipped[0].reason.contains("fetch failed"));
}

#[test]
fn the_index_is_canonical_json() {
    let temp = TempDir::new("canonical");
    let archive = Archive::open(temp.path()).unwrap();
    let index = report::harvest(
        &[repo("acme", "skills", Provenance::CuratedList)],
        &fetcher(),
        &archive,
        "test",
    )
    .unwrap();

    let json = report::index_json(&index).unwrap();
    assert!(json.ends_with("}\n"));
    assert!(!json.contains('\r'));

    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    let recanonical = serde_json::to_string_pretty(&parsed).unwrap() + "\n";
    assert_eq!(
        json, recanonical,
        "sorted keys, two-space indent, one newline"
    );

    // No floats: every rate in this project is integer arithmetic.
    assert!(
        !regex_free_float_scan(&json),
        "the index must contain no floating-point number"
    );
}

/// Detect a JSON float without pulling in a regex crate.
fn regex_free_float_scan(text: &str) -> bool {
    text.split(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-'))
        .any(|token| token.contains('.') && token.chars().any(|c| c.is_ascii_digit()))
}

#[test]
fn the_report_separates_head_from_tail() {
    let temp = TempDir::new("report");
    let archive = Archive::open(temp.path()).unwrap();
    let index = report::harvest(
        &[
            repo("curated", "skills", Provenance::CuratedList),
            repo("found", "skills", Provenance::CodeSearch),
        ],
        &fetcher(),
        &archive,
        "2026-08",
    )
    .unwrap();

    let text = report::report(&index);

    assert!(text.contains("snapshot `2026-08`"));
    // Bias before findings, not in a footnote.
    let bias = text.find("what these numbers do not mean").unwrap();
    assert!(bias < text.find("## Structure").unwrap());
    // Head and tail are named as separate populations.
    assert!(text.contains("| Head | Tail | All |"));
    assert!(text.contains("only code search reaches the tail"));
    // Lexical numbers are labelled as upper bounds.
    assert!(text.contains("upper bound"));
    // Every rate carries its denominator.
    assert!(text.contains('/') && text.contains('%'));
    // The kill gate is stated as the point of the exercise.
    assert!(text.contains("kill gate"));
    // No wall-clock timestamp: the report must be reproducible from its inputs.
    assert!(!text.contains("Generated at"));
}

#[test]
fn a_missing_token_fails_before_any_work() {
    // Only meaningful when the variable is genuinely absent; a developer machine
    // may legitimately have one exported.
    if std::env::var_os("GITHUB_TOKEN").is_some() {
        eprintln!("skipping: GITHUB_TOKEN is set in this environment");
        return;
    }
    let error = skillmap_corpus::github_token().unwrap_err();
    let message = error.to_string();
    assert!(message.contains("GITHUB_TOKEN"));
    assert!(
        message.contains("60 requests/hour"),
        "the message must explain why a token is required, not just that it is"
    );
}
