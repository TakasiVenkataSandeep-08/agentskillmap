#![warn(missing_docs)]

//! Rule loading, validation, and query compilation.
//!
//! Invariant 7 is the whole point of this crate: **detection is data**. A rule is
//! a TOML file, a tree-sitter query, and two fixtures. Nothing in here knows what
//! a credential path looks like, what `open` means, or that Python exists beyond
//! one line in a grammar registry — adding coverage never requires touching a
//! `.rs` file, and if it appears to, that is a bug in the engine.
//!
//! The engine's entire vocabulary is four **roles**:
//!
//! | Role | Meaning |
//! |---|---|
//! | `site` | Required. The span reported as evidence. |
//! | `path` | A literal filtered through `[match].path_prefixes`. |
//! | `host` | A literal filtered through `[match].host_suffixes`. |
//! | `dynamic` | The target could not be resolved; emit `unresolved`, never silence. |
//!
//! A new sink, a new language, or a new obfuscation trick all reduce to those
//! four. Wanting a fifth is a design discussion, not a rule PR.

use serde::Deserialize;
use skillmap_core::{CapabilityTerm, DiagnosticCode, InstructionSignal};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use tree_sitter::{Language, Query};

/// Grammars compiled into this crate.
///
/// The one place a language name meets Rust. Registering a grammar is not a sink
/// pattern — `docs/03-rules-authoring.md` lists it as step 1 of adding a
/// language, alongside a `rules/languages.toml` section and a query file.
fn grammar(name: &str) -> Option<Language> {
    match name {
        "python" => Some(tree_sitter_python::LANGUAGE.into()),
        _ => None,
    }
}

/// A capture role. The engine's complete vocabulary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Role {
    /// The evidence span. Required on every rule.
    Site,
    /// A path literal, filtered through `[match].path_prefixes`.
    Path,
    /// A host literal, filtered through `[match].host_suffixes`.
    Host,
    /// An unresolvable target; becomes `unresolved: computed_target`.
    Dynamic,
}

impl Role {
    /// Every role, in a fixed order.
    pub const ALL: &'static [Self] = &[Self::Site, Self::Path, Self::Host, Self::Dynamic];

    /// The name used as a `[captures]` key.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Site => "site",
            Self::Path => "path",
            Self::Host => "host",
            Self::Dynamic => "dynamic",
        }
    }

    /// Parse a `[captures]` key.
    #[must_use]
    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|role| role.as_str() == name)
    }
}

/// What a rule claims to detect: a capability, or an instruction-plane signal.
///
/// Two variants rather than one string, so a `proven` rule cannot name an
/// `instruction.*` signal and a `pattern` rule cannot name a capability. The tier
/// separation of invariant 5 starts here, at load time, rather than being
/// something the manifest assembler has to remember.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Claim {
    /// Tier `proven`: a capability term from the closed taxonomy.
    Capability(CapabilityTerm),
    /// Tier `pattern`: an instruction-plane signal.
    Instruction(InstructionSignal),
}

/// Literal data a rule filters its captures through.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MatchData {
    /// Path prefixes that make a `path` capture interesting.
    #[serde(default)]
    pub path_prefixes: Vec<String>,
    /// Host suffixes that make a `host` capture interesting.
    #[serde(default)]
    pub host_suffixes: Vec<String>,
}

/// Prose for humans reading a finding.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Docs {
    /// One line describing what fires.
    #[serde(default)]
    pub summary: String,
    /// Why this is worth reporting, and why it is not a verdict.
    #[serde(default)]
    pub rationale: String,
    /// Known false positives, honestly.
    #[serde(default)]
    pub false_positive_notes: String,
}

/// The on-disk shape of `rules/<lang>/<id>.toml`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RuleFile {
    id: String,
    language: String,
    capability: String,
    tier: String,
    query: String,
    captures: BTreeMap<String, String>,
    #[serde(default, rename = "match")]
    match_data: MatchData,
    #[serde(default)]
    docs: Docs,
}

/// A rule that loaded, validated, and compiled.
pub struct CompiledRule {
    /// Rule id, e.g. `py.credential-read.dotfile`.
    pub id: String,
    /// Language name, matching a `rules/languages.toml` section.
    pub language: String,
    /// What it claims to detect.
    pub claim: Claim,
    /// The compiled query.
    pub query: Query,
    /// Role → capture index within [`CompiledRule::query`].
    pub roles: BTreeMap<Role, u32>,
    /// Literal filters.
    pub match_data: MatchData,
    /// Prose.
    pub docs: Docs,
}

