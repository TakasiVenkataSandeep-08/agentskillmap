//! The suites, and the expectations they check.
//!
//! Adversarial cases are **data**: each lives in
//! `fixtures/adversarial/<id>/` as a real bundle plus an `expect.toml` declaring
//! what must hold. Adding a red-team case is a directory, in the same spirit as
//! invariant 7 — nobody should have to edit Rust to attack the scanner.

use crate::pipeline;
use serde::Deserialize;
use skillmap_core::Manifest;
use skillmap_rules::RuleSet;
use std::path::{Path, PathBuf};

/// Why a case cannot run yet.
///
/// A declared-but-unrunnable case is tracked rather than deleted. `docs/05-eval.md`
/// lists eight adversarial cases as the minimum set; three of them need machinery
/// later tasks build, and pretending the suite is complete without them would
/// overstate coverage in exactly the way the project refuses to elsewhere.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Requirement {
    /// Everything this case needs exists.
    Available,
    /// Waits on a task that has not landed.
    Task(String),
    /// Waits on a capability term no rule detects yet.
    Rule(String),
    /// Waits on the corpus harvest.
    Corpus(String),
    /// Needs a live model call, which this suite deliberately never makes.
    ///
    /// Distinct from [`Requirement::Task`] because the distinction is the whole
    /// point: T7 landed, and this case still cannot run. Leaving it as
    /// `task = "T7"` after T7 shipped would misdescribe a permanent property of
    /// the suite as a temporary backlog item.
    ///
    /// The eval gate is offline (invariant 9) and deterministic (invariant 2).
    /// A case that called a model would be neither, and a case that called a
    /// *replay* provider would be asserting what the fixture author typed. The
    /// deterministic half of what this case claims — that the auditor is
    /// unaffected by the injection — is proved in
    /// `crates/skillmap-scan/tests/quarantine.rs`.
    Model(String),
}

impl Requirement {
    /// The reason this case is pending, or `None` if it can run.
    #[must_use]
    pub fn pending_reason(&self) -> Option<String> {
        match self {
            Self::Available => None,
            Self::Task(task) => Some(format!("needs {task}")),
            Self::Rule(term) => Some(format!("needs a rule detecting `{term}`")),
            Self::Corpus(what) => Some(format!("needs the T3 corpus: {what}")),
            Self::Model(what) => Some(format!("needs a live model: {what}")),
        }
    }
}

/// One thing that must be true of the analysed bundle.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Expectation {
    /// A capability must be reported, optionally with a given reachability.
    Capability {
        /// The taxonomy term.
        capability: String,
        /// Required reachability, if the case pins one.
        #[serde(default)]
        reachability: Option<String>,
    },
    /// An `unresolved` entry with this reason must be present.
    Unresolved {
        /// The reason code.
        reason: String,
    },
    /// A file must carry a given load phase.
    LoadPhase {
        /// Bundle-relative path.
        path: String,
        /// The expected phase.
        phase: String,
    },
    /// An instruction-plane signal must be reported.
    Instruction {
        /// The signal name.
        signal: String,
    },
    /// The bundle must gain this capability relative to the case's `lock.json`.
    ///
    /// The only expectation that is about a *pair* of states rather than one. It
    /// needs a `lock.json` beside `expect.toml` standing in for the previous
    /// release; a case declaring this without one fails rather than passing
    /// vacuously, because "no lock, so nothing to compare, so no failures" is
    /// exactly the silent-pass shape invariant 3 rejects.
    Escalation {
        /// The taxonomy term that must appear as newly added.
        capability: String,
    },
    /// No capability may be reported at all.
    NoCapability,
    /// No instruction signal may be reported at all.
    NoInstruction,
    /// The manifest must contain no verdict language and no score.
    ///
    /// Invariant 1, checked mechanically. `docs/05-eval.md` is explicit that the
    /// quiet cases matter as much as the loud ones: *"A scanner that cannot stay
    /// quiet on legitimate behaviour will not survive contact with users."*
    NoVerdict,
}

/// The `expect.toml` beside an adversarial bundle.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CaseFile {
    /// What `docs/05-eval.md` calls this case.
    description: String,
    /// Whether it can run.
    #[serde(default = "available")]
    requires: Requirement,
    /// What must hold.
    expect: Vec<Expectation>,
}

fn available() -> Requirement {
    Requirement::Available
}

