#![warn(missing_docs)]

//! Assembling a full manifest from all three planes.
//!
//! This lived inside `skillmap-eval` until T8, with a note saying a crate whose
//! only job is to call three functions, *written before there is a second
//! caller*, would be a stub. T8 produced the second caller: `skillmap ci` has to
//! scan a bundle before it can compare one, and a product binary reaching into
//! the test harness for the ability to scan would have the dependency arrow
//! backwards. So it moved, on the condition its own comment set.
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
use skillmap_semantic::{BundleView, FileView};
use std::path::Path;

pub use skillmap_semantic::{Limits as SemanticLimits, Provider};

/// What the semantic pass is allowed to see.
///
/// The description comes from `SKILL.md` frontmatter — the ~100 tokens an agent
/// reads at session start, and the thing every deep file is compared against.
/// Everything else in the bundle is deep content, including `SKILL.md`'s own
/// body: a body that instructs something its description omits is a disclosure
/// delta as surely as a reference file that does.
///
/// It does not take the `Manifest`. That is deliberate and is the cheapest
/// possible enforcement of the quarantine: the function that builds the semantic
/// pass's input cannot copy a deterministic finding into it, because it has
/// never seen one.
fn bundle_view(walk: &inventory::Walk) -> BundleView {
    let description = walk
        .files
        .iter()
        .find(|file| file.path == "SKILL.md")
        .and_then(|file| file.text.as_deref())
        .and_then(|text| skillmap_parse::frontmatter::parse(text).ok())
        .and_then(|front| front.scalar("description").map(str::to_owned))
        .unwrap_or_default();

    // Inventory order, so two runs over the same bytes build the same prompt.
    let mut files: Vec<FileView> = walk
        .files
        .iter()
        .filter_map(|file| {
            Some(FileView {
                path: file.path.clone(),
                text: file.text.as_deref()?.to_owned(),
            })
        })
        .collect();
    files.sort_by(|a, b| a.path.cmp(&b.path));

    BundleView { description, files }
}

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
    analyze_bundle_with(bundle, rules, resolver, None)
}

/// Analyze a bundle and run the quarantined semantic pass over it.
///
/// The **only** function in the workspace that sees both a deterministic
/// finding and an advisory one, which is what `docs/04-semantic-layer.md` means
/// by *"the manifest assembler is the only code that sees both"*. Read the
/// assembly below with that in mind: the advisory result is written to
/// `manifest.advisory` and nowhere else, and it arrives after `capabilities`,
/// `instructions` and the code plane's `unresolved` are already final.
///
/// This is also the single network path in the scan flow (invariant 9), and it
/// opens only because a caller passed a provider.
///
/// # Errors
///
/// Propagates the parser's error if the bundle cannot be walked.
pub fn analyze_bundle_advised(
    bundle: &Path,
    rules: &RuleSet,
    resolver: &dyn Resolver,
    provider: &dyn Provider,
    semantic_limits: &SemanticLimits,
) -> Result<Manifest, skillmap_parse::ParseError> {
    analyze_bundle_with(bundle, rules, resolver, Some((provider, semantic_limits)))
}

fn analyze_bundle_with(
    bundle: &Path,
    rules: &RuleSet,
    resolver: &dyn Resolver,
    semantic: Option<(&dyn Provider, &SemanticLimits)>,
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

    // The quarantine, at the one point where it could be broken. Everything the
    // deterministic tiers produce is already assembled above and is never read
    // again below; the advisory result touches `advisory`, `unresolved` (for
    // content it could not cover — invariant 3 applies to it too) and
    // `diagnostics`, and has no path to `capabilities` or `instructions`.
    //
    // `BundleView` is what makes that structural rather than careful: it carries
    // a description and file text, and has no field through which a capability
    // could be handed to the semantic pass or returned from it.
    if let Some((provider, semantic_limits)) = semantic {
        let view = bundle_view(&walk);
        let outcome = skillmap_semantic::analyze(&view, provider, semantic_limits);
        manifest.advisory = outcome.advisory;
        manifest.unresolved.extend(outcome.unresolved);
        manifest.diagnostics.extend(outcome.diagnostics);
    }

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
