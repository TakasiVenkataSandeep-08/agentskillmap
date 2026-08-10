#![warn(missing_docs)]

//! Turn a discovered bundle into a manifest.
//!
//! One parser, shared across every agent — what differs per agent is discovery,
//! which lives in `skillmap-resolve`. This crate reads what a bundle contains:
//! frontmatter, a file inventory with per-file digests, the merkle
//! `content_digest`, and **load-phase classification**, which is the signal the
//! project exists to surface (see [`refgraph`]).
//!
//! It produces no `capabilities` and no `instructions`. Those need the rule
//! engine (T4) and the instruction plane (T5); a manifest out of this crate has
//! both arrays empty, and says so honestly rather than implying nothing was found.

pub mod frontmatter;
pub mod inventory;
pub mod refgraph;

use skillmap_core::{
    content_digest, Advisory, Disclosure, Manifest, Target, Tool, Unresolved, UnresolvedReason,
    SCHEMA_VERSION,
};
use skillmap_resolve::{BundleRef, Resolver};
use std::path::{Path, PathBuf};

/// The entry point every bundle is required to have.
pub const ENTRY_FILE: &str = "SKILL.md";

/// Bounds on what the parser will read.
///
/// Hostile input is the normal case here (invariant 10), so every bound is
/// explicit and every breach is reported rather than silently applied.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// Largest file read into memory and analyzed. Larger files are still
    /// hashed and inventoried — identity must stay exact — but their content is
    /// never held in memory, and they gain an `unresolved` entry with reason
    /// `size_limit`.
    pub max_file_bytes: u64,
}

impl Default for Limits {
    fn default() -> Self {
        // 5 MiB. Comfortably above any real SKILL.md or helper script, well below
        // anything that would make a scan of a large repository swap.
        Self {
            max_file_bytes: 5 * 1024 * 1024,
        }
    }
}

/// Failures that make an entire bundle unreadable.
///
/// Deliberately narrow. Anything wrong with a *single file* is an `unresolved`
/// entry in the manifest, not an error — one unreadable file must not take down
/// the scan of everything around it.
#[derive(Debug)]
pub enum ParseError {
    /// A path could not be read or listed.
    Io {
        /// The path involved.
        path: PathBuf,
        /// Why.
        source: std::io::Error,
    },
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io { path, source } => write!(f, "cannot read {}: {source}", path.display()),
        }
    }
}

impl std::error::Error for ParseError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
        }
    }
}