/// A declared case.
#[derive(Debug)]
pub struct Case {
    /// Directory name.
    pub id: String,
    /// Human description.
    pub description: String,
    /// Whether it can run.
    pub requires: Requirement,
    /// What must hold.
    pub expect: Vec<Expectation>,
    /// Where the bundle lives.
    pub bundle: PathBuf,
    /// The case directory, which is where a `lock.json` would sit.
    pub dir: PathBuf,
}

/// What happened when a case ran.
#[derive(Debug)]
pub struct Outcome {
    /// The case id.
    pub id: String,
    /// Human description.
    pub description: String,
    /// Set when the case could not run.
    pub pending: Option<String>,
    /// Expectations that were not met.
    pub failures: Vec<String>,
}

impl Outcome {
    /// Whether this outcome should let the build pass.
    ///
    /// Pending cases are acceptable — they are declared future work, and failing
    /// the build on them would mean the suite could never be green until every
    /// task landed. They are still counted and printed.
    #[must_use]
    pub fn acceptable(&self) -> bool {
        self.pending.is_some() || self.failures.is_empty()
    }
}

/// Load every declared adversarial case.
#[must_use]
pub fn load_cases(root: &Path) -> Vec<Case> {
    let mut found = Vec::new();
    let Ok(entries) = std::fs::read_dir(crate::adversarial_dir(root)) else {
        return found;
    };

    for entry in entries.flatten() {
        let dir = entry.path();
        if !dir.is_dir() {
            continue;
        }
        let Some(id) = dir.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let Ok(text) = std::fs::read_to_string(dir.join("expect.toml")) else {
            continue;
        };
        let Ok(file) = toml::from_str::<CaseFile>(&text) else {
            continue;
        };
        found.push(Case {
            id: id.to_owned(),
            description: file.description,
            requires: file.requires,
            expect: file.expect,
            bundle: dir.join("bundle"),
            dir,
        });
    }

    found.sort_by(|a, b| a.id.cmp(&b.id));
    found
}

/// Run the adversarial suite.
#[must_use]
pub fn run_adversarial_suite(root: &Path, rules: &RuleSet) -> Vec<Outcome> {
    load_cases(root)
        .into_iter()
        .map(|case| {
            if let Some(reason) = case.requires.pending_reason() {
                return Outcome {
                    id: case.id,
                    description: case.description,
                    pending: Some(reason),
                    failures: Vec::new(),
                };
            }

            let failures = match pipeline::analyze(&case.bundle, rules) {
                Ok(manifest) => check(&case.expect, &manifest, &case.dir),
                Err(error) => vec![format!("the bundle could not be analysed: {error}")],
            };

            Outcome {
                id: case.id,
                description: case.description,
                pending: None,
                failures,
            }
        })
        .collect()
}

/// Compare the analysed bundle against the case's `lock.json`.
///
/// This is the only expectation that needs T8's machinery, and it is the reason
/// `capability-added-in-update` sat pending from T6 until now: the bundle on disk
/// is v1.1, `lock.json` is what v1.0 recorded, and the case asserts that the
/// difference between them is reported as *added* rather than merely present.
fn check_escalation(capability: &str, manifest: &Manifest, dir: &Path) -> Vec<String> {
    let path = dir.join("lock.json");
    let Ok(text) = std::fs::read_to_string(&path) else {
        return vec![format!(
            "this case expects an escalation but there is no {}. With no previous \
             state there is nothing to compare against, and a case that cannot \
             fail is worse than no case at all.",
            path.display()
        )];
    };

    let lock = match skillmap_diff::Lock::from_json(&text) {
        Ok(lock) => lock,
        Err(error) => {
            return vec![format!(
                "{} is not a readable lock: {error}",
                path.display()
            )]
        }
    };

    let delta = skillmap_diff::diff(&lock, std::slice::from_ref(manifest));
    let added: Vec<&str> = delta
        .escalations()
        .iter()
        .filter_map(|change| match change {
            skillmap_diff::Change::CapabilityAdded { capability, .. } => Some(capability.as_str()),
            _ => None,
        })
        .collect();

    if added.contains(&capability) {
        return Vec::new();
    }

    // A root mismatch produces BundleAdded rather than CapabilityAdded, which
    // looks like a detection failure and is not one. Say which it is.
    if delta
        .changes
        .iter()
        .any(|change| matches!(change, skillmap_diff::Change::BundleAdded { .. }))
    {
        return vec![format!(
            "lock.json keys bundles {:?} but the scan produced `{}`; the diff sees \
             an unrelated bundle rather than an update to this one",
            lock.bundles.keys().collect::<Vec<_>>(),
            manifest.target.root
        )];
    }

    vec![format!(
        "expected `{capability}` to be reported as newly added against lock.json; \
         the diff reports {added:?}"
    )]
}

