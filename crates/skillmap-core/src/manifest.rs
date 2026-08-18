//! The manifest types.
//!
//! These mirror `schema/manifest-v1.schema.json` exactly. Two structural rules
//! from `AGENTS.md` are load-bearing here and are enforced by the shape of the
//! types rather than by review:
//!
//! - **Invariant 5, tier separation.** `proven`, `pattern`, and `advisory`
//!   findings are three distinct types in three distinct fields. There is no
//!   `tier` field to filter on and therefore no way to filter on it wrongly.
//! - **Invariant 1, no verdicts.** No score, grade, severity, or `suspicious`
//!   field exists anywhere below, and none can be added without a schema-version
//!   event.

use crate::{Digest, Error, NonEmpty};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::num::NonZeroU64;

/// The schema version this crate produces.
pub const SCHEMA_VERSION: &str = "1.4.0";

/// A skillmap capability manifest.
///
/// Descriptive only: it records what a bundle *can do* and the evidence for each
/// claim. It renders no judgement — that belongs in `policy.toml`.
///
/// Build one, then render it with [`Manifest::to_canonical_json`], which is the
/// only supported way to serialize it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    /// Schema version, `major.minor.patch`. See [`SCHEMA_VERSION`].
    pub schema_version: String,
    /// Identity and version of the tool that produced this manifest.
    pub tool: Tool,
    /// The bundle that was analyzed.
    pub target: Target,
    /// Every file in the bundle, sorted by `path`.
    pub inventory: Vec<InventoryEntry>,
    /// What the bundle discloses about itself up front.
    pub disclosure: Disclosure,
    /// Tier `proven` — code-plane static analysis.
    pub capabilities: Vec<Capability>,
    /// Tier `pattern` — instruction-plane lexical signals over prose.
    pub instructions: Vec<Instruction>,
    /// Gaps in the analysis **of the bundle**. Top-level so it stays visible when
    /// `capabilities` is empty, which is exactly when it matters most.
    pub unresolved: Vec<Unresolved>,
    /// Tier `advisory` — the quarantined model pass.
    pub advisory: Advisory,
    /// Problems with **the run**, not with the bundle.
    pub diagnostics: Vec<Diagnostic>,
}

impl Manifest {
    /// How many files the **code plane** actually read.
    ///
    /// Zero is the common case and the important one: 89.8% of published skills
    /// ship no file any grammar covers, so their manifest is empty because
    /// nothing looked, not because nothing was there. Every caller that needs to
    /// tell those apart — the human report, and the differ deciding whether a
    /// content change went unreviewed — asks here, so there is one definition.
    ///
    /// `code_languages` comes from the loaded [`RuleSet`](skillmap_rules) minus
    /// markdown, so adding a grammar changes every answer with no edit here
    /// (invariant 7). A file whose language has no grammar counts as unread:
    /// `parsed_as` records what a file *is*, not what was done to it.
    #[must_use]
    pub fn code_files_read(&self, code_languages: &BTreeSet<String>) -> usize {
        self.inventory
            .iter()
            .filter(|entry| {
                entry.parse_status == ParseStatus::Ok && code_languages.contains(&entry.parsed_as)
            })
            .count()
    }
}

/// Identity of the tool that produced a manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Tool {
    /// Tool name, e.g. `skillmap`.
    pub name: String,
    /// Tool version, e.g. `0.1.0`.
    pub version: String,
}

/// The bundle a manifest describes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Target {
    /// What kind of bundle this is.
    pub kind: BundleKind,
    /// Bundle name.
    pub name: String,
    /// Id of the resolver that discovered it, e.g. `claude-code`.
    pub resolver: String,
    /// Bundle root as a forward-slash path relative to the **resolver's discovery
    /// root** — for `claude-code`, the path under `.claude/skills/`. Never
    /// relative to cwd or the project directory: either would leak machine layout
    /// into the manifest and break byte-identity between two checkouts.
    pub root: String,
    /// Merkle root over the sorted inventory. See [`crate::content_digest`].
    pub content_digest: Digest,
}

/// What kind of bundle a [`Target`] is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BundleKind {
    /// A single skill.
    Skill,
    /// A plugin, which may wrap several skills.
    Plugin,
    /// A bundle of related skills.
    Bundle,
}

