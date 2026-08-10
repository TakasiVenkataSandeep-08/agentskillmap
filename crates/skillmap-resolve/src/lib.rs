#![warn(missing_docs)]

//! Bundle discovery: one parser, many resolvers.
//!
//! `SKILL.md` is one standard across agents, and the **parser is ~90% shared**.
//! What differs between Claude Code, Cursor, Codex, Windsurf and the rest is
//! *discovery*: which directories hold skills, project scope versus user scope,
//! how plugin wrappers nest. Forking the parser per agent would be the obvious
//! mistake; this crate exists so that adding an agent is a [`Resolver`] impl of
//! roughly thirty lines and nothing else.
//!
//! That is also the hedge described in `ARCHITECTURE.md`: no single registry
//! will ever write policy spanning every agent, so being cross-agent from commit
//! one is the durable position.

use skillmap_core::BundleKind;
use std::path::{Component, Path, PathBuf};

/// Where a bundle was found, which decides which discovery roots apply.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Scope {
    /// Installed into a specific project, e.g. `<project>/.claude/skills`.
    Project,
    /// Installed for the user across all projects, e.g. `~/.claude/skills`.
    User,
}

impl Scope {
    /// Every scope, in a fixed order.
    pub const ALL: &'static [Self] = &[Self::Project, Self::User];
}

/// Per-agent discovery conventions.
///
/// Implementations describe *where* an agent keeps skills and *what counts* as a
/// bundle. They never parse bundle contents — that is `skillmap-parse`'s job,
/// and keeping the split sharp is what stops per-agent quirks leaking into the
/// parser.
pub trait Resolver {
    /// Stable identifier, e.g. `claude-code`. Appears in manifest provenance, so
    /// changing one is a breaking change for every `skillmap.lock` in the wild.
    fn id(&self) -> &'static str;

    /// Candidate discovery roots for this scope, relative to a project root
    /// (for [`Scope::Project`]) or a home directory (for [`Scope::User`]).
    ///
    /// Returned in a fixed order; discovery does not sort them, because for some
    /// agents precedence between roots is meaningful.
    fn search_paths(&self, scope: Scope) -> Vec<PathBuf>;

    /// Recognise a directory as a bundle root.
    ///
    /// `dir` is an existing directory directly beneath a discovery root.
    /// Returning `None` means "not a bundle", not "error".
    fn classify(&self, dir: &Path) -> Option<BundleKind>;

    /// Frontmatter keys carrying capabilities the author declared, if the agent
    /// defines any.
    ///
    /// Values read from these keys land in `disclosure.declared_capabilities`
    /// **verbatim** — they are third-party prose in the author's vocabulary, and
    /// mapping them into our closed taxonomy is a separate, explicitly lossy step.
    fn declared_capability_keys(&self) -> &'static [&'static str] {
        &[]
    }
}

/// A discovered bundle, before anything has been read out of it.
///
/// Ordering is deliberately not derived. A derived `Ord` would compare `kind`,
/// making the order bundles are discovered in depend on the declaration order of
/// [`BundleKind`]'s variants — the same class of hazard the manifest
/// canonicalizer avoids. [`sort_key`](BundleRef::sort_key) keys on strings only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BundleRef {
    /// Id of the resolver that found it.
    pub resolver: &'static str,
    /// What kind of bundle it is.
    pub kind: BundleKind,
    /// Directory name of the bundle. `skillmap-parse` may prefer the
    /// frontmatter `name` over this for `target.name`; `root` always keeps the
    /// on-disk truth.
    pub name: String,
    /// The discovery root this bundle sits beneath, as an absolute path.
    ///
    /// **Never enters the manifest.** It is machine-specific — that is exactly
    /// why `target.root` is stored relative to it (invariant 2).
    pub discovery_root: PathBuf,
    /// Bundle root relative to [`BundleRef::discovery_root`], forward-slashed.
    /// This is the value that becomes `target.root`.
    pub root: String,
}

impl BundleRef {
    /// A total order over stable string fields.
    ///
    /// `(resolver, root, name)` — no two bundles can share a resolver and a root,
    /// so this is total, and none of the three can be reordered by editing an
    /// enum. `discovery_root` is excluded because it is machine-specific and
    /// sorting on it would make discovery order differ between checkouts.
    pub fn sort_key(&self) -> (&str, &str, &str) {
        (self.resolver, self.root.as_str(), self.name.as_str())
    }

