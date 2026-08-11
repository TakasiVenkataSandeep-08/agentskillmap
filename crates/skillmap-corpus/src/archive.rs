//! Content-addressed storage, and the ledger that stops a re-run re-fetching.
//!
//! `docs/01-corpus-scan.md`: *"Cache aggressively by digest — a re-run must not
//! re-fetch."* There is a subtlety in that sentence. The archive is keyed by the
//! bundle's `content_digest`, which is only known *after* the bytes arrive, so
//! the digest alone cannot prevent the fetch. The ledger closes that gap: it maps
//! `(owner, name, commit)` — all known before any transfer — to what that fetch
//! produced. A second run reads the ledger and never opens a socket.
//!
//! Pinning the commit is what makes this sound. Keyed on a branch name, the cache
//! would happily serve last month's contents as this month's.

use crate::{Error, RepoRef};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Ledger filename, kept inside `raw/` so one `.gitignore` rule covers every
/// unpublished corpus artifact.
const LEDGER: &str = "ledger.json";

/// What a previous fetch of one repository produced.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LedgerEntry {
    /// Bundle roots found, forward-slashed, each with its content digest.
    pub bundles: BTreeMap<String, String>,
    /// Set when the fetch succeeded but the repository held no bundle, so a
    /// re-run does not retry a repository already known to be empty.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub empty_reason: Option<String>,
}

/// The fetch ledger.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Ledger {
    /// Keyed by [`RepoRef::cache_key`], so a moved commit is a cache miss.
    pub entries: BTreeMap<String, LedgerEntry>,
}

/// The corpus directory layout.
#[derive(Debug, Clone)]
pub struct Archive {
    root: PathBuf,
}

impl Archive {
    /// Open (and create) the archive under `corpus_dir`.
    ///
    /// # Errors
    ///
    /// [`Error::Io`] if the directories cannot be created.
    pub fn open(corpus_dir: &Path) -> Result<Self, Error> {
        let root = corpus_dir.to_path_buf();
        std::fs::create_dir_all(root.join("raw")).map_err(|source| Error::Io {
            path: root.join("raw"),
            source,
        })?;
        Ok(Self { root })
    }

    /// The corpus root.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Where a bundle with `digest` is archived.
    ///
    /// The `sha256:` prefix is stripped: a colon is not a legal path character on
    /// Windows, and a corpus that cannot be checked out on one platform is not
    /// reproducible.
    #[must_use]
    pub fn bundle_dir(&self, digest: &str) -> PathBuf {
        let bare = digest.strip_prefix("sha256:").unwrap_or(digest);
        self.root.join("raw").join(bare)
    }

    /// Scratch directory for a clone, before its bundles are known.
    #[must_use]
    pub fn checkout_dir(&self, repo: &RepoRef) -> PathBuf {
        self.root
            .join("raw")
            .join(".checkout")
            .join(repo.cache_key())
    }

    /// Read the ledger, or an empty one.
    ///
    /// A corrupt ledger is treated as absent rather than fatal: the cost is
    /// re-fetching, which is slow but correct, whereas refusing to run would
    /// strand the operator with no way forward but deleting a file by hand.
    #[must_use]
    pub fn ledger(&self) -> Ledger {
        let path = self.root.join("raw").join(LEDGER);
        std::fs::read_to_string(&path)
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default()
    }

    /// Write the ledger.
    ///
    /// # Errors
    ///
    /// [`Error::Io`] if it cannot be written.
    pub fn write_ledger(&self, ledger: &Ledger) -> Result<(), Error> {
        let path = self.root.join("raw").join(LEDGER);
        let text = serde_json::to_string_pretty(ledger).map_err(|error| Error::Io {
            path: path.clone(),
            source: std::io::Error::other(error.to_string()),
        })?;
        std::fs::write(&path, format!("{text}\n")).map_err(|source| Error::Io { path, source })
    }

    /// Copy a bundle directory into the archive under its digest.
    ///
    /// Idempotent: an already-archived digest is left alone, since identical
    /// content is identical content and rewriting it would only risk a partial
    /// write over a good copy.
    ///
    /// # Errors
    ///
    /// [`Error::Io`] if the copy fails.
    pub fn store(&self, digest: &str, from: &Path) -> Result<PathBuf, Error> {
        let target = self.bundle_dir(digest);
        if target.is_dir() {
            return Ok(target);
        }
        copy_tree(from, &target)?;
        Ok(target)
    }