/// One file in the bundle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InventoryEntry {
    /// Forward-slash path relative to the bundle root.
    pub path: String,
    /// Size in bytes.
    pub size: u64,
    /// SHA-256 of the file's bytes, LF-normalized for text.
    pub sha256: Digest,
    /// When this file enters the agent's context.
    pub load_phase: LoadPhase,
    /// What the file was parsed as, e.g. `markdown`, `python`.
    pub parsed_as: String,
    /// Whether parsing succeeded.
    pub parse_status: ParseStatus,
}

/// When a file enters the agent's context.
///
/// The gap between [`LoadPhase::Always`] and [`LoadPhase::Reference`] is the
/// disclosure signal this project is built around.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LoadPhase {
    /// The frontmatter description — the ~100 tokens seen at session start.
    Always,
    /// The `SKILL.md` body.
    OnTrigger,
    /// Reachable from the body by link or explicit instruction.
    Reference,
    /// Present in the bundle, reachable by no documented path.
    Unreferenced,
}

/// Whether a file could be parsed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParseStatus {
    /// Parsed cleanly.
    Ok,
    /// Parsing failed; an [`Unresolved`] entry accompanies this.
    Error,
    /// No grammar for this file type; an [`Unresolved`] entry accompanies this.
    Unsupported,
}

/// What the bundle says about itself before any analysis.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Disclosure {
    /// Size of the frontmatter description in bytes.
    pub description_bytes: u64,
    /// Capability strings **verbatim** from third-party frontmatter, sorted and
    /// deduplicated.
    ///
    /// Deliberately raw `String`, not [`CapabilityTerm`]: these are written by
    /// authors who have never seen our vocabulary, so constraining them to the
    /// closed taxonomy would hard-fail on the first real bundle. Mapping them into
    /// the taxonomy is a separate, explicitly lossy step that never happens
    /// silently.
    pub declared_capabilities: Vec<String>,
    /// Terms extracted from the description, sorted and deduplicated. Extracted,
    /// not scored.
    pub trigger_terms: Vec<String>,
    /// How many files are in [`LoadPhase::Reference`].
    pub reference_files: u64,
    /// How many files are in [`LoadPhase::Unreferenced`]. A script nothing links
    /// to is either dead weight or a payload waiting to be wired up.
    pub unreferenced_files: u64,
}

/// A capability finding. Tier `proven`: code-plane static analysis only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Capability {
    /// Term from the closed taxonomy.
    pub capability: CapabilityTerm,
    /// How much the analysis actually established about reachability.
    pub reachability: Reachability,
    /// Statically resolved paths or hosts, when there are any.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub detail: Option<Detail>,
    /// Evidence, sorted by `(file, start_byte)`. Non-empty by construction: the
    /// schema declares `minItems: 1`, and invariant 4 says a finding nobody can
    /// point at cannot be regression-tested.
    pub evidence: NonEmpty<EvidenceStrict>,
}

/// An instruction-plane finding. Tier `pattern`: lexical, deliberately weak, and
/// never promoted into [`Capability`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Instruction {
    /// Signal from the closed `instruction.*` vocabulary.
    pub signal: InstructionSignal,
    /// Evidence, sorted by `(file, start_byte)`. Non-empty by construction, for
    /// the same reason as [`Capability::evidence`].
    pub evidence: NonEmpty<EvidenceStrict>,
}

/// Statically resolved detail attached to a [`Capability`].
///
/// Closed by design: this sits inside an artifact required to be byte-identical,
/// so every key needs a declared type and a declared order. Adding one is a
/// schema-version event, the same rule as adding a taxonomy term.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Detail {
    /// Paths, sorted and deduplicated.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub paths: Option<Vec<String>>,
    /// Hosts, sorted and deduplicated.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub hosts: Option<Vec<String>>,
}

impl Detail {
    /// Whether this carries nothing. An empty `detail` is dropped during
    /// canonicalization so a bare `{}` never reaches the artifact.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.paths.as_ref().is_none_or(Vec::is_empty)
            && self.hosts.as_ref().is_none_or(Vec::is_empty)
    }
}

