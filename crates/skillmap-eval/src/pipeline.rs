//! Assembling a full manifest from all three planes.
//!
//! This is the first place the parser, the code plane and the instruction plane
//! are wired together, because eval is the first thing that needs a whole
//! manifest rather than one plane's output. **T9's CLI lifts this**; it lives
//! here rather than in a crate of its own because a crate whose only job is to
//! call three functions, written before there is a second caller, is a stub.
//!
//! The assembly order is the one thing worth reading closely. Each plane is
//! called separately and its output goes into its own field; nothing merges
//! findings across tiers, and there is no code path by which an instruction
//! finding could reach `capabilities`. That is invariant 5 at the point where it
//! would actually be easy to break.

use skillmap_code::SourceFile;
use skillmap_core::Manifest;
use skillmap_instr::ProseFile;
use skillmap_parse::{inventory, Limits};
use skillmap_resolve::{ClaudeCode, Resolver};
use skillmap_rules::RuleSet;
use std::path::Path;

/// Parse a bundle and run every plane over it.
///
/// # Errors
///
/// Propagates the parser's error if the bundle cannot be walked at all.
pub fn analyze_bundle(
    bundle: &Path,
    rules: &RuleSet,
    resolver: &dyn Resolver,
) -> Result<Manifest, skillmap_parse::ParseError> {
    let limits = Limits::default();
    let mut manifest = skillmap_parse::parse_path(bundle, resolver, &limits)?;
    let walk = inventory::walk(bundle, &limits)?;

    // The load phase the parser computed decides what counts as running, so the
    // code plane and the parser cannot disagree about it.
    let phase: std::collections::BTreeMap<&str, skillmap_core::LoadPhase> = manifest
        .inventory
        .iter()
        .map(|entry| (entry.path.as_str(), entry.load_phase))
        .collect();

    let sources: Vec<SourceFile<'_>> = walk
        .files
        .iter()
        .filter_map(|file| {
            let text = file.text.as_deref()?;
            let load_phase = phase.get(file.path.as_str()).copied()?;
            Some(SourceFile {
                path: file.path.as_str(),
                text,
                entered: skillmap_code::is_entered(&file.path, load_phase, rules),
            })
        })
        .collect();

    let prose: Vec<ProseFile<'_>> = walk
        .files
        .iter()
        .filter_map(|file| {
            Some(ProseFile {
                path: file.path.as_str(),
                text: file.text.as_deref()?,
            })
        })
        .collect();

    let code = skillmap_code::analyze(&sources, rules);
    let instructions = skillmap_instr::analyze(&prose, rules);

    manifest.capabilities = code.capabilities;
    manifest.instructions = instructions;
    manifest.unresolved.extend(code.unresolved);

    // Canonicalize once at the end so the assembled manifest is in the same order
    // the artifact would be written in.
    manifest
        .canonicalize()
        .map_err(|error| skillmap_parse::ParseError::Io {
            path: bundle.to_path_buf(),
            source: std::io::Error::other(error.to_string()),
        })?;
    Ok(manifest)
}

/// Analyze a bundle with the default `claude-code` resolver.
///
/// # Errors
///
/// Propagates the parser's error if the bundle cannot be walked.
pub fn analyze(bundle: &Path, rules: &RuleSet) -> Result<Manifest, skillmap_parse::ParseError> {
    analyze_bundle(bundle, rules, &ClaudeCode)
}