impl CompiledRule {
    /// The capture index for a role, if the rule declares it.
    #[must_use]
    pub fn capture_index(&self, role: Role) -> Option<u32> {
        self.roles.get(&role).copied()
    }
}

/// Per-language configuration from `rules/languages.toml`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LanguageConfig {
    /// File extensions, without the dot.
    pub extensions: Vec<String>,
    /// Path to the reachability query, relative to the repository root.
    pub reachability: String,
    /// Basenames that execute on their own.
    #[serde(default)]
    pub entry_basenames: Vec<String>,
}

/// A language the engine can analyze: config, grammar, and reachability query.
pub struct LoadedLanguage {
    /// The language name.
    pub name: String,
    /// Its configuration.
    pub config: LanguageConfig,
    /// The tree-sitter grammar.
    pub grammar: Language,
    /// The compiled reachability query.
    pub reachability: Query,
}

/// Everything loaded: languages and rules, plus what went wrong loading them.
pub struct RuleSet {
    /// Languages by name.
    pub languages: BTreeMap<String, LoadedLanguage>,
    /// Rules that survived validation, sorted by id.
    pub rules: Vec<CompiledRule>,
    /// Problems encountered. These are **run-scoped**: a rule that would not load
    /// is a fault in this tool's configuration, not in the bundle being scanned,
    /// so it is a `diagnostic` and never an `unresolved` entry.
    pub diagnostics: Vec<skillmap_core::Diagnostic>,
}

impl RuleSet {
    /// The language whose extensions include `extension`.
    #[must_use]
    pub fn language_for_extension(&self, extension: &str) -> Option<&LoadedLanguage> {
        self.languages.values().find(|language| {
            language
                .config
                .extensions
                .iter()
                .any(|candidate| candidate == extension)
        })
    }

    /// Rules for a language, in id order.
    #[must_use]
    pub fn rules_for(&self, language: &str) -> Vec<&CompiledRule> {
        self.rules
            .iter()
            .filter(|rule| rule.language == language)
            .collect()
    }
}

/// Build a run-scoped diagnostic.
fn diagnostic(
    code: DiagnosticCode,
    file: Option<String>,
    note: String,
) -> skillmap_core::Diagnostic {
    skillmap_core::Diagnostic {
        code,
        file,
        note: Some(note),
    }
}

/// Load `rules/languages.toml` and every `rules/**/*.toml` beneath `root`.
///
/// Never fails as a whole: a rule that cannot be read, parsed, or validated
/// becomes a diagnostic and the rest still load. A scanner that refuses to start
/// because one contributed rule has a typo is a scanner nobody can extend.
#[must_use]
pub fn load(root: &Path) -> RuleSet {
    let mut diagnostics = Vec::new();
    let languages = load_languages(root, &mut diagnostics);
    let mut rules = Vec::new();

    for path in rule_files(&root.join("rules")) {
        let relative = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");

        match load_rule(root, &path, &languages) {
            Ok(rule) => rules.push(rule),
            Err((code, note)) => diagnostics.push(diagnostic(code, Some(relative), note)),
        }
    }

    rules.sort_by(|a, b| a.id.cmp(&b.id));

    // A duplicate id would make findings ambiguous and `rules bless` unstable.
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    let mut duplicates: Vec<String> = Vec::new();
    for rule in &rules {
        if !seen.insert(rule.id.as_str()) {
            duplicates.push(rule.id.clone());
        }
    }
    for id in duplicates {
        diagnostics.push(diagnostic(
            DiagnosticCode::RuleValidationError,
            None,
            format!("duplicate rule id `{id}`"),
        ));
    }

    diagnostics.sort_by(|a, b| {
        (a.code.as_str(), a.file.as_deref(), a.note.as_deref()).cmp(&(
            b.code.as_str(),
            b.file.as_deref(),
            b.note.as_deref(),
        ))
    });

    RuleSet {
        languages,
        rules,
        diagnostics,
    }
}