/// Provenance for the deterministic tiers (`proven`, `pattern`).
///
/// Complete by requirement: a rule fired, so all five fields exist, and a finding
/// that cannot be pointed at cannot be regression-tested.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceStrict {
    /// Forward-slash path relative to the bundle root.
    pub file: String,
    /// Byte offset where the evidence span starts.
    pub start_byte: u64,
    /// Byte offset one past where it ends.
    pub end_byte: u64,
    /// 1-indexed line of `start_byte`. `NonZeroU64` because the schema declares
    /// `minimum: 1` and line 0 does not exist.
    pub start_line: NonZeroU64,
    /// Id of the rule that fired.
    pub rule_id: String,
    /// SHA-256 of the captured snippet, so a regression test can detect the span
    /// drifting onto different bytes.
    pub snippet_sha256: Digest,
}

/// Provenance for the `advisory` tier: file and line only.
///
/// This type structurally *cannot* carry a `rule_id` or a snippet hash. No rule
/// fired, and a byte span back-derived from a model's prose citation is
/// manufactured precision — worse than an honest line number, because it looks
/// checkable and isn't.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceAdvisory {
    /// Forward-slash path relative to the bundle root.
    pub file: String,
    /// 1-indexed line the model cited. `NonZeroU64`: see [`EvidenceStrict::start_line`].
    pub start_line: NonZeroU64,
}

/// Something the analysis could not cover **in the bundle**.
///
/// Never a silent skip (invariant 3). If a different tool scanning the same
/// bundle would hit the same wall, it belongs here; if it is a fault in this
/// run, it is a [`Diagnostic`] instead.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Unresolved {
    /// Why the analysis stopped.
    pub reason: UnresolvedReason,
    /// File where it stopped.
    pub file: String,
    /// Byte offset, when there is a specific site.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub start_byte: Option<u64>,
    /// End byte offset, when there is a specific site.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub end_byte: Option<u64>,
    /// 1-indexed line, when there is a specific site.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub start_line: Option<NonZeroU64>,
    /// Human-readable detail.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub note: Option<String>,
}

/// Why an analysis gap exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnresolvedReason {
    /// The call target is chosen at runtime.
    DynamicDispatch,
    /// The sink's argument is a variable, concatenation, or computed value.
    ComputedTarget,
    /// A call through a value the analysis could not follow.
    IndirectCall,
    /// No grammar is wired up for this file's language.
    UnsupportedLanguage,
    /// The file's grammar rejected it.
    ParseError,
    /// The file is not text.
    BinaryFile,
    /// The file exceeded the configured size limit.
    SizeLimit,
    /// A symlink pointed outside the bundle root.
    SymlinkEscape,
}

/// The quarantined model pass.
///
/// An enum rather than a struct with a `bool`, so the schema's `if`/`then`
/// pinning rule is unrepresentable to violate: "did not run" carries no model,
/// no prompt hash, and no findings; "ran" carries all three. `enabled: false` is
/// still *present* in the output — "not checked" and "checked, found nothing"
/// must stay distinguishable in a diff.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Advisory {
    /// The pass did not run.
    Disabled,
    /// The pass ran, pinned to a specific model and prompt.
    Enabled(AdvisoryRun),
}

/// A semantic pass that actually ran.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdvisoryRun {
    /// Model id, e.g. `claude-sonnet-5`. Pinned because an unpinned advisory
    /// branch is not reproducible and turns every CI diff into noise.
    pub model: String,
    /// SHA-256 of the prompt template, pinned for the same reason.
    pub prompt_sha256: Digest,
    /// Findings, sorted by `(kind, first evidence file, first evidence
    /// start_line, claim)`. May legitimately be empty: that is "checked, found
    /// nothing".
    pub findings: Vec<AdvisoryFinding>,
}

impl Advisory {
    /// Findings, or an empty slice when the pass did not run.
    #[must_use]
    pub fn findings(&self) -> &[AdvisoryFinding] {
        match self {
            Self::Disabled => &[],
            Self::Enabled(run) => &run.findings,
        }
    }

    /// Whether the pass ran.
    #[must_use]
    pub const fn is_enabled(&self) -> bool {
        matches!(self, Self::Enabled(_))
    }
}

/// The flat on-the-wire shape of [`Advisory`], matching the schema.
///
/// Only exists to bridge the enum to the JSON object the schema declares; the
/// consistency rules the schema expresses as `if`/`then` are checked once here,
/// at the boundary, and are then unrepresentable in the parsed value.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct AdvisoryWire {
    enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    prompt_sha256: Option<Digest>,
    findings: Vec<AdvisoryFinding>,
}

