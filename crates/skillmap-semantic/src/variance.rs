//! Running the same input n times and reporting how much the answer moves.
//!
//! `docs/04-semantic-layer.md`: *"Non-determinism is expected here. Run each
//! eval item n times, report variance, and treat a high-variance finding kind as
//! not ready to ship."*
//!
//! This is the only honest way to publish anything about a model pass sitting
//! next to two deterministic tiers. A single run tells you what the model said
//! once. The deterministic branches are byte-identical across a thousand
//! shuffles and two platforms, and putting an unmeasured branch beside them
//! without saying how much it wobbles invites readers to assume it is as solid —
//! which is exactly the "poisoning by association" invariant 6 is guarding
//! against.
//!
//! **This harness has not been run against a live model.** The numbers it
//! produces are the deliverable, and inventing them would be the failure this
//! project is defined against. See `docs/00-tasks.md`, T7.

use crate::{analyze, BundleView, Limits, Provider};
use skillmap_core::{Advisory, AdvisoryKind};
use std::collections::{BTreeMap, BTreeSet};

/// How one finding kind behaved across n runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KindVariance {
    /// The kind.
    pub kind: AdvisoryKind,
    /// Runs in which at least one finding of this kind appeared.
    pub runs_present: usize,
    /// Distinct `(file, line, claim)` triples seen across all runs.
    pub distinct_findings: usize,
    /// Findings that appeared in **every** run.
    pub stable_findings: usize,
}

impl KindVariance {
    /// Fraction of runs in which this kind appeared at all, in `[0, 1]`.
    ///
    /// `0.0` or `1.0` is a kind that behaves; anything in between is a coin
    /// flip a reviewer would experience as the tool changing its mind.
    #[must_use]
    pub fn presence_rate(&self, runs: usize) -> f64 {
        if runs == 0 {
            return 0.0;
        }
        #[allow(
            clippy::cast_precision_loss,
            reason = "run counts are small; this is a reported ratio, never a manifest value"
        )]
        {
            self.runs_present as f64 / runs as f64
        }
    }

    /// Fraction of distinct findings that appeared in every run.
    ///
    /// The number `docs/04-semantic-layer.md` means by "treat a high-variance
    /// finding kind as not ready to ship": `1.0` is a kind that says the same
    /// thing every time.
    #[must_use]
    pub fn stability(&self) -> f64 {
        if self.distinct_findings == 0 {
            return 1.0;
        }
        #[allow(
            clippy::cast_precision_loss,
            reason = "finding counts are small; this is a reported ratio, never a manifest value"
        )]
        {
            self.stable_findings as f64 / self.distinct_findings as f64
        }
    }
}

/// The result of n runs over one bundle.
#[derive(Debug, Clone)]
pub struct Report {
    /// How many times the pass ran.
    pub runs: usize,
    /// Runs whose model call failed or whose output failed validation.
    pub runs_failed: usize,
    /// Per kind, never blended. `disclosure_delta` and `injection_attempt` have
    /// very different base rates, and one number over both hides that.
    pub kinds: Vec<KindVariance>,
}

impl Report {
    /// The least stable kind, if any findings appeared at all.
    #[must_use]
    pub fn worst(&self) -> Option<&KindVariance> {
        self.kinds
            .iter()
            .min_by(|a, b| a.stability().total_cmp(&b.stability()))
    }
}