    /// The bundle's absolute path on this machine.
    pub fn path(&self) -> PathBuf {
        let mut path = self.discovery_root.clone();
        for segment in self.root.split('/') {
            path.push(segment);
        }
        path
    }
}

/// Claude Code's conventions.
///
/// Skills live in `.claude/skills/<name>/SKILL.md`, in the project or under the
/// user's home directory.
#[derive(Debug, Clone, Copy, Default)]
pub struct ClaudeCode;

impl Resolver for ClaudeCode {
    fn id(&self) -> &'static str {
        "claude-code"
    }

    fn search_paths(&self, scope: Scope) -> Vec<PathBuf> {
        // Identical for both scopes; the difference is the base the caller joins
        // them onto — project root versus home directory.
        match scope {
            Scope::Project | Scope::User => vec![PathBuf::from(".claude").join("skills")],
        }
    }

    fn classify(&self, dir: &Path) -> Option<BundleKind> {
        // A bundle is a directory holding a SKILL.md. Nothing more is claimed:
        // `BundleKind::Plugin` exists in the schema because the manifest format
        // has to describe plugin-wrapped skills, but this resolver does not yet
        // walk `.claude/plugins`, and returning Plugin from a code path that
        // cannot actually produce one would be a stub (invariant 12). Tracked in
        // docs/00-tasks.md.
        dir.join("SKILL.md").is_file().then_some(BundleKind::Skill)
    }
}

/// Everything that can go wrong discovering bundles.
#[derive(Debug)]
pub enum DiscoverError {
    /// A discovery root exists but could not be listed.
    ReadDir {
        /// The directory that could not be read.
        path: PathBuf,
        /// Why.
        source: std::io::Error,
    },
}

impl std::fmt::Display for DiscoverError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ReadDir { path, source } => {
                write!(f, "cannot list discovery root {}: {source}", path.display())
            }
        }
    }
}

impl std::error::Error for DiscoverError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ReadDir { source, .. } => Some(source),
        }
    }
}

/// Find every bundle `resolver` recognises beneath `base` for `scope`.
///
/// `base` is the project root for [`Scope::Project`] and the user's home
/// directory for [`Scope::User`]. A discovery root that does not exist is not an
/// error — most projects have no `.claude/skills` — but one that exists and
/// cannot be read is, because silently returning zero bundles there would look
/// exactly like a clean scan (invariant 3).
///
/// Results are sorted, so two machines whose filesystems enumerate directories
/// in different orders still produce the same sequence (invariant 2).
///
/// # Errors
///
/// [`DiscoverError::ReadDir`] if an existing discovery root cannot be listed.
pub fn discover(
    resolver: &dyn Resolver,
    base: &Path,
    scope: Scope,
) -> Result<Vec<BundleRef>, DiscoverError> {
    let mut found = Vec::new();

    for relative in resolver.search_paths(scope) {
        let discovery_root = base.join(&relative);
        if !discovery_root.is_dir() {
            continue;
        }

        let entries =
            std::fs::read_dir(&discovery_root).map_err(|source| DiscoverError::ReadDir {
                path: discovery_root.clone(),
                source,
            })?;

        // Collect names first so a mid-iteration IO error is a real error rather
        // than a truncated listing that reads as "no more bundles".
        let mut names: Vec<String> = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|source| DiscoverError::ReadDir {
                path: discovery_root.clone(),
                source,
            })?;
            if !entry.path().is_dir() {
                continue;
            }
            // A directory name that is not valid UTF-8 cannot round-trip through
            // a JSON manifest, so it is skipped rather than lossily converted
            // into a path that points nowhere.
            if let Some(name) = entry.file_name().to_str() {
                names.push(name.to_owned());
            }
        }
        names.sort_by(|a, b| a.as_bytes().cmp(b.as_bytes()));

        for name in names {
            let dir = discovery_root.join(&name);
            if let Some(kind) = resolver.classify(&dir) {
                found.push(BundleRef {
                    resolver: resolver.id(),
                    kind,
                    name: name.clone(),
                    discovery_root: discovery_root.clone(),
                    root: name,
                });
            }
        }
    }

    found.sort_by(|a, b| a.sort_key().cmp(&b.sort_key()));
    Ok(found)
}