impl Serialize for Advisory {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let wire = match self {
            Self::Disabled => AdvisoryWire {
                enabled: false,
                model: None,
                prompt_sha256: None,
                findings: Vec::new(),
            },
            Self::Enabled(run) => AdvisoryWire {
                enabled: true,
                model: Some(run.model.clone()),
                prompt_sha256: Some(run.prompt_sha256),
                findings: run.findings.clone(),
            },
        };
        wire.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Advisory {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let wire = AdvisoryWire::deserialize(deserializer)?;
        let inconsistent =
            |why| serde::de::Error::custom(Error::InconsistentAdvisory(why).to_string());
        match (wire.enabled, wire.model, wire.prompt_sha256) {
            (true, Some(model), Some(prompt_sha256)) => Ok(Self::Enabled(AdvisoryRun {
                model,
                prompt_sha256,
                findings: wire.findings,
            })),
            (true, _, _) => Err(inconsistent(
                "the pass ran but did not pin both `model` and `prompt_sha256`",
            )),
            (false, None, None) if wire.findings.is_empty() => Ok(Self::Disabled),
            (false, None, None) => Err(inconsistent(
                "the pass did not run but carries findings; `enabled: false` means nothing was checked",
            )),
            (false, _, _) => Err(inconsistent(
                "the pass did not run but pins a model or prompt hash",
            )),
        }
    }
}

/// One finding from the semantic pass.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdvisoryFinding {
    /// What kind of claim this is.
    pub kind: AdvisoryKind,
    /// The model's claim, in prose.
    pub claim: String,
    /// Where the model says to look. File and line only — see [`EvidenceAdvisory`].
    /// Non-empty by construction: a claim with nothing to look at is not a finding.
    pub evidence: NonEmpty<EvidenceAdvisory>,
}

/// Kind of [`AdvisoryFinding`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdvisoryKind {
    /// Capability present in `reference` files but not implied by `always` content.
    DisclosureDelta,
    /// Prose directing behaviour the deterministic tiers did not flag.
    UndeclaredInstruction,
    /// Content apparently aimed at the agent reading the bundle.
    InjectionAttempt,
    /// Intent the bundle appears to be concealing.
    ObfuscatedIntent,
}

/// Something wrong with **the run**, not with the bundle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Diagnostic {
    /// What went wrong.
    pub code: DiagnosticCode,
    /// The file involved, when there is one.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub file: Option<String>,
    /// Human-readable detail.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub note: Option<String>,
}

/// Run-scoped diagnostic code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticCode {
    /// A rule file could not be read or parsed.
    RuleLoadError,
    /// A rule loaded but failed validation.
    RuleValidationError,
    /// The semantic pass returned output that failed schema validation; the
    /// finding was discarded.
    SemanticSchemaViolation,
    /// The semantic model call did not complete.
    SemanticCallFailed,
    /// `policy.toml` could not be read or parsed.
    PolicyLoadError,
}

/// How much the analysis established about a [`Capability`].
///
/// Never silently upgraded: a capability whose evidence is entirely
/// [`Reachability::Present`] stays `present`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Reachability {
    /// Reachable from a bundle entry point.
    Observed,
    /// The sink exists; reachability is unproven.
    Present,
    /// Dynamic dispatch or a computed target blocked the analysis.
    Unresolved,
}

