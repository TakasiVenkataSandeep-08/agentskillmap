//! Where rule data comes from.
//!
//! Two answers, and the split exists because of T9. A checkout has `rules/` and
//! `queries/` on disk; a released binary has neither, and until this abstraction
//! existed `skillmap ci` needed `--rules` pointing at a clone — which is not a
//! distributable tool.
//!
//! Both sources hand back the same bytes, so the loader below has exactly one
//! code path and there is no way for the embedded ruleset to drift into
//! behaving differently from the one contributors test against.
//! `crates/skillmap-rules/tests/embedded.rs` asserts the two agree.

use std::path::{Path, PathBuf};

// The generated table: `&[(repo_relative_path, contents)]`, sorted.
include!(concat!(env!("OUT_DIR"), "/embedded.rs"));

/// A place rule data can be read from.
pub trait Source {
    /// Repo-relative paths of every rule file, sorted, excluding
    /// `rules/languages.toml`.
    fn rule_files(&self) -> Vec<String>;

    /// Read a repo-relative path, forward-slashed.
    ///
    /// # Errors
    ///
    /// A human-readable reason, which becomes a diagnostic note.
    fn read(&self, path: &str) -> Result<String, String>;

    /// Where this data came from, for a human reading a diagnostic.
    fn origin(&self) -> String;
}

/// Rules read from a checkout.
#[derive(Debug, Clone)]
pub struct Dir {
    root: PathBuf,
}

impl Dir {
    /// Read `<root>/rules` and `<root>/queries`.
    #[must_use]
    pub fn new(root: &Path) -> Self {
        Self {
            root: root.to_path_buf(),
        }
    }
}

impl Source for Dir {
    fn rule_files(&self) -> Vec<String> {
        let rules = self.root.join("rules");
        let mut found = Vec::new();
        let mut work = vec![rules];

        while let Some(current) = work.pop() {
            let Ok(entries) = std::fs::read_dir(&current) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    work.push(path);
                    continue;
                }
                if path
                    .extension()
                    .is_some_and(|extension| extension == "toml")
                    && path
                        .file_name()
                        .is_some_and(|name| name != "languages.toml")
                {
                    if let Some(relative) = path
                        .strip_prefix(&self.root)
                        .ok()
                        .and_then(|relative| relative.to_str())
                    {
                        found.push(relative.replace('\\', "/"));
                    }
                }
            }
        }

        found.sort();
        found
    }

    fn read(&self, path: &str) -> Result<String, String> {
        let mut full = self.root.clone();
        for segment in path.split('/') {
            full.push(segment);
        }
        std::fs::read_to_string(&full).map_err(|error| format!("cannot read: {error}"))
    }

    fn origin(&self) -> String {
        self.root.join("rules").display().to_string()
    }
}

/// Rules baked into the binary at build time.
///
/// This is what a released `skillmap` uses. See `crates/skillmap-rules/build.rs`
/// for why the data is embedded as literals rather than `include_str!`.
#[derive(Debug, Clone, Copy, Default)]
pub struct Embedded;

impl Source for Embedded {
    fn rule_files(&self) -> Vec<String> {
        // Already sorted by the build script, which sorts so that two machines
        // whose filesystems enumerate directories differently still produce the
        // same table.
        FILES
            .iter()
            .map(|(path, _)| *path)
            .filter(|path| path.starts_with("rules/") && path.ends_with(".toml"))
            .filter(|path| *path != "rules/languages.toml")
            .map(str::to_owned)
            .collect()
    }

    fn read(&self, path: &str) -> Result<String, String> {
        FILES
            .iter()
            .find(|(candidate, _)| *candidate == path)
            .map(|(_, text)| (*text).to_owned())
            .ok_or_else(|| "not embedded in this binary".to_owned())
    }

    fn origin(&self) -> String {
        "embedded in the binary".to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_embedded_tree_is_not_empty() {
        // A build that embedded nothing produces a scanner that reports every
        // project clean. build.rs refuses to emit one; this catches the case
        // where it emitted a table that is somehow empty anyway.
        assert!(!FILES.is_empty());
        assert!(!Embedded.rule_files().is_empty());
        assert!(Embedded.read("rules/languages.toml").is_ok());
    }

    #[test]
    fn the_embedded_table_is_sorted_and_unique() {
        // Order here becomes rule load order, and duplicate paths would make
        // `read` return whichever came first.
        let paths: Vec<&str> = FILES.iter().map(|(path, _)| *path).collect();
        let mut sorted = paths.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(paths, sorted);
    }

    #[test]
    fn languages_toml_is_not_offered_as_a_rule() {
        // It configures languages; loading it as a rule would produce a
        // diagnostic on every single run.
        assert!(!Embedded
            .rule_files()
            .iter()
            .any(|path| path == "rules/languages.toml"));
    }

    #[test]
    fn a_path_that_was_never_embedded_is_an_error_not_an_empty_string() {
        // Invariant 3: a missing query must become a diagnostic, never a rule
        // that silently compiles against nothing.
        assert!(Embedded.read("queries/ruby/credential-read.scm").is_err());
    }
}