/// Load and compile every language section.
fn load_languages(
    root: &Path,
    diagnostics: &mut Vec<skillmap_core::Diagnostic>,
) -> BTreeMap<String, LoadedLanguage> {
    let path = root.join("rules").join("languages.toml");
    let mut loaded = BTreeMap::new();

    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) => {
            diagnostics.push(diagnostic(
                DiagnosticCode::RuleLoadError,
                Some("rules/languages.toml".to_owned()),
                format!("cannot read: {error}"),
            ));
            return loaded;
        }
    };

    let configs: BTreeMap<String, LanguageConfig> = match toml::from_str(&text) {
        Ok(configs) => configs,
        Err(error) => {
            diagnostics.push(diagnostic(
                DiagnosticCode::RuleLoadError,
                Some("rules/languages.toml".to_owned()),
                format!("invalid TOML: {error}"),
            ));
            return loaded;
        }
    };

    for (name, config) in configs {
        let Some(grammar) = grammar(&name) else {
            diagnostics.push(diagnostic(
                DiagnosticCode::RuleValidationError,
                Some("rules/languages.toml".to_owned()),
                format!(
                    "language `{name}` has no compiled grammar; add the tree-sitter \
                     dependency and register it before adding a section"
                ),
            ));
            continue;
        };

        let query_path = root.join(&config.reachability);
        let source = match std::fs::read_to_string(&query_path) {
            Ok(source) => source,
            Err(error) => {
                diagnostics.push(diagnostic(
                    DiagnosticCode::RuleLoadError,
                    Some(config.reachability.clone()),
                    format!("cannot read reachability query: {error}"),
                ));
                continue;
            }
        };

        match Query::new(&grammar, &source) {
            Ok(reachability) => {
                loaded.insert(
                    name.clone(),
                    LoadedLanguage {
                        name,
                        config,
                        grammar,
                        reachability,
                    },
                );
            }
            Err(error) => diagnostics.push(diagnostic(
                DiagnosticCode::RuleValidationError,
                Some(config.reachability.clone()),
                format!("reachability query does not compile: {error}"),
            )),
        }
    }

    loaded
}

/// Every `.toml` under `dir` except `languages.toml`, sorted.
fn rule_files(dir: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut work = vec![dir.to_path_buf()];

    while let Some(current) = work.pop() {
        let Ok(entries) = std::fs::read_dir(&current) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                work.push(path);
            } else if path.extension().is_some_and(|ext| ext == "toml")
                && path
                    .file_name()
                    .is_some_and(|name| name != "languages.toml")
            {
                found.push(path);
            }
        }
    }

    found.sort();
    found
}

/// Load, validate, and compile one rule file.
fn load_rule(
    root: &Path,
    path: &Path,
    languages: &BTreeMap<String, LoadedLanguage>,
) -> Result<CompiledRule, (DiagnosticCode, String)> {
    let text = std::fs::read_to_string(path).map_err(|error| {
        (
            DiagnosticCode::RuleLoadError,
            format!("cannot read: {error}"),
        )
    })?;
    let file: RuleFile = toml::from_str(&text).map_err(|error| {
        (
            DiagnosticCode::RuleLoadError,
            format!("invalid TOML: {error}"),
        )
    })?;

    let language = languages.get(&file.language).ok_or_else(|| {
        (
            DiagnosticCode::RuleValidationError,
            format!(
                "unknown language `{}`; add a section to rules/languages.toml",
                file.language
            ),
        )
    })?;

    // Tier and claim are validated together: this is where invariant 5 is
    // enforced, before a finding of the wrong shape can ever be constructed.
    let claim = match file.tier.as_str() {
        "proven" => CapabilityTerm::ALL
            .iter()
            .copied()
            .find(|term| term.as_str() == file.capability)
            .map(Claim::Capability)
            .ok_or_else(|| {
                (
                    DiagnosticCode::RuleValidationError,
                    format!(
                        "`{}` is not in the capability taxonomy; adding a term is a \
                         schema-version event, not a rule PR",
                        file.capability
                    ),
                )
            })?,
        "pattern" => InstructionSignal::ALL
            .iter()
            .copied()
            .find(|signal| signal.as_str() == file.capability)
            .map(Claim::Instruction)
            .ok_or_else(|| {
                (
                    DiagnosticCode::RuleValidationError,
                    format!(
                        "`{}` is not an instruction-plane signal; a `pattern` rule \
                         cannot name a capability term (invariant 5)",
                        file.capability
                    ),
                )
            })?,
        other => {
            return Err((
                DiagnosticCode::RuleValidationError,
                format!("unknown tier `{other}`; expected `proven` or `pattern`"),
            ))
        }
    };

    let query_source = std::fs::read_to_string(root.join(&file.query)).map_err(|error| {
        (
            DiagnosticCode::RuleLoadError,
            format!("cannot read query {}: {error}", file.query),
        )
    })?;
    let query = Query::new(&language.grammar, &query_source).map_err(|error| {
        (
            DiagnosticCode::RuleValidationError,
            format!("query {} does not compile: {error}", file.query),
        )
    })?;

    let roles = validate_captures(&file, &query)?;

    Ok(CompiledRule {
        id: file.id,
        language: file.language,
        claim,
        query,
        roles,
        match_data: file.match_data,
        docs: file.docs,
    })
}