/// A term from the closed capability taxonomy.
///
/// Adding a term is a schema-version event, and invariant 12 forbids shipping one
/// that no rule detects.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CapabilityTerm {
    /// Spawns a subprocess with a statically-known target.
    #[serde(rename = "process.exec")]
    ProcessExec,
    /// Spawns a subprocess with a computed target.
    #[serde(rename = "process.exec.dynamic")]
    ProcessExecDynamic,
    /// Outbound network; `detail.hosts` when statically resolvable.
    #[serde(rename = "net.egress")]
    NetEgress,
    /// Fetched content reaches an exec or eval sink.
    #[serde(rename = "net.fetch_then_execute")]
    NetFetchThenExecute,
    /// Reads a file at a path conventionally holding credentials.
    ///
    /// **Files only.** This said "or secret-bearing env var" until
    /// `env.read.secret` got a rule and the overlap became load-bearing: the two
    /// terms claimed the same act, so which one a manifest carried would have
    /// depended on which rule happened to fire. No wire name changed and no
    /// manifest has ever carried this term for an environment read, so the
    /// serialized form is untouched — but the *meaning* narrowed, and that is
    /// worth knowing when reading an older manifest.
    #[serde(rename = "fs.read.credential")]
    FsReadCredential,
    /// Reads outside the bundle root and project.
    #[serde(rename = "fs.read.outside_bundle")]
    FsReadOutsideBundle,
    /// Writes outside the bundle root.
    #[serde(rename = "fs.write.outside_bundle")]
    FsWriteOutsideBundle,
    /// Writes `CLAUDE.md`, `settings.json`, hook or statusline config.
    #[serde(rename = "fs.write.agent_config")]
    FsWriteAgentConfig,
    /// `eval`, `exec`, `Function`, `source` of computed content.
    #[serde(rename = "code.dynamic_eval")]
    CodeDynamicEval,
    /// An encoding/decoding chain feeding a sink.
    #[serde(rename = "code.obfuscation")]
    CodeObfuscation,
    /// Reads env vars matching the secret-name set.
    #[serde(rename = "env.read.secret")]
    EnvReadSecret,
}

/// A signal from the closed instruction-plane vocabulary.
///
/// A separate namespace from [`CapabilityTerm`] on purpose: these never appear in
/// `capabilities` and can never be promoted there.
///
/// # Three terms were removed at schema 1.4.0, and the reasons differ
///
/// `instruction.exfil` **shipped a rule and was withdrawn on measurement.** T13
/// hand-labelled 145 bundles across four strata and it scored **2/36 precision**
/// on the stratum drawn for it. The false positives are not tunable: in this
/// corpus `send` and `transfer` usually mean moving crypto tokens, `post` means
/// publishing, `push` means mobile notifications, and the largest single group
/// is prose *forbidding* the behaviour — a security-hardening policy made
/// entirely of prohibitions fires on it by naming what it forbids. Two repairs
/// were measured rather than assumed: qualifying the noun gave 1/30, and adding
/// a negation guard gave 0/7, because the guard removed 23 false positives and
/// both true positives with them.
///
/// `instruction.silence` and `instruction.privilege_claim` **never had a rule at
/// all.** They sat in this enum from T5 as vocabulary a detector might one day
/// need, and nothing could ever produce them, so every manifest claimed a
/// taxonomy two terms wider than the tool could fill. T13's pass read 156
/// bundles and found no candidate prose for either. A vocabulary entry no code
/// path can emit is the shape invariant 12 forbids.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InstructionSignal {
    /// Prose telling the agent to treat fetched content as instructions.
    #[serde(rename = "instruction.fetch_as_instruction")]
    FetchAsInstruction,
    /// Prose directing edits to agent config.
    #[serde(rename = "instruction.config_mutation")]
    ConfigMutation,
    /// Prose directing the agent to run a command that writes to, copies into,
    /// or makes executable a path outside the bundle.
    ///
    /// The shape carries its own intent, which is why it is reportable at all.
    /// Reference material demonstrates logic and never mutates the reader's
    /// machine as an illustration — nobody teaches programming by appending to
    /// a shell profile. Three earlier candidates (directing egress, credential
    /// access, subprocess spawning) were defined and then withdrawn because a
    /// network call or a subprocess inside a code sample is reference material,
    /// and they carried 23-26% base rates with no contextual separator.
    ///
    /// `mkdir` alone is excluded: creating an empty directory is preparation,
    /// not a write. `sudo` is excluded for the opposite reason, being a
    /// package-manager invocation in nearly all of its corpus instances.
    #[serde(rename = "instruction.directs_outside_write")]
    DirectsOutsideWrite,
    /// Prose directing the agent to run a command that fetches remote content
    /// and executes it.
    ///
    /// Narrow on purpose. "Directs execution of a command it supplies" is
    /// satisfied by `python scripts/build.py` in any usage section, and a
    /// signal that fires on every skill describes nothing. What this reports is
    /// that the code being run **is not in the bundle and is not reviewable**:
    /// it arrives from a URL at run time, so reading the bundle cannot tell you
    /// what will execute.
    ///
    /// Not a verdict. Of 40 corpus bundles drawn for this shape, 34 carry it
    /// and nearly all are ordinary installer instructions for real tools. The
    /// manifest reports the directive and its bytes; `policy.toml` decides.
    #[serde(rename = "instruction.exec_directive")]
    ExecDirective,
}

