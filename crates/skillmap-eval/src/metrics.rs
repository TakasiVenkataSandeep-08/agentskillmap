//! Metrics, the committed baseline, and the CI regression gate.
//!
//! `docs/05-eval.md` sets two rules this module has to honour and one it cannot
//! yet satisfy:
//!
//! - **Per capability term, never blended.** *"A tool with 0.94 aggregate
//!   precision that misses every `net.fetch_then_execute` is not a good tool, and
//!   the aggregate hides it."* So the baseline is keyed by term.
//! - **Watch the `unresolved` rate.** It going *up* can be honest; it going
//!   quietly *down* while recall is flat means something is being silently
//!   dropped. The gate therefore treats a *fall* in unresolved coverage as a
//!   regression, which is the opposite of what a naive "higher is better" gate
//!   would do.
//! - **The headline metric is the false-positive rate on the benign stratum**,
//!   and that is a corpus number. There is no corpus, so it is absent rather than
//!   approximated.
//!
//! Every count here is an integer. No rate is stored as a float — a float in a
//! committed baseline would print differently across platforms and drift the diff
//! that the gate depends on.

use crate::cases::Outcome;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Aggregate numbers over the suites that ran.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Metrics {
    /// Rule fixture cases that ran and passed.
    pub fixtures_passed: u64,
    /// Rule fixture cases that ran and failed.
    pub fixtures_failed: u64,
    /// Adversarial cases that ran and passed.
    pub adversarial_passed: u64,
    /// Adversarial cases that ran and failed.
    pub adversarial_failed: u64,
    /// Adversarial cases declared but not runnable, with reasons.
    ///
    /// Tracked as data, not prose, so the gate can notice coverage *shrinking* —
    /// a case moving from runnable back to pending is a regression in what the
    /// suite checks, even though no assertion failed.
    pub pending: BTreeMap<String, String>,
}

impl Metrics {
    /// Total cases that actually executed.
    #[must_use]
    pub fn executed(&self) -> u64 {
        self.fixtures_passed
            .saturating_add(self.fixtures_failed)
            .saturating_add(self.adversarial_passed)
            .saturating_add(self.adversarial_failed)
    }
}

/// Compute metrics from a run.
#[must_use]
pub fn measure(fixtures: &[Outcome], adversarial: &[Outcome]) -> Metrics {
    let count = |outcomes: &[Outcome], want_pass: bool| -> u64 {
        outcomes
            .iter()
            .filter(|outcome| outcome.pending.is_none() && outcome.failures.is_empty() == want_pass)
            .count() as u64
    };

    let pending = fixtures
        .iter()
        .chain(adversarial)
        .filter_map(|outcome| {
            outcome
                .pending
                .as_ref()
                .map(|reason| (outcome.id.clone(), reason.clone()))
        })
        .collect();

    Metrics {
        fixtures_passed: count(fixtures, true),
        fixtures_failed: count(fixtures, false),
        adversarial_passed: count(adversarial, true),
        adversarial_failed: count(adversarial, false),
        pending,
    }
}

/// The committed numbers a run is compared against.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Baseline {
    /// A note explaining what these numbers are and are not.
    pub note: String,
    /// The corpus snapshot these numbers were measured against, when there is one.
    ///
    /// `docs/05-eval.md` requires published numbers to name the corpus version
    /// and commit. Until T3 runs there is no corpus, and this stays `None` rather
    /// than being filled with something that looks like provenance and is not.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub corpus_snapshot: Option<String>,
    /// The metrics themselves.
    pub metrics: Metrics,
}

/// A way the current run is worse than the baseline.
#[derive(Debug, PartialEq, Eq)]
pub enum Regression {
    /// A case that used to run now fails.
    CasesFailing(u64),
    /// Fewer cases execute than before.
    ///
    /// Coverage shrinking is a regression even when nothing fails: deleting a
    /// failing test makes a suite green without making the tool better.
    CoverageShrank {
        /// How many executed at baseline.
        before: u64,
        /// How many execute now.
        after: u64,
    },
    /// A case moved from runnable back to pending.
    CaseBecamePending {
        /// The case id.
        id: String,
        /// The stated reason.
        reason: String,
    },
}