    /// Write `index.json` and `report.md`.
    ///
    /// # Errors
    ///
    /// [`Error::Io`] if either cannot be written.
    pub fn write_outputs(&self, index_json: &str, report_md: &str) -> Result<(), Error> {
        for (name, body) in [("index.json", index_json), ("report.md", report_md)] {
            let path = self.root.join(name);
            std::fs::write(&path, body).map_err(|source| Error::Io { path, source })?;
        }
        Ok(())
    }
}

/// Recursively copy `from` into `to`, skipping `.git`.
///
/// `.git` is excluded deliberately: it is most of the bytes, none of the
/// content, and it carries per-clone state that would make two archives of the
/// same commit differ.
fn copy_tree(from: &Path, to: &Path) -> Result<(), Error> {
    std::fs::create_dir_all(to).map_err(|source| Error::Io {
        path: to.to_path_buf(),
        source,
    })?;

    let entries = std::fs::read_dir(from).map_err(|source| Error::Io {
        path: from.to_path_buf(),
        source,
    })?;

    // An explicit worklist rather than recursion, for the same reason the parser
    // uses one: directory depth is attacker controlled, and a stack overflow
    // aborts rather than unwinds.
    let mut work: Vec<(PathBuf, PathBuf)> = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|source| Error::Io {
            path: from.to_path_buf(),
            source,
        })?;
        work.push((entry.path(), to.join(entry.file_name())));
    }

    while let Some((src, dst)) = work.pop() {
        if src.file_name().is_some_and(|name| name == ".git") {
            continue;
        }
        if src.is_dir() {
            std::fs::create_dir_all(&dst).map_err(|source| Error::Io {
                path: dst.clone(),
                source,
            })?;
            let children = std::fs::read_dir(&src).map_err(|source| Error::Io {
                path: src.clone(),
                source,
            })?;
            for child in children {
                let child = child.map_err(|source| Error::Io {
                    path: src.clone(),
                    source,
                })?;
                work.push((child.path(), dst.join(child.file_name())));
            }
        } else if src.is_file() {
            std::fs::copy(&src, &dst).map_err(|source| Error::Io {
                path: dst.clone(),
                source,
            })?;
        }
        // Anything else — a symlink, a device node — is not corpus content and
        // is not copied. The manifest for the bundle records it as unresolved.
    }
    Ok(())
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    reason = "a failed unwrap in a test is the test failing"
)]
mod tests {
    use super::*;

    fn temp(tag: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!("skillmap-archive-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        path
    }

    #[test]
    fn bundle_dir_strips_the_colon_windows_cannot_use() {
        let root = temp("colon");
        let archive = Archive::open(&root).unwrap();
        let dir = archive.bundle_dir("sha256:abc123");
        assert!(
            !dir.to_string_lossy().contains(':')
                || cfg!(windows) && dir.to_string_lossy().matches(':').count() == 1,
            "a digest colon must not reach the path: {dir:?}"
        );
        assert!(dir.ends_with("abc123"));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn ledger_round_trips_and_a_corrupt_one_reads_as_empty() {
        let root = temp("ledger");
        let archive = Archive::open(&root).unwrap();
        assert!(archive.ledger().entries.is_empty());

        let mut ledger = Ledger::default();
        ledger.entries.insert(
            "owner__repo__abc".to_owned(),
            LedgerEntry {
                bundles: BTreeMap::from([("skills/a".to_owned(), "sha256:aa".to_owned())]),
                empty_reason: None,
            },
        );
        archive.write_ledger(&ledger).unwrap();
        assert_eq!(archive.ledger().entries.len(), 1);

        std::fs::write(root.join("raw").join(LEDGER), "{ not json").unwrap();
        assert!(
            archive.ledger().entries.is_empty(),
            "a corrupt ledger must degrade to a re-fetch, not strand the operator"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn store_skips_git_and_is_idempotent() {
        let root = temp("store");
        let archive = Archive::open(&root).unwrap();

        let src = root.join("src");
        std::fs::create_dir_all(src.join(".git")).unwrap();
        std::fs::create_dir_all(src.join("nested")).unwrap();
        std::fs::write(src.join(".git").join("HEAD"), "ref: refs/heads/main").unwrap();
        std::fs::write(src.join("SKILL.md"), "---\nname: a\n---\n").unwrap();
        std::fs::write(src.join("nested").join("x.py"), "pass\n").unwrap();

        let stored = archive.store("sha256:deadbeef", &src).unwrap();
        assert!(stored.join("SKILL.md").is_file());
        assert!(stored.join("nested").join("x.py").is_file());
        assert!(
            !stored.join(".git").exists(),
            ".git is most of the bytes and none of the content"
        );

        // Idempotent: a second store over the same digest must not fail.
        archive.store("sha256:deadbeef", &src).unwrap();
        let _ = std::fs::remove_dir_all(&root);
    }
}
