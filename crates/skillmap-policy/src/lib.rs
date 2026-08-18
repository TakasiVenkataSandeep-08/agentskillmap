#![warn(missing_docs)]

//! `policy.toml` — where judgement lives, and the exit codes CI reads.
//!
//! Invariant 1 keeps verdicts out of the manifest: *"Half the flagged
//! capabilities are legitimate. A build skill needs `process.exec`. A deploy
//! skill needs `net.egress`."* The scanner therefore describes and stops.
//!
//! This is the other half of that argument. Whether `fs.read.credential` is
//! acceptable is a question only a specific repository can answer, and the answer
//! differs between a credential-manager skill and a markdown formatter. So the
//! judgement is a file the repository owns, reviewed like any other config, and
//! the scanner never guesses at it.
//!
//! # Two independent questions
//!
//! A CI run asks two things, and conflating them would make both useless:
//!
//! 1. **Is this capability allowed here at all?** Answered by the allowlist.
//! 2. **Is this capability new?** Answered by the diff against `skillmap.lock`.
//!
//! A skill can hold an allowed capability it did not have yesterday — that is an
//! escalation worth a human look even though the capability is permitted. And a
//! skill can hold a disallowed capability it has held all along, which the diff
//! would never mention. Distinct exit codes, so a consumer can tell them apart.

use serde::Deserialize;
use skillmap_core::Manifest;
use skillmap_diff::Delta;
use std::collections::{BTreeMap, BTreeSet};

/// The parsed `policy.toml`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Policy {
    /// Capabilities accepted from any bundle in this repository.
    #[serde(default)]
    pub allow: Allow,
    /// Per-bundle additions, keyed by `target.root`.
    ///
    /// Additive only: a bundle entry widens what that bundle may do and can
    /// never narrow the repository default. Narrowing would let a policy file
    /// grant something globally and quietly retract it in one place, which is
    /// how an allowlist becomes unreadable.
    #[serde(default)]
    pub bundle: BTreeMap<String, Allow>,
    /// What to fail on beyond capability escalation.
    #[serde(default)]
    pub review: Review,
}

/// Failures a repository opts into, over and above capability escalation.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Review {
    /// Fail when a bundle's bytes change and **no code in it could be read**.
    ///
    /// Off by default, and the default is the load-bearing decision. Nine
    /// published skills in ten ship no file any grammar covers, so turning this
    /// on globally would fail CI on every routine prose edit — the failure mode
    /// `hook.rs` argues at length, where a check that fights the author gets
    /// switched off and takes the real detections with it.
    ///
    /// On, it closes the gap an external review named: for those nine in ten the
    /// capability diff is silent because nothing looked, and a lock entry that
    /// is only a digest gives a reader nothing a checksum would not. This makes
    /// the digest mean something — *these bytes changed and no analysis saw
    /// them* — which is a claim worth a human's eye on a skill an agent runs.
    ///
    /// Repositories that vendor a small set of prose skills and want every edit
    /// reviewed are the case this is for. A repository tracking hundreds of
    /// third-party skills should leave it off and rely on escalation.
    #[serde(default)]
    pub unanalysed_content_changes: bool,
}

/// A set of permitted capability terms.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Allow {
    /// Capability terms, as they appear in the manifest.
    #[serde(default)]
    pub capabilities: Vec<String>,
}

/// Why a policy file could not be used.
#[derive(Debug)]
pub enum Error {
    /// The file could not be read.
    Io(std::io::Error),
    /// The file is not valid TOML, or carries keys this build does not know.
    Parse(toml::de::Error),
    /// The file names a capability term that is not in the taxonomy.
    ///
    /// Rejected rather than ignored: a typo'd term in an allowlist silently
    /// permits nothing, so the capability it was meant to allow fails CI and the
    /// reason is invisible. Better to fail on the file than on the scan.
    UnknownCapability {
        /// The term as written.
        term: String,
        /// Where it appeared — `allow`, or a bundle root.
        section: String,
    },
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(f, "cannot read policy.toml: {error}"),
            Self::Parse(error) => write!(f, "policy.toml is not valid: {error}"),
            Self::UnknownCapability { term, section } => write!(
                f,
                "policy.toml [{section}] allows `{term}`, which is not a capability \
                 term. A term that does not exist permits nothing, so the capability \
                 it was meant to allow would fail CI with no visible reason."
            ),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Parse(error) => Some(error),
            Self::UnknownCapability { .. } => None,
        }
    }
}

