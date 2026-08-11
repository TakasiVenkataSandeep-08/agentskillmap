#![warn(missing_docs)]

//! The eval harness — three suites, per-capability metrics, and the CI gate.
//!
//! `docs/05-eval.md` opens with the reason this exists: *"Eval is a shipped
//! artifact, not a phase. It is the operational definition of 'not a stub'."*
//! Without measured numbers the quality claim is unfalsifiable and the project is
//! indistinguishable from a regex script with good documentation.
//!
//! # A suite that skips cases silently is worse than no suite
//!
//! Several adversarial cases `docs/05-eval.md` specifies cannot run yet: the
//! semantic pass is T7, the diff is T8, and two capability terms have no rules.
//! The harness therefore does not *omit* them — it declares all of them, runs
//! what it can, and reports the rest as **pending** with the reason. A green
//! suite that quietly covered five of eight cases would be exactly the false
//! comfort invariant 3 exists to prevent, one level up: absence of failures is
//! only meaningful next to a complete list of what was not attempted.
//!
//! # What is blocked
//!
//! The corpus suite and the published numbers both need T3's harvest, which has
//! not run. There is no labeled corpus, so there is no held-out split, no
//! precision/recall against ground truth, and no false-positive rate on a benign
//! stratum — which `docs/05-eval.md` names as the headline metric. This crate
//! measures the fixture and adversarial suites, which are real but small, and
//! says so everywhere the number appears.

pub mod cases;
pub mod corpus;
pub mod metrics;

/// Manifest assembly, which now lives in `skillmap-scan`.
///
/// Re-exported under the old path because `skillmap ci` needed the same
/// function and a product binary must not depend on the test harness to get it.
pub use skillmap_scan as pipeline;

pub use cases::{Case, Expectation, Outcome, Requirement};
pub use metrics::{Baseline, Metrics, Regression};

use std::path::{Path, PathBuf};

/// Everything the harness produces in one run.
#[derive(Debug)]
pub struct Report {
    /// Per-rule fixture results.
    pub fixtures: Vec<Outcome>,
    /// Adversarial case results, including the ones that could not run.
    pub adversarial: Vec<Outcome>,
    /// Aggregate numbers over what actually ran.
    pub metrics: Metrics,
}

impl Report {
    /// Whether every case that *could* run did so successfully.
    ///
    /// Pending cases do not fail the build — they are declared work, not
    /// regressions — but they are counted and printed so the number of things
    /// this suite does not yet check is always visible.
    #[must_use]
    pub fn passed(&self) -> bool {
        self.fixtures
            .iter()
            .chain(&self.adversarial)
            .all(Outcome::acceptable)
    }

    /// Cases that could not run, with their reasons.
    #[must_use]
    pub fn pending(&self) -> Vec<&Outcome> {
        self.fixtures
            .iter()
            .chain(&self.adversarial)
            .filter(|outcome| outcome.pending.is_some())
            .collect()
    }

    /// Cases that ran and failed.
    #[must_use]
    pub fn failures(&self) -> Vec<&Outcome> {
        self.fixtures
            .iter()
            .chain(&self.adversarial)
            .filter(|outcome| !outcome.acceptable())
            .collect()
    }
}

/// Run every suite that can run.
///
/// # Errors
///
/// [`Error`] only if the rule set itself cannot be loaded — that is a fault in
/// this repository, not a measurement.
pub fn run(root: &Path) -> Result<Report, Error> {
    let rules = skillmap_rules::load(root);
    if !rules.diagnostics.is_empty() {
        return Err(Error::Rules(format!(
            "the rule set does not load cleanly, so no measurement is meaningful: {:?}",
            rules.diagnostics
        )));
    }

    let fixtures = cases::run_fixture_suite(root, &rules);
    let adversarial = cases::run_adversarial_suite(root, &rules);
    let metrics = metrics::measure(&fixtures, &adversarial);

    Ok(Report {
        fixtures,
        adversarial,
        metrics,
    })
}

/// Where the harness reads and writes.
#[must_use]
pub fn adversarial_dir(root: &Path) -> PathBuf {
    root.join("fixtures").join("adversarial")
}

/// The committed baseline the CI gate compares against.
#[must_use]
pub fn baseline_path(root: &Path) -> PathBuf {
    root.join("eval").join("baseline.json")
}

/// Failures that stop a measurement being meaningful.
#[derive(Debug)]
pub enum Error {
    /// The rule set could not be loaded.
    Rules(String),
    /// A file could not be read or written.
    Io {
        /// The path involved.
        path: PathBuf,
        /// Why.
        source: std::io::Error,
    },
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Rules(message) => write!(f, "{message}"),
            Self::Io { path, source } => write!(f, "{}: {source}", path.display()),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Rules(_) => None,
        }
    }
}