/// Check `[captures]` against the query, **in both directions**.
///
/// A capture the query emits that the TOML never declares is a rule silently
/// dropping information on the floor — exactly how a detection quietly stops
/// firing. A capture the TOML declares that the query never emits is a rule that
/// looks like it covers something it does not. Both are errors, not warnings.
fn validate_captures(
    file: &RuleFile,
    query: &Query,
) -> Result<BTreeMap<Role, u32>, (DiagnosticCode, String)> {
    let emitted: BTreeSet<&str> = query
        .capture_names()
        .iter()
        .copied()
        // `@_`-prefixed captures are query-local: they exist to drive `#eq?` and
        // `#match?` predicates and are invisible to the engine by design.
        .filter(|name| !name.starts_with('_'))
        .collect();

    let mut roles = BTreeMap::new();
    let mut declared: BTreeSet<&str> = BTreeSet::new();

    for (key, capture) in &file.captures {
        let role = Role::parse(key).ok_or_else(|| {
            (
                DiagnosticCode::RuleValidationError,
                format!(
                    "`{key}` is not an engine role; expected one of {}. Adding a role \
                     is an engine change and a schema-version event.",
                    Role::ALL
                        .iter()
                        .map(|role| role.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            )
        })?;

        let name = capture.strip_prefix('@').unwrap_or(capture);
        declared.insert(name);

        let index = query.capture_index_for_name(name).ok_or_else(|| {
            (
                DiagnosticCode::RuleValidationError,
                format!(
                    "role `{key}` maps to capture `@{name}`, which the query never \
                     produces"
                ),
            )
        })?;
        roles.insert(role, index);
    }

    if !roles.contains_key(&Role::Site) {
        return Err((
            DiagnosticCode::RuleValidationError,
            "every rule must declare a `site` capture: it is the evidence span, and \
             a finding nobody can point at cannot be regression-tested (invariant 4)"
                .to_owned(),
        ));
    }

    let undeclared: Vec<&str> = emitted
        .iter()
        .copied()
        .filter(|name| !declared.contains(name))
        .collect();
    if !undeclared.is_empty() {
        return Err((
            DiagnosticCode::RuleValidationError,
            format!(
                "query produces capture(s) {} that [captures] never declares; prefix \
                 them `@_` if they are query-local, or map them to a role",
                undeclared
                    .iter()
                    .map(|name| format!("@{name}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        ));
    }

    Ok(roles)
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "a failed assertion in a test is the test failing"
)]
mod tests {
    use super::*;

    fn repo_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
    }

    #[test]
    fn roles_round_trip() {
        for role in Role::ALL {
            assert_eq!(Role::parse(role.as_str()), Some(*role));
        }
        assert_eq!(Role::parse("sink"), None);
    }

    #[test]
    fn the_repository_loads_cleanly() {
        // The reference rule triple is the contract. If it stops loading, the
        // documented shape in docs/03-rules-authoring.md is a lie.
        let set = load(&repo_root());
        assert!(
            set.diagnostics.is_empty(),
            "the shipped rules must load without diagnostics: {:?}",
            set.diagnostics
        );
        assert!(set.languages.contains_key("python"));
        assert_eq!(set.rules.len(), 1);

        let rule = &set.rules[0];
        assert_eq!(rule.id, "py.credential-read.dotfile");
        assert_eq!(
            rule.claim,
            Claim::Capability(CapabilityTerm::FsReadCredential)
        );
        assert!(rule.capture_index(Role::Site).is_some());
        assert!(rule.capture_index(Role::Path).is_some());
        assert!(rule.capture_index(Role::Dynamic).is_some());
        assert!(rule.capture_index(Role::Host).is_none());
        assert!(!rule.match_data.path_prefixes.is_empty());
    }

    #[test]
    fn python_extensions_resolve() {
        let set = load(&repo_root());
        assert_eq!(
            set.language_for_extension("py").map(|l| l.name.as_str()),
            Some("python")
        );
        assert!(set.language_for_extension("rb").is_none());
    }
}