impl Policy {
    /// Parse a policy from TOML, rejecting unknown capability terms.
    ///
    /// # Errors
    ///
    /// [`Error::Parse`] for malformed TOML, [`Error::UnknownCapability`] for a
    /// term outside the closed taxonomy.
    pub fn parse(text: &str) -> Result<Self, Error> {
        let policy: Self = toml::from_str(text).map_err(Error::Parse)?;

        let known: BTreeSet<&str> = skillmap_core::CapabilityTerm::ALL
            .iter()
            .map(|term| term.as_str())
            .collect();

        let mut sections: Vec<(&str, &Allow)> = vec![("allow", &policy.allow)];
        sections.extend(
            policy
                .bundle
                .iter()
                .map(|(root, allow)| (root.as_str(), allow)),
        );

        for (section, allow) in sections {
            for term in &allow.capabilities {
                if !known.contains(term.as_str()) {
                    return Err(Error::UnknownCapability {
                        term: term.clone(),
                        section: section.to_owned(),
                    });
                }
            }
        }
        Ok(policy)
    }

    /// Load a policy from disk. **A missing file is `None`, not an empty policy.**
    ///
    /// The distinction is invariant 3 applied to configuration: an absent file is
    /// an absent opinion, and a present empty one is the opinion "nothing is
    /// permitted". Collapsing them costs a repository dearly in the same
    /// direction each way — treat absent as permissive and the check silently
    /// approves everything; treat absent as restrictive and the first run in
    /// every repository that has not written a policy fails on every capability
    /// it already had, which teaches people the check cries wolf.
    ///
    /// So an absent policy means the policy check does not run, and callers say
    /// so out loud. The escalation check against the lock still does.
    ///
    /// # Errors
    ///
    /// [`Error`] if the file exists but cannot be read or parsed.
    pub fn load(path: &std::path::Path) -> Result<Option<Self>, Error> {
        match std::fs::read_to_string(path) {
            Ok(text) => Self::parse(&text).map(Some),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(Error::Io(error)),
        }
    }

    /// Whether `capability` is permitted for the bundle at `root`.
    #[must_use]
    pub fn permits(&self, root: &str, capability: &str) -> bool {
        self.allow
            .capabilities
            .iter()
            .any(|term| term == capability)
            || self
                .bundle
                .get(root)
                .is_some_and(|allow| allow.capabilities.iter().any(|term| term == capability))
    }
}

/// A capability a manifest reports that the policy does not permit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Violation {
    /// Bundle root.
    pub root: String,
    /// The term that is not allowed.
    pub capability: String,
    /// Where it was found, for a reviewer to check.
    pub file: String,
    /// 1-indexed line.
    pub line: u64,
    /// The rule that fired.
    pub rule_id: String,
}

/// Every capability across `manifests` that `policy` does not permit.
#[must_use]
pub fn violations(policy: &Policy, manifests: &[Manifest]) -> Vec<Violation> {
    let mut found: Vec<Violation> = manifests
        .iter()
        .flat_map(|manifest| {
            manifest.capabilities.iter().filter_map(|capability| {
                let term = capability.capability.as_str();
                if policy.permits(&manifest.target.root, term) {
                    return None;
                }
                let evidence = capability.evidence.first()?;
                Some(Violation {
                    root: manifest.target.root.clone(),
                    capability: term.to_owned(),
                    file: evidence.file.clone(),
                    line: evidence.start_line.get(),
                    rule_id: evidence.rule_id.clone(),
                })
            })
        })
        .collect();
    found.sort_by(|a, b| (&a.root, &a.capability, &a.file).cmp(&(&b.root, &b.capability, &b.file)));
    found
}

/// Render violations the way [`skillmap_diff::render`] renders escalations —
/// one header per bundle, one indented line per finding, each carrying the file,
/// the line and the rule so a reviewer can open the right place immediately.
#[must_use]
pub fn render(violations: &[Violation]) -> String {
    use std::fmt::Write as _;

    let mut out = String::new();
    let mut current = "";
    for violation in violations {
        if violation.root != current {
            current = &violation.root;
            let _ = writeln!(out, "✗ {current}  capability not allowed by policy.toml");
        }
        let _ = writeln!(
            out,
            "    ! {}   {}:{}   {}",
            violation.capability, violation.file, violation.line, violation.rule_id
        );
    }
    out
}

/// What a CI run concluded, and the exit code that carries it.
///
/// Distinct codes rather than a single failure, so a consumer can branch. A
/// repository mid-migration may want to fail on escalation and only warn on
/// policy, and it cannot do that if both exit 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// Nothing new, nothing disallowed.
    Clean,
    /// A bundle gained capability it did not have in the lock.
    Escalation,
    /// A capability is present that the policy does not permit.
    PolicyViolation,
    /// Both.
    Both,
}

impl Outcome {
    /// The process exit code.
    ///
    /// `0` clean, `1` escalation, `2` policy violation, `3` both. Configuration
    /// errors exit `4` and are raised by the caller, because "the tool could not
    /// run" must never be mistaken for "the tool ran and found nothing" —
    /// invariant 3, applied to the exit status.
    #[must_use]
    pub const fn exit_code(self) -> u8 {
        match self {
            Self::Clean => 0,
            Self::Escalation => 1,
            Self::PolicyViolation => 2,
            Self::Both => 3,
        }
    }