/// Identity of a finding for the purpose of "is this the same finding".
///
/// `(kind, file, line, claim)`. Claim text is included deliberately: two runs
/// that cite the same line for different reasons have not agreed, and a
/// variance number that treated them as one finding would overstate stability.
type Identity = (&'static str, String, u64, String);

/// Run the pass `runs` times and report how much the answer moved.
///
/// Callers pass a provider that really calls a model; a [`crate::provider::Replay`]
/// will report perfect stability, which is true and uninteresting.
#[must_use]
pub fn measure(
    bundle: &BundleView,
    provider: &dyn Provider,
    limits: &Limits,
    runs: usize,
) -> Report {
    let mut seen: BTreeMap<Identity, usize> = BTreeMap::new();
    let mut present: BTreeMap<&'static str, usize> = BTreeMap::new();
    let mut failed = 0;

    for _ in 0..runs {
        let outcome = analyze(bundle, provider, limits);
        let Advisory::Enabled(run) = outcome.advisory else {
            failed += 1;
            continue;
        };
        if !outcome.diagnostics.is_empty() {
            failed += 1;
        }

        let mut kinds_this_run: BTreeSet<&'static str> = BTreeSet::new();
        for finding in &run.findings {
            kinds_this_run.insert(finding.kind.as_str());
            let first = finding.evidence.first();
            let identity: Identity = (
                finding.kind.as_str(),
                first.map(|e| e.file.clone()).unwrap_or_default(),
                first.map_or(0, |e| e.start_line.get()),
                finding.claim.clone(),
            );
            *seen.entry(identity).or_insert(0) += 1;
        }
        for kind in kinds_this_run {
            *present.entry(kind).or_insert(0) += 1;
        }
    }

    let mut kinds = Vec::new();
    for kind in AdvisoryKind::ALL {
        let name = kind.as_str();
        let mine: Vec<usize> = seen
            .iter()
            .filter(|((seen_kind, _, _, _), _)| *seen_kind == name)
            .map(|(_, count)| *count)
            .collect();

        // A kind that never appeared is omitted rather than reported as
        // perfectly stable at zero, which would read as a measurement.
        if mine.is_empty() && present.get(name).copied().unwrap_or(0) == 0 {
            continue;
        }

        kinds.push(KindVariance {
            kind: *kind,
            runs_present: present.get(name).copied().unwrap_or(0),
            distinct_findings: mine.len(),
            stable_findings: mine.iter().filter(|count| **count == runs).count(),
        });
    }

    Report {
        runs,
        runs_failed: failed,
        kinds,
    }
}

/// Render a report for a human.
#[must_use]
pub fn render(report: &Report) -> String {
    use std::fmt::Write as _;

    let mut out = String::new();
    let _ = writeln!(
        out,
        "advisory variance over {} run(s), {} failed",
        report.runs, report.runs_failed
    );

    if report.kinds.is_empty() {
        let _ = writeln!(out, "  no findings of any kind appeared");
        return out;
    }

    for kind in &report.kinds {
        let _ = writeln!(
            out,
            "  {:<24} present {:>3}/{:<3}  distinct {:>3}  stable {:>3}  stability {:.2}",
            kind.kind.as_str(),
            kind.runs_present,
            report.runs,
            kind.distinct_findings,
            kind.stable_findings,
            kind.stability()
        );
    }
    out
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "a failed assertion in a test is the test failing"
)]
mod tests {
    use super::*;
    use crate::{provider::Replay, FileView};

    fn bundle() -> BundleView {
        BundleView {
            description: "Summarises notes.".to_owned(),
            files: vec![FileView {
                path: "SKILL.md".to_owned(),
                text: "a\nb\nc\n".to_owned(),
            }],
        }
    }

    fn one_finding() -> String {
        r#"{"findings":[{"kind":"disclosure_delta","claim":"Asks for a credential the description omits.","evidence":[{"file":"SKILL.md","start_line":2}]}]}"#.to_owned()
    }

    #[test]
    fn a_provider_that_never_changes_its_mind_reports_full_stability() {
        let report = measure(
            &bundle(),
            &Replay::new("replay/fixed", &one_finding()),
            &Limits::default(),
            5,
        );
        assert_eq!(report.runs, 5);
        assert_eq!(report.runs_failed, 0);
        assert_eq!(report.kinds.len(), 1);
        assert_eq!(report.kinds[0].stability(), 1.0);
        assert_eq!(report.kinds[0].presence_rate(5), 1.0);
    }

    #[test]
    fn kinds_that_never_appeared_are_omitted_not_reported_as_stable() {
        // Reporting `injection_attempt: stability 1.00` for a kind that never
        // fired would read as a measurement of something. It is a measurement
        // of nothing.
        let report = measure(&bundle(), &Replay::silent(), &Limits::default(), 3);
        assert!(report.kinds.is_empty());
        assert!(render(&report).contains("no findings of any kind"));
    }

    #[test]
    fn a_failing_provider_is_counted_rather_than_ignored() {
        let report = measure(
            &bundle(),
            &crate::provider::Unavailable,
            &Limits::default(),
            4,
        );
        assert_eq!(report.runs_failed, 4);
        assert!(report.kinds.is_empty());
    }

    #[test]
    fn the_report_names_its_worst_kind() {
        let report = measure(
            &bundle(),
            &Replay::new("replay/fixed", &one_finding()),
            &Limits::default(),
            2,
        );
        assert_eq!(report.worst().unwrap().kind, AdvisoryKind::DisclosureDelta);
    }
}