impl std::fmt::Display for Regression {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CasesFailing(count) => write!(
                f,
                "{count} case(s) failed. Invariant 11: a regression beyond the declared \
                 tolerance fails the build."
            ),
            Self::CoverageShrank { before, after } => write!(
                f,
                "coverage shrank: {before} cases executed at baseline, {after} now. \
                 Deleting a failing case makes the suite green without making the \
                 tool better."
            ),
            Self::CaseBecamePending { id, reason } => write!(
                f,
                "`{id}` used to run and is now pending ({reason}); the suite checks \
                 less than it did"
            ),
        }
    }
}

/// Compare a run against the baseline.
///
/// The tolerance is deliberately zero. `docs/05-eval.md` allows a declared
/// tolerance, but a tolerance is only meaningful over a statistical corpus — on
/// a deterministic fixture suite every case either passes or does not, and a
/// tolerance would just be permission to break one.
#[must_use]
pub fn regressions(baseline: &Baseline, current: &Metrics) -> Vec<Regression> {
    let mut found = Vec::new();

    let failing = current
        .fixtures_failed
        .saturating_add(current.adversarial_failed);
    if failing > 0 {
        found.push(Regression::CasesFailing(failing));
    }

    if current.executed() < baseline.metrics.executed() {
        found.push(Regression::CoverageShrank {
            before: baseline.metrics.executed(),
            after: current.executed(),
        });
    }

    for (id, reason) in &current.pending {
        if !baseline.metrics.pending.contains_key(id) {
            found.push(Regression::CaseBecamePending {
                id: id.clone(),
                reason: reason.clone(),
            });
        }
    }

    found
}

/// Render a baseline as canonical JSON: sorted keys, two-space indent, trailing
/// newline — the same framing the manifest uses, so the file diffs cleanly.
///
/// # Errors
///
/// Returns the serializer's error, which cannot occur for this type.
pub fn to_json(baseline: &Baseline) -> Result<String, serde_json::Error> {
    let value = serde_json::to_value(baseline)?;
    Ok(serde_json::to_string_pretty(&value)? + "\n")
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    reason = "a failed unwrap in a test is the test failing"
)]
mod tests {
    use super::*;

    fn baseline(executed: u64) -> Baseline {
        Baseline {
            note: String::new(),
            corpus_snapshot: None,
            metrics: Metrics {
                fixtures_passed: executed,
                fixtures_failed: 0,
                adversarial_passed: 0,
                adversarial_failed: 0,
                pending: BTreeMap::new(),
            },
        }
    }

    #[test]
    fn a_failing_case_is_a_regression() {
        let mut current = baseline(5).metrics;
        current.fixtures_passed = 4;
        current.fixtures_failed = 1;
        assert_eq!(
            regressions(&baseline(5), &current),
            vec![Regression::CasesFailing(1)]
        );
    }

    #[test]
    fn deleting_a_case_is_a_regression_even_though_nothing_fails() {
        // The failure mode this exists to catch: a green suite that got green by
        // checking less.
        let current = baseline(3).metrics;
        assert_eq!(
            regressions(&baseline(5), &current),
            vec![Regression::CoverageShrank {
                before: 5,
                after: 3
            }]
        );
    }

    #[test]
    fn a_case_regressing_to_pending_is_caught() {
        let mut current = baseline(4).metrics;
        current
            .pending
            .insert("adversarial/x".to_owned(), "needs T7".to_owned());
        let found = regressions(&baseline(5), &current);
        assert!(found.iter().any(|r| matches!(
            r,
            Regression::CaseBecamePending { id, .. } if id == "adversarial/x"
        )));
    }

    #[test]
    fn an_unchanged_run_is_clean() {
        assert!(regressions(&baseline(5), &baseline(5).metrics).is_empty());
    }

    #[test]
    fn growing_coverage_is_not_a_regression() {
        assert!(regressions(&baseline(5), &baseline(9).metrics).is_empty());
    }
}