/// Generate `as_str` plus an `ALL` slice, and a test proving `as_str` agrees with
/// what serde emits.
///
/// Sort orders are defined over these strings rather than over `#[derive(Ord)]`,
/// which would silently reorder the manifest the day somebody moved an enum
/// variant. The paired test is what keeps the two spellings from drifting.
macro_rules! wire_names {
    ($ty:ty, $test:ident, [$($variant:ident => $wire:literal),+ $(,)?]) => {
        impl $ty {
            /// Every variant, for exhaustive iteration in tests and tooling.
            pub const ALL: &'static [Self] = &[$(Self::$variant),+];

            /// The exact string this variant serializes to.
            #[must_use]
            pub const fn as_str(&self) -> &'static str {
                match self { $(Self::$variant => $wire),+ }
            }
        }

        #[cfg(test)]
        #[test]
        fn $test() {
            for variant in <$ty>::ALL {
                let via_serde = serde_json::to_value(variant)
                    .ok()
                    .and_then(|v| v.as_str().map(str::to_owned));
                assert_eq!(
                    via_serde.as_deref(),
                    Some(variant.as_str()),
                    "as_str() disagrees with serde for {variant:?}; sort order and wire form must not drift"
                );
            }
        }
    };
}

wire_names!(CapabilityTerm, capability_term_wire_names_match_serde, [
    ProcessExec => "process.exec",
    ProcessExecDynamic => "process.exec.dynamic",
    NetEgress => "net.egress",
    NetFetchThenExecute => "net.fetch_then_execute",
    FsReadCredential => "fs.read.credential",
    FsReadOutsideBundle => "fs.read.outside_bundle",
    FsWriteOutsideBundle => "fs.write.outside_bundle",
    FsWriteAgentConfig => "fs.write.agent_config",
    CodeDynamicEval => "code.dynamic_eval",
    CodeObfuscation => "code.obfuscation",
    EnvReadSecret => "env.read.secret",
]);

wire_names!(InstructionSignal, instruction_signal_wire_names_match_serde, [
    FetchAsInstruction => "instruction.fetch_as_instruction",
    ConfigMutation => "instruction.config_mutation",
    ExecDirective => "instruction.exec_directive",
    DirectsOutsideWrite => "instruction.directs_outside_write",
]);

wire_names!(UnresolvedReason, unresolved_reason_wire_names_match_serde, [
    DynamicDispatch => "dynamic_dispatch",
    ComputedTarget => "computed_target",
    IndirectCall => "indirect_call",
    UnsupportedLanguage => "unsupported_language",
    ParseError => "parse_error",
    BinaryFile => "binary_file",
    SizeLimit => "size_limit",
    SymlinkEscape => "symlink_escape",
]);

wire_names!(AdvisoryKind, advisory_kind_wire_names_match_serde, [
    DisclosureDelta => "disclosure_delta",
    UndeclaredInstruction => "undeclared_instruction",
    InjectionAttempt => "injection_attempt",
    ObfuscatedIntent => "obfuscated_intent",
]);

wire_names!(DiagnosticCode, diagnostic_code_wire_names_match_serde, [
    RuleLoadError => "rule_load_error",
    RuleValidationError => "rule_validation_error",
    SemanticSchemaViolation => "semantic_schema_violation",
    SemanticCallFailed => "semantic_call_failed",
    PolicyLoadError => "policy_load_error",
]);

wire_names!(LoadPhase, load_phase_wire_names_match_serde, [
    Always => "always",
    OnTrigger => "on_trigger",
    Reference => "reference",
    Unreferenced => "unreferenced",
]);

wire_names!(ParseStatus, parse_status_wire_names_match_serde, [
    Ok => "ok",
    Error => "error",
    Unsupported => "unsupported",
]);

wire_names!(Reachability, reachability_wire_names_match_serde, [
    Observed => "observed",
    Present => "present",
    Unresolved => "unresolved",
]);

wire_names!(BundleKind, bundle_kind_wire_names_match_serde, [
    Skill => "skill",
    Plugin => "plugin",
    Bundle => "bundle",
]);