/// Render `path` relative to `base` as a forward-slash string.
///
/// Returns `None` if `path` is not beneath `base`, or if any component is not
/// valid UTF-8. Used for every path that reaches the manifest: backslashes on
/// Windows would make the same bundle hash differently there than on Linux,
/// which is invariant 2's most obvious failure mode.
pub fn relative_slash_path(base: &Path, path: &Path) -> Option<String> {
    let relative = path.strip_prefix(base).ok()?;
    let mut segments = Vec::new();
    for component in relative.components() {
        match component {
            Component::Normal(segment) => segments.push(segment.to_str()?),
            // `.` contributes nothing; anything else (`..`, a root, a Windows
            // drive prefix) means this is not a plain descendant path and must
            // not be reported as one.
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    Some(segments.join("/"))
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "a failed unwrap or out-of-bounds index in a test is the test failing, \
              which is the point. Invariant 10 bans these in library code, where \
              hostile input is the normal case and a crash is a DoS on somebody's CI."
)]
mod tests {
    use super::*;

    fn write(path: &Path, contents: &str) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, contents).unwrap();
    }

    /// A scratch directory that cleans itself up.
    struct TempDir(PathBuf);

    impl TempDir {
        fn new(tag: &str) -> Self {
            let mut path = std::env::temp_dir();
            path.push(format!("skillmap-resolve-{tag}-{}", std::process::id()));
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

    #[test]
    fn finds_skills_and_ignores_non_bundles() {
        let temp = TempDir::new("discover");
        let skills = temp.path().join(".claude").join("skills");
        write(
            &skills.join("zebra").join("SKILL.md"),
            "---\nname: zebra\n---\n",
        );
        write(
            &skills.join("alpha").join("SKILL.md"),
            "---\nname: alpha\n---\n",
        );
        // A directory with no SKILL.md is not a bundle.
        std::fs::create_dir_all(skills.join("not-a-skill")).unwrap();
        // A stray file at the discovery root is not a bundle either.
        write(&skills.join("README.md"), "hello\n");

        let found = discover(&ClaudeCode, temp.path(), Scope::Project).unwrap();
        let names: Vec<&str> = found.iter().map(|b| b.name.as_str()).collect();
        assert_eq!(names, ["alpha", "zebra"], "results must be sorted");
        assert_eq!(found[0].resolver, "claude-code");
        assert_eq!(found[0].kind, skillmap_core::BundleKind::Skill);
        assert_eq!(found[0].root, "alpha");
        assert!(found[0].path().join("SKILL.md").is_file());
    }

    #[test]
    fn a_missing_discovery_root_is_not_an_error() {
        let temp = TempDir::new("missing");
        assert!(discover(&ClaudeCode, temp.path(), Scope::Project)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn target_root_never_contains_the_machine_path() {
        let temp = TempDir::new("root");
        let skills = temp.path().join(".claude").join("skills");
        write(
            &skills.join("demo").join("SKILL.md"),
            "---\nname: demo\n---\n",
        );

        let found = discover(&ClaudeCode, temp.path(), Scope::Project).unwrap();
        assert_eq!(found[0].root, "demo");
        assert!(
            !found[0].root.contains(std::path::MAIN_SEPARATOR),
            "target.root must be forward-slashed and relative, never a machine path"
        );
    }

    #[test]
    fn relative_paths_are_forward_slashed_and_contained() {
        let base = Path::new("/tmp/bundle");
        assert_eq!(
            relative_slash_path(base, &base.join("scripts").join("run.sh")).as_deref(),
            Some("scripts/run.sh")
        );
        assert_eq!(relative_slash_path(base, base).as_deref(), Some(""));
        // Not a descendant.
        assert_eq!(relative_slash_path(base, Path::new("/etc/passwd")), None);
    }

    #[test]
    fn search_paths_are_stable() {
        for scope in Scope::ALL {
            assert_eq!(
                ClaudeCode.search_paths(*scope),
                vec![PathBuf::from(".claude").join("skills")]
            );
        }
    }
}