    /// The exit code reserved for a run that could not complete.
    pub const CONFIG_ERROR: u8 = 4;
}

/// Combine an escalation check and a policy check into one outcome.
#[must_use]
pub fn decide(delta: &Delta, violations: &[Violation]) -> Outcome {
    match (!delta.escalations().is_empty(), !violations.is_empty()) {
        (false, false) => Outcome::Clean,
        (true, false) => Outcome::Escalation,
        (false, true) => Outcome::PolicyViolation,
        (true, true) => Outcome::Both,
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    reason = "a failed unwrap in a test is the test failing"
)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_policy_permits_nothing() {
        // A file that exists and allows nothing is a real, restrictive opinion.
        // Absent is a different thing entirely — see the next test.
        let policy = Policy::parse("").unwrap();
        assert!(!policy.permits("any", "process.exec"));
    }

    #[test]
    fn an_absent_policy_is_none_rather_than_an_empty_one() {
        // If absent collapsed to Policy::default(), the first `skillmap ci` in
        // every repository without a policy.toml would fail on every capability
        // the repository already had. That is how a check gets muted.
        let missing = std::path::Path::new("policy.toml.does-not-exist");
        assert!(matches!(Policy::load(missing), Ok(None)));
    }

    #[test]
    fn a_bundle_entry_widens_the_repository_default() {
        let policy = Policy::parse(
            r#"
[allow]
capabilities = ["process.exec"]

[bundle."creds-manager"]
capabilities = ["fs.read.credential"]
"#,
        )
        .unwrap();

        assert!(policy.permits("anything", "process.exec"));
        assert!(policy.permits("creds-manager", "fs.read.credential"));
        assert!(
            !policy.permits("other", "fs.read.credential"),
            "a bundle entry must not leak to other bundles"
        );
        assert!(
            policy.permits("creds-manager", "process.exec"),
            "a bundle entry adds to the default, never replaces it"
        );
    }

    #[test]
    fn an_unknown_capability_term_is_rejected() {
        // A typo'd term permits nothing, so the capability it was meant to allow
        // fails CI with no visible cause. Failing on the file is far kinder.
        let error =
            Policy::parse("[allow]\ncapabilities = [\"fs.read.credentials\"]\n").unwrap_err();
        assert!(matches!(error, Error::UnknownCapability { .. }));
        assert!(error.to_string().contains("fs.read.credentials"));
    }

    #[test]
    fn unknown_keys_are_rejected() {
        assert!(Policy::parse("[allow]\ncapabilties = []\n").is_err());
    }

    #[test]
    fn a_violation_line_carries_everything_needed_to_open_the_file() {
        // The ten-second budget in T8's "done when" is spent reading this line.
        // If it does not name the file, the line and the rule, it is spent
        // hunting instead.
        let rendered = render(&[
            Violation {
                root: "example-skill".to_owned(),
                capability: "fs.read.credential".to_owned(),
                file: "scripts/collect.py".to_owned(),
                line: 17,
                rule_id: "py.credential-read.dotfile".to_owned(),
            },
            Violation {
                root: "example-skill".to_owned(),
                capability: "net.egress".to_owned(),
                file: "scripts/send.py".to_owned(),
                line: 4,
                rule_id: "py.net.egress".to_owned(),
            },
        ]);

        assert_eq!(
            rendered,
            "✗ example-skill  capability not allowed by policy.toml\n\
             \x20   ! fs.read.credential   scripts/collect.py:17   py.credential-read.dotfile\n\
             \x20   ! net.egress   scripts/send.py:4   py.net.egress\n"
        );
        assert_eq!(
            rendered.matches("✗ example-skill").count(),
            1,
            "one header per bundle, not one per finding"
        );
    }

    #[test]
    fn exit_codes_distinguish_the_two_questions() {
        assert_eq!(Outcome::Clean.exit_code(), 0);
        assert_eq!(Outcome::Escalation.exit_code(), 1);
        assert_eq!(Outcome::PolicyViolation.exit_code(), 2);
        assert_eq!(Outcome::Both.exit_code(), 3);
        assert_eq!(Outcome::CONFIG_ERROR, 4);

        // Every outcome has a distinct code, or a consumer cannot branch on them.
        let codes = [
            Outcome::Clean.exit_code(),
            Outcome::Escalation.exit_code(),
            Outcome::PolicyViolation.exit_code(),
            Outcome::Both.exit_code(),
            Outcome::CONFIG_ERROR,
        ];
        let mut unique = codes.to_vec();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(unique.len(), codes.len());
    }
}