/// Check every expectation against an analysed manifest.
///
/// `dir` is the case directory, needed only by [`Expectation::Escalation`],
/// which compares the manifest against the `lock.json` sitting beside it.
fn check(expectations: &[Expectation], manifest: &Manifest, dir: &Path) -> Vec<String> {
    let mut failures = Vec::new();

    for expectation in expectations {
        match expectation {
            Expectation::Capability {
                capability,
                reachability,
            } => {
                let found = manifest
                    .capabilities
                    .iter()
                    .find(|entry| entry.capability.as_str() == capability);
                match found {
                    None => failures.push(format!(
                        "expected capability `{capability}`, but the manifest reports {:?}",
                        manifest
                            .capabilities
                            .iter()
                            .map(|entry| entry.capability.as_str())
                            .collect::<Vec<_>>()
                    )),
                    Some(entry) => {
                        if let Some(expected) = reachability {
                            if entry.reachability.as_str() != expected {
                                failures.push(format!(
                                    "`{capability}` reported reachability `{}`, expected `{expected}`",
                                    entry.reachability.as_str()
                                ));
                            }
                        }
                    }
                }
            }
            Expectation::Unresolved { reason } => {
                if !manifest
                    .unresolved
                    .iter()
                    .any(|entry| entry.reason.as_str() == reason)
                {
                    failures.push(format!(
                        "expected an `unresolved` entry with reason `{reason}`; silence here \
                         is the invariant 3 failure this case exists to catch"
                    ));
                }
            }
            Expectation::LoadPhase { path, phase } => {
                match manifest.inventory.iter().find(|entry| &entry.path == path) {
                    None => failures.push(format!("`{path}` is not in the inventory")),
                    Some(entry) => {
                        if entry.load_phase.as_str() != phase {
                            failures.push(format!(
                                "`{path}` is `{}`, expected `{phase}`",
                                entry.load_phase.as_str()
                            ));
                        }
                    }
                }
            }
            Expectation::Instruction { signal } => {
                if !manifest
                    .instructions
                    .iter()
                    .any(|entry| entry.signal.as_str() == signal)
                {
                    failures.push(format!("expected instruction signal `{signal}`"));
                }
            }
            Expectation::Escalation { capability } => {
                failures.extend(check_escalation(capability, manifest, dir));
            }
            Expectation::NoCapability => {
                if !manifest.capabilities.is_empty() {
                    failures.push(format!(
                        "expected no capability, got {:?}. Staying quiet on legitimate \
                         behaviour is what determines whether this tool survives use.",
                        manifest
                            .capabilities
                            .iter()
                            .map(|entry| entry.capability.as_str())
                            .collect::<Vec<_>>()
                    ));
                }
            }
            Expectation::NoInstruction => {
                if !manifest.instructions.is_empty() {
                    failures.push(format!(
                        "expected no instruction signal, got {:?}",
                        manifest
                            .instructions
                            .iter()
                            .map(|entry| entry.signal.as_str())
                            .collect::<Vec<_>>()
                    ));
                }
            }
            Expectation::NoVerdict => failures.extend(verdict_language(manifest)),
        }
    }

    failures
}