/// Parse a discovered bundle into a manifest.
///
/// `resolver` supplies the agent-specific frontmatter keys that carry declared
/// capabilities; everything else about the parse is shared across agents.
///
/// # Errors
///
/// [`ParseError`] if the bundle root cannot be walked at all.
pub fn parse_bundle(
    bundle: &BundleRef,
    resolver: &dyn Resolver,
    limits: &Limits,
) -> Result<Manifest, ParseError> {
    let root = bundle.path();
    let walk = inventory::walk(&root, limits)?;
    let mut unresolved = walk.unresolved;

    let entry_text = walk
        .files
        .iter()
        .find(|file| file.path == ENTRY_FILE)
        .and_then(|file| file.text.as_deref());

    let front = read_frontmatter(entry_text, &mut unresolved);

    let description = front
        .as_ref()
        .and_then(|f| f.scalar("description"))
        .unwrap_or_default();

    let declared_capabilities = front.as_ref().map_or_else(Vec::new, |f| {
        let mut declared: Vec<String> = resolver
            .declared_capability_keys()
            .iter()
            .flat_map(|key| f.list(key))
            .map(str::to_owned)
            .collect();
        // Canonicalization sorts and dedupes anyway; doing it here too keeps this
        // function's own output stable for callers that inspect it directly.
        declared.sort();
        declared.dedup();
        declared
    });

    let phases = refgraph::classify(ENTRY_FILE, &walk.files);
    let digest_input: Vec<(String, skillmap_core::Digest)> = walk
        .files
        .iter()
        .map(|file| (file.path.clone(), file.sha256))
        .collect();

    let entries = inventory::to_entries(walk.files, |path| {
        phases
            .get(path)
            .copied()
            // Unreachable: `classify` assigns a phase to every walked file. Being
            // explicit beats an unwrap in a library crate (invariant 10), and
            // "unreferenced" is the honest answer if it ever did happen.
            .unwrap_or(skillmap_core::LoadPhase::Unreferenced)
    });

    let reference_files = count_phase(&entries, skillmap_core::LoadPhase::Reference);
    let unreferenced_files = count_phase(&entries, skillmap_core::LoadPhase::Unreferenced);

    let name = front
        .as_ref()
        .and_then(|f| f.scalar("name"))
        .filter(|name| !name.trim().is_empty())
        .map_or_else(|| bundle.name.clone(), str::to_owned);

    let mut manifest = Manifest {
        schema_version: SCHEMA_VERSION.to_owned(),
        tool: Tool {
            name: env!("CARGO_PKG_NAME")
                .strip_suffix("-parse")
                .unwrap_or("skillmap")
                .to_owned(),
            version: env!("CARGO_PKG_VERSION").to_owned(),
        },
        target: Target {
            kind: bundle.kind,
            name,
            resolver: resolver.id().to_owned(),
            root: bundle.root.clone(),
            content_digest: content_digest(&digest_input),
        },
        inventory: entries,
        disclosure: Disclosure {
            description_bytes: description.len() as u64,
            declared_capabilities,
            trigger_terms: refgraph::trigger_terms(description),
            reference_files,
            unreferenced_files,
        },
        // No code plane and no instruction plane yet (T4, T5). Empty here means
        // "not analyzed", and the `unresolved` list above is what makes that
        // distinguishable from "analyzed, found nothing" (invariant 3).
        capabilities: Vec::new(),
        instructions: Vec::new(),
        unresolved,
        // The semantic pass is opt-in and lives in T7. `Disabled` is present in
        // the output rather than omitted, so "not checked" and "checked, found
        // nothing" stay distinguishable in a diff.
        advisory: Advisory::Disabled,
        diagnostics: Vec::new(),
    };

    // Sort now so callers that read the struct see the same order the serialized
    // artifact will have. `to_canonical_json` would do this anyway; doing it here
    // means the two can never disagree.
    manifest.canonicalize().map_err(|error| ParseError::Io {
        path: root,
        source: std::io::Error::other(error.to_string()),
    })?;

    Ok(manifest)
}

/// Parse the entry file's frontmatter, recording any failure as `unresolved`.
fn read_frontmatter(
    entry_text: Option<&str>,
    unresolved: &mut Vec<Unresolved>,
) -> Option<frontmatter::Frontmatter> {
    let Some(text) = entry_text else {
        // No SKILL.md, or it was binary or unreadable. The walk already reported
        // why if the file existed; this covers the case where it does not.
        unresolved.push(Unresolved {
            reason: UnresolvedReason::ParseError,
            file: ENTRY_FILE.to_owned(),
            start_byte: None,
            end_byte: None,
            start_line: None,
            note: Some(format!(
                "{ENTRY_FILE} is missing or could not be read as text"
            )),
        });
        return None;
    };

    match frontmatter::parse(text) {
        Ok(front) => Some(front),
        Err(error) => {
            unresolved.push(Unresolved {
                reason: UnresolvedReason::ParseError,
                file: ENTRY_FILE.to_owned(),
                start_byte: None,
                end_byte: None,
                start_line: std::num::NonZeroU64::new(error.line),
                note: Some(format!("frontmatter: {}", error.message)),
            });
            None
        }
    }
}

/// Count inventory entries in a given load phase.
fn count_phase(entries: &[skillmap_core::InventoryEntry], phase: skillmap_core::LoadPhase) -> u64 {
    entries.iter().filter(|e| e.load_phase == phase).count() as u64
}

/// Parse a bundle directly from a path, without discovery.
///
/// For scanning a directory the user named explicitly. `discovery_root` is the
/// directory `target.root` is reported relative to — pass the bundle's parent to
/// get just the bundle's own directory name, which is what discovery would have
/// produced.
///
/// # Errors
///
/// [`ParseError`] if the bundle root cannot be walked.
pub fn parse_path(
    path: &Path,
    resolver: &dyn Resolver,
    limits: &Limits,
) -> Result<Manifest, ParseError> {
    let absolute = path.canonicalize().map_err(|source| ParseError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let discovery_root = absolute.parent().unwrap_or(&absolute).to_path_buf();
    let name = absolute
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("bundle")
        .to_owned();

    let bundle = BundleRef {
        resolver: resolver.id(),
        kind: resolver
            .classify(&absolute)
            .unwrap_or(skillmap_core::BundleKind::Skill),
        name: name.clone(),
        discovery_root,
        root: name,
    };
    parse_bundle(&bundle, resolver, limits)
}