/// Words and shapes invariant 1 forbids from ever reaching a manifest.
///
/// Checked on the serialized artifact rather than the struct, because that is
/// what a user reads and what CI diffs. A `note` field is the likeliest place for
/// a verdict to sneak in, since it is free text.
fn verdict_language(manifest: &Manifest) -> Vec<String> {
    let Ok(json) = manifest.to_canonical_json() else {
        return vec!["the manifest could not be serialized".to_owned()];
    };
    let lowered = json.to_lowercase();

    const FORBIDDEN: &[&str] = &[
        "malicious",
        "suspicious",
        "dangerous",
        "unsafe",
        "risk_score",
        "severity",
        "\"grade\"",
        "threat_level",
        "\"safe\"",
    ];

    let mut failures: Vec<String> = FORBIDDEN
        .iter()
        .filter(|word| lowered.contains(**word))
        .map(|word| {
            format!(
                "the manifest contains verdict language {word:?}. Invariant 1: emit \
                 capabilities with evidence, never a judgement — half of what this \
                 tool flags is legitimate."
            )
        })
        .collect();

    // A float is a score wearing a disguise, and the schema has no float anywhere.
    //
    // Detected by walking the parsed value, not by scanning the text. A textual
    // scan flags `"version": "0.1.0"` and every semver string in the document —
    // which it did, on first run. The question is whether a JSON *number* is
    // fractional, and only the parser can answer that.
    match serde_json::from_str::<serde_json::Value>(&json) {
        Ok(value) if contains_float(&value) => {
            failures.push("the manifest contains a float, which is a score in disguise".to_owned())
        }
        Ok(_) => {}
        Err(error) => failures.push(format!("the manifest is not valid JSON: {error}")),
    }

    failures
}

/// Whether any JSON number in `value` is fractional.
fn contains_float(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::Number(number) => number.is_f64(),
        serde_json::Value::Array(items) => items.iter().any(contains_float),
        serde_json::Value::Object(map) => map.values().any(contains_float),
        _ => false,
    }
}

/// Run every rule's fixtures — the fast per-rule suite.
///
/// `docs/05-eval.md`: *"Fast — must stay under a few seconds so nobody is tempted
/// to skip it."*
#[must_use]
pub fn run_fixture_suite(root: &Path, rules: &RuleSet) -> Vec<Outcome> {
    let mut outcomes = Vec::new();
    let fixtures = root.join("fixtures");

    // Driven by the languages the ruleset knows, not by whatever directories
    // happen to sit under `fixtures/`. This was a blocklist — skip `bundles/` and
    // `adversarial/` — until T8 added `fixtures/projects/`, whose two version
    // directories were promptly read as languages with no rule fixtures and
    // failed the suite. A blocklist has to be updated by whoever adds the next
    // directory; asking the ruleset what a language is cannot fall out of date.
    let mut dirs: Vec<PathBuf> = Vec::new();
    for language in rules.languages.keys() {
        if let Ok(rules_dir) = std::fs::read_dir(fixtures.join(language)) {
            dirs.extend(
                rules_dir
                    .flatten()
                    .map(|entry| entry.path())
                    .filter(|path| path.is_dir()),
            );
        }
    }
    dirs.sort();

    for dir in dirs {
        let id = dir
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("?")
            .to_owned();
        let mut failures = Vec::new();

        for (stem, must_fire) in [("positive", true), ("negative", false)] {
            let Some(path) = fixture_file(&dir, stem) else {
                failures.push(format!("no {stem} fixture: invariant 8"));
                continue;
            };
            let Ok(text) = std::fs::read_to_string(&path) else {
                failures.push(format!("{stem} fixture could not be read"));
                continue;
            };
            let name = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or(stem)
                .to_owned();

            let fired = fires(&name, &text, rules);
            if fired != must_fire {
                failures.push(if must_fire {
                    format!("{stem} fixture did not fire")
                } else {
                    format!("{stem} fixture fired when it must not")
                });
            }
        }

        outcomes.push(Outcome {
            id: format!("fixture/{id}"),
            description: format!("rule fixtures for {id}"),
            pending: None,
            failures,
        });
    }

    outcomes
}

/// Whether either plane produces a finding for one fixture file.
fn fires(name: &str, text: &str, rules: &RuleSet) -> bool {
    let code = skillmap_code::analyze(
        &[skillmap_code::SourceFile {
            path: name,
            text,
            entered: true,
        }],
        rules,
    );
    let instructions =
        skillmap_instr::analyze(&[skillmap_instr::ProseFile { path: name, text }], rules);
    !code.capabilities.is_empty() || !code.unresolved.is_empty() || !instructions.is_empty()
}

/// The file in `dir` whose stem is `stem`.
fn fixture_file(dir: &Path, stem: &str) -> Option<PathBuf> {
    std::fs::read_dir(dir).ok()?.flatten().find_map(|entry| {
        let path = entry.path();
        (path.file_stem().is_some_and(|found| found == stem)).then_some(path)
    })
}
