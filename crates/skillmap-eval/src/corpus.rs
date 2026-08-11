//! The corpus suite: scoring the scanner against ground truth.
//!
//! `docs/05-eval.md`'s second suite, and the one that had never run. Its inputs
//! are `corpus/labels.toml` — bundles a human read and judged — and
//! `corpus/raw/`, which only exists on a machine that ran the harvest.
//!
//! # Every number here is per stratum and per term. There is no aggregate.
//!
//! Not an oversight. *"A tool with 0.94 aggregate precision that misses every
//! `net.fetch_then_execute` is not a good tool, and the aggregate hides it."*
//! The same argument applies to strata: the sample deliberately over-represents
//! bundles carrying credential markers, so any pooled rate would be a number
//! about the sampling design rather than about the ecosystem.
//!
//! # Small n is reported as small n
//!
//! A labelling pass is expensive and this one is partial. Point estimates from
//! forty bundles look identical to point estimates from four thousand, and only
//! one of them means anything, so every rate carries a **Wilson 95% interval**
//! and every table prints its denominator. A zero false-positive count over
//! thirty bundles has an upper bound near 11%: that is the honest reading, and
//! quoting "0%" instead would be the single most misleading number this
//! repository could publish.
//!
//! # Unlabelled is not clean
//!
//! Bundles in the sample with no label are counted and reported as unlabelled.
//! Folding them into a denominator as though they had been checked and found
//! fine is invariant 3's failure, one level up from the manifest.

use serde::Deserialize;
use skillmap_core::Manifest;
use skillmap_rules::RuleSet;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// What a labeller concluded about one bundle.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Label {
    /// Content digest, matching `corpus/sample.json` and the directory under
    /// `corpus/raw/`.
    pub digest: String,
    /// Which stratum it was drawn from.
    pub stratum: String,
    /// `labelled`, or a reason it could not be.
    pub verdict: Verdict,
    /// Capability terms genuinely present, judged from the source.
    ///
    /// Empty is a real answer and the common one. Only terms named in
    /// [`Labels::terms_labelled`] are scored; anything else here is recorded
    /// but not counted, because a term the labeller did not look for
    /// exhaustively cannot support a recall number.
    #[serde(default)]
    pub capabilities: Vec<String>,
    /// `path:line` for each claim above, so a label can be checked.
    #[serde(default)]
    pub evidence: Vec<String>,
    /// Whether the deep files ask for behaviour the description does not
    /// disclose. The input to `docs/04-semantic-layer.md`'s cut criterion.
    #[serde(default)]
    pub disclosure_delta: bool,
    /// What the bundle does, in a sentence. Describes patterns, not people.
    #[serde(default)]
    pub note: String,
}

/// Whether a bundle could be labelled at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    /// Read and judged.
    Labelled,
    /// Too large to read within the labelling budget.
    TooLarge,
    /// Present in the sample but not in the local archive.
    Missing,
}

/// The parsed `corpus/labels.toml`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Labels {
    /// Corpus snapshot these labels describe.
    pub snapshot: String,
    /// Seed of the sample they were drawn from.
    pub sample_seed: String,
    /// Who labelled. Published, because a single-annotator corpus is a weaker
    /// artifact than a reviewed one and the reader has to be able to tell.
    pub labeller: String,
    /// Who reviewed. Empty means nobody has.
    #[serde(default)]
    pub reviewed_by: String,
    /// Every term a label is allowed to name.
    pub vocabulary: Vec<String>,
    /// Terms the labeller looked for **exhaustively**, and therefore the only
    /// terms precision and recall are computed for.
    ///
    /// The distinction matters. A labeller who noted `net.egress` when it was
    /// obvious, but did not hunt for it, has produced usable notes and unusable
    /// recall — every bundle they did not think to check would count as a false
    /// negative against the scanner.
    pub terms_labelled: Vec<String>,
    /// One entry per labelled bundle.
    #[serde(default, rename = "label")]
    pub labels: Vec<Label>,
}

/// Why a label file could not be used.
///
/// Boxed: `toml::de::Error` is large, and an unboxed variant makes every
/// `Result` in this module the size of its worst case.
#[derive(Debug)]
pub enum Error {
    /// Not present. The corpus suite is skipped, not failed: the labels are
    /// research output and a fresh clone has none.
    Absent(PathBuf),
    /// Present and unreadable.
    Io(PathBuf, std::io::Error),
    /// Present and malformed.
    Parse(PathBuf, Box<toml::de::Error>),
    /// A label names a term outside the vocabulary, or `terms_labelled` names
    /// one. A typo scores silently as a miss, so it is rejected.
    UnknownTerm(String),
    /// Two labels for one bundle.
    Duplicate(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Absent(path) => write!(f, "{} does not exist", path.display()),
            Self::Io(path, error) => write!(f, "cannot read {}: {error}", path.display()),
            Self::Parse(path, error) => write!(f, "{} is malformed: {error}", path.display()),
            Self::UnknownTerm(term) => write!(
                f,
                "`{term}` is not in the label vocabulary; a term nothing recognises \
                 scores as a silent miss"
            ),
            Self::Duplicate(digest) => {
                write!(f, "{digest} is labelled twice; which one wins is arbitrary")
            }
        }
    }
}

impl Labels {
    /// Read and validate `corpus/labels.toml`.
    ///
    /// # Errors
    ///
    /// [`Error`] for a missing, unreadable, malformed, or self-inconsistent file.
    pub fn load(path: &Path) -> Result<Self, Error> {
        if !path.is_file() {
            return Err(Error::Absent(path.to_path_buf()));
        }
        let text =
            std::fs::read_to_string(path).map_err(|error| Error::Io(path.to_path_buf(), error))?;
        let labels: Self =
            toml::from_str(&text).map_err(|error| Error::Parse(path.to_path_buf(), Box::new(error)))?;

        let vocabulary: BTreeSet<&str> = labels.vocabulary.iter().map(String::as_str).collect();
        for term in &labels.terms_labelled {
            if !vocabulary.contains(term.as_str()) {
                return Err(Error::UnknownTerm(term.clone()));
            }
        }

        let mut seen = BTreeSet::new();
        for label in &labels.labels {
            if !seen.insert(label.digest.clone()) {
                return Err(Error::Duplicate(label.digest.clone()));
            }
            for term in &label.capabilities {
                if !vocabulary.contains(term.as_str()) {
                    return Err(Error::UnknownTerm(term.clone()));
                }
            }
        }
        Ok(labels)
    }
}

/// A proportion with a Wilson 95% interval.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rate {
    /// Numerator.
    pub hits: usize,
    /// Denominator.
    pub total: usize,
    /// Lower bound of the 95% interval.
    pub low: f64,
    /// Upper bound of the 95% interval.
    pub high: f64,
}

impl Rate {
    /// Compute a rate and its Wilson score interval.
    ///
    /// Wilson rather than the normal approximation, because the normal one is
    /// wrong exactly where this suite lives: small n, and proportions near zero.
    /// At 0 hits in 30 it produces an interval of zero width — the number that
    /// would let this project publish "0% false positives" and mean nothing by
    /// it.
    #[must_use]
    pub fn new(hits: usize, total: usize) -> Self {
        if total == 0 {
            return Self {
                hits,
                total,
                low: 0.0,
                high: 1.0,
            };
        }
        #[allow(
            clippy::cast_precision_loss,
            reason = "sample sizes here are in the hundreds; this is a reported ratio"
        )]
        let (hits_f, total_f) = (hits as f64, total as f64);

        let z = 1.959_963_984_540_054_f64; // two-sided 95%
        let p = hits_f / total_f;
        let denominator = 1.0 + z * z / total_f;
        let centre = (p + z * z / (2.0 * total_f)) / denominator;
        let spread =
            z * (p * (1.0 - p) / total_f + z * z / (4.0 * total_f * total_f)).sqrt() / denominator;

        Self {
            hits,
            total,
            low: (centre - spread).max(0.0),
            high: (centre + spread).min(1.0),
        }
    }

    /// The point estimate, or `None` when nothing was observed.
    ///
    /// `None` rather than `0.0`: a rate over an empty denominator is not zero,
    /// it is unknown, and the two must not print the same.
    #[must_use]
    pub fn point(&self) -> Option<f64> {
        if self.total == 0 {
            return None;
        }
        #[allow(
            clippy::cast_precision_loss,
            reason = "sample sizes here are in the hundreds; this is a reported ratio"
        )]
        Some(self.hits as f64 / self.total as f64)
    }

    /// `x/y (p%, 95% CI a–b)`, or an explicit "no data".
    #[must_use]
    pub fn render(&self) -> String {
        match self.point() {
            None => "0/0 (no labelled bundles)".to_owned(),
            Some(point) => format!(
                "{}/{} ({:.1}%, 95% CI {:.1}–{:.1}%)",
                self.hits,
                self.total,
                point * 100.0,
                self.low * 100.0,
                self.high * 100.0
            ),
        }
    }
}

/// Precision and recall for one capability term.
#[derive(Debug, Clone)]
pub struct TermScore {
    /// The term.
    pub term: String,
    /// Bundles where the scanner reported it and the label agreed.
    pub true_positive: usize,
    /// Reported and not in the label.
    pub false_positive: usize,
    /// In the label and not reported.
    pub false_negative: usize,
    /// Of everything reported, how much was right.
    pub precision: Rate,
    /// Of everything genuinely present, how much was found.
    pub recall: Rate,
}

/// What one run of the corpus suite produced.
#[derive(Debug, Clone)]
pub struct Report {
    /// Corpus snapshot.
    pub snapshot: String,
    /// Who labelled, and whether anybody reviewed.
    pub labeller: String,
    /// Empty when unreviewed.
    pub reviewed_by: String,
    /// Bundles scanned and scored.
    pub scored: usize,
    /// Labelled but absent from the local archive, so unscoreable.
    pub unscoreable: usize,
    /// In the sample and carrying no label at all.
    pub unlabelled: usize,
    /// Per term, for terms in `terms_labelled` only.
    pub terms: Vec<TermScore>,
    /// Per stratum: bundles where the scanner reported a term the label did not
    /// contain. On `code_clean` this is the headline metric.
    pub false_positive_rate: BTreeMap<String, Rate>,
    /// Fraction of scored bundles carrying at least one `unresolved` entry.
    ///
    /// `docs/05-eval.md`: *"This number going up on a release is acceptable if it
    /// reflects newly-honest reporting; it going quietly down while recall is
    /// flat means something is being silently dropped."*
    pub unresolved_rate: Rate,
    /// Labelled bundles judged to have a real disclosure delta, per stratum.
    /// The input to `docs/04-semantic-layer.md`'s cut criterion.
    pub disclosure_delta: BTreeMap<String, Rate>,
}

/// Where a labelled bundle's bytes live.
#[must_use]
pub fn bundle_dir(corpus: &Path, digest: &str) -> PathBuf {
    corpus
        .join("raw")
        .join(digest.split_once(':').map_or(digest, |(_, hex)| hex))
}

/// Run the corpus suite.
///
/// Scans every labelled bundle present in the archive and scores it against its
/// label. `sample_size` is the number of bundles drawn, so the report can say
/// how much of the sample has been labelled rather than only reporting what has.
#[must_use]
pub fn run(labels: &Labels, corpus: &Path, rules: &RuleSet, sample_size: usize) -> Report {
    let scored_terms: BTreeSet<&str> = labels.terms_labelled.iter().map(String::as_str).collect();

    let mut scanned: Vec<(&Label, Manifest)> = Vec::new();
    let mut unscoreable = 0;

    for label in &labels.labels {
        if label.verdict != Verdict::Labelled {
            unscoreable += 1;
            continue;
        }
        let dir = bundle_dir(corpus, &label.digest);
        match skillmap_scan::analyze(&dir, rules) {
            Ok(manifest) => scanned.push((label, manifest)),
            // A bundle that cannot be scanned is not a bundle that scanned
            // clean. It leaves the denominator and is counted.
            Err(_) => unscoreable += 1,
        }
    }

    let mut terms = Vec::new();
    for term in &labels.terms_labelled {
        let mut true_positive = 0;
        let mut false_positive = 0;
        let mut false_negative = 0;

        for (label, manifest) in &scanned {
            let reported = manifest
                .capabilities
                .iter()
                .any(|entry| entry.capability.as_str() == term);
            let truth = label.capabilities.iter().any(|candidate| candidate == term);
            match (reported, truth) {
                (true, true) => true_positive += 1,
                (true, false) => false_positive += 1,
                (false, true) => false_negative += 1,
                (false, false) => {}
            }
        }

        terms.push(TermScore {
            term: term.clone(),
            true_positive,
            false_positive,
            false_negative,
            precision: Rate::new(true_positive, true_positive + false_positive),
            recall: Rate::new(true_positive, true_positive + false_negative),
        });
    }

    // Per stratum: did the scanner report any scored term the label did not
    // contain? Only scored terms count — a capability nobody labelled
    // exhaustively cannot be called a false positive.
    let mut per_stratum: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    let mut delta: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    for (label, manifest) in &scanned {
        let entry = per_stratum.entry(label.stratum.clone()).or_insert((0, 0));
        entry.1 += 1;
        let spurious = manifest.capabilities.iter().any(|reported| {
            let term = reported.capability.as_str();
            scored_terms.contains(term) && !label.capabilities.iter().any(|truth| truth == term)
        });
        if spurious {
            entry.0 += 1;
        }

        let seen = delta.entry(label.stratum.clone()).or_insert((0, 0));
        seen.1 += 1;
        if label.disclosure_delta {
            seen.0 += 1;
        }
    }

    let unresolved_hits = scanned
        .iter()
        .filter(|(_, manifest)| !manifest.unresolved.is_empty())
        .count();

    Report {
        snapshot: labels.snapshot.clone(),
        labeller: labels.labeller.clone(),
        reviewed_by: labels.reviewed_by.clone(),
        scored: scanned.len(),
        unscoreable,
        unlabelled: sample_size.saturating_sub(labels.labels.len()),
        terms,
        false_positive_rate: per_stratum
            .into_iter()
            .map(|(stratum, (hits, total))| (stratum, Rate::new(hits, total)))
            .collect(),
        unresolved_rate: Rate::new(unresolved_hits, scanned.len()),
        disclosure_delta: delta
            .into_iter()
            .map(|(stratum, (hits, total))| (stratum, Rate::new(hits, total)))
            .collect(),
    }
}

/// Render a report for a human, and for the README.
#[must_use]
pub fn render(report: &Report) -> String {
    use std::fmt::Write as _;
    let mut out = String::new();

    let _ = writeln!(
        out,
        "corpus suite — snapshot {}, {} bundle(s) scored, {} unscoreable, {} unlabelled",
        report.snapshot, report.scored, report.unscoreable, report.unlabelled
    );
    let _ = writeln!(
        out,
        "  labelled by {}{}",
        report.labeller,
        if report.reviewed_by.is_empty() {
            ", UNREVIEWED — single annotator, inter-annotator agreement unmeasured".to_owned()
        } else {
            format!(", reviewed by {}", report.reviewed_by)
        }
    );

    if report.scored == 0 {
        let _ = writeln!(
            out,
            "\n  nothing scored. The labels name bundles this machine does not have:\n  \
             corpus/raw/ is gitignored and only exists where the harvest ran."
        );
        return out;
    }

    let _ = writeln!(
        out,
        "\n  per capability term (only terms labelled exhaustively):"
    );
    for term in &report.terms {
        let _ = writeln!(out, "    {}", term.term);
        let _ = writeln!(out, "      precision  {}", term.precision.render());
        let _ = writeln!(out, "      recall     {}", term.recall.render());
        let _ = writeln!(
            out,
            "      tp {} / fp {} / fn {}",
            term.true_positive, term.false_positive, term.false_negative
        );
    }

    let _ = writeln!(
        out,
        "\n  false-positive rate per stratum (code_clean is the headline):"
    );
    for (stratum, rate) in &report.false_positive_rate {
        let _ = writeln!(out, "    {stratum:<18} {}", rate.render());
    }

    let _ = writeln!(
        out,
        "\n  unresolved rate    {}",
        report.unresolved_rate.render()
    );

    let _ = writeln!(
        out,
        "\n  disclosure delta per stratum (docs/04-semantic-layer.md's cut criterion):"
    );
    for (stratum, rate) in &report.disclosure_delta {
        let _ = writeln!(out, "    {stratum:<18} {}", rate.render());
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

    #[test]
    fn a_zero_count_does_not_produce_a_zero_width_interval() {
        // The number that would let this project publish "0% false positives"
        // from thirty bundles and mean nothing by it.
        let rate = Rate::new(0, 30);
        assert_eq!(rate.point(), Some(0.0));
        assert!(
            rate.high > 0.10 && rate.high < 0.12,
            "upper bound was {:.3}; Wilson at 0/30 is ~0.113",
            rate.high
        );
        assert!(rate.render().contains("95% CI"));
    }

    #[test]
    fn more_evidence_narrows_the_interval() {
        let small = Rate::new(0, 30);
        let large = Rate::new(0, 3000);
        assert!(large.high < small.high / 10.0);
    }

    #[test]
    fn an_empty_denominator_is_unknown_rather_than_zero() {
        let rate = Rate::new(0, 0);
        assert_eq!(rate.point(), None);
        assert!(rate.render().contains("no labelled bundles"));
        assert_eq!(rate.high, 1.0, "nothing observed bounds nothing");
    }

    #[test]
    fn a_half_rate_brackets_a_half() {
        let rate = Rate::new(50, 100);
        assert!((rate.point().unwrap() - 0.5).abs() < 1e-9);
        assert!(rate.low < 0.5 && rate.high > 0.5);
        assert!(rate.low > 0.39 && rate.high < 0.61, "{rate:?}");
    }

    fn labels_toml(body: &str) -> String {
        format!(
            "snapshot = \"2026-08\"\n\
             sample_seed = \"s\"\n\
             labeller = \"someone\"\n\
             vocabulary = [\"fs.read.credential\", \"net.egress\"]\n\
             terms_labelled = [\"fs.read.credential\"]\n{body}"
        )
    }

    fn write(dir: &Path, text: &str) -> PathBuf {
        let path = dir.join("labels.toml");
        std::fs::write(&path, text).unwrap();
        path
    }

    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("skillmap-labels-{}-{name}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn a_term_outside_the_vocabulary_is_rejected() {
        // A typo'd term scores as a silent miss against the scanner, which is
        // the worst possible failure for a ground-truth file.
        let dir = scratch("vocab");
        let path = write(
            &dir,
            &labels_toml(
                "[[label]]\ndigest = \"sha256:aa\"\nstratum = \"code_clean\"\n\
                 verdict = \"labelled\"\ncapabilities = [\"fs.read.credentials\"]\n",
            ),
        );
        assert!(matches!(Labels::load(&path), Err(Error::UnknownTerm(_))));
    }

    #[test]
    fn a_duplicate_label_is_rejected() {
        let dir = scratch("dupe");
        let entry =
            "[[label]]\ndigest = \"sha256:aa\"\nstratum = \"code_clean\"\nverdict = \"labelled\"\n";
        let path = write(&dir, &labels_toml(&format!("{entry}{entry}")));
        assert!(matches!(Labels::load(&path), Err(Error::Duplicate(_))));
    }

    #[test]
    fn an_absent_file_is_distinguishable_from_a_broken_one() {
        // A fresh clone has no labels, and that must skip the suite rather than
        // fail the build — but it must not look like an empty label set either.
        let missing = scratch("absent").join("nope.toml");
        assert!(matches!(Labels::load(&missing), Err(Error::Absent(_))));
    }

    #[test]
    fn unlabelled_bundles_are_counted_not_assumed_clean() {
        let dir = scratch("partial");
        let path = write(
            &dir,
            &labels_toml(
                "[[label]]\ndigest = \"sha256:aa\"\nstratum = \"code_clean\"\nverdict = \"missing\"\n",
            ),
        );
        let labels = Labels::load(&path).unwrap();
        let rules = RuleSet {
            languages: BTreeMap::new(),
            rules: Vec::new(),
            diagnostics: Vec::new(),
        };
        let report = run(&labels, &dir, &rules, 130);

        assert_eq!(report.scored, 0);
        assert_eq!(report.unscoreable, 1);
        assert_eq!(
            report.unlabelled, 129,
            "129 of the 130 sampled bundles carry no label and must be said to"
        );
        assert!(render(&report).contains("nothing scored"));
    }

    #[test]
    fn the_render_always_names_the_labeller_and_review_state() {
        // A single-annotator corpus is a weaker artifact than a reviewed one and
        // every number printed from it has to carry that.
        let dir = scratch("who");
        let path = write(&dir, &labels_toml(""));
        let labels = Labels::load(&path).unwrap();
        let rules = RuleSet {
            languages: BTreeMap::new(),
            rules: Vec::new(),
            diagnostics: Vec::new(),
        };
        let rendered = render(&run(&labels, &dir, &rules, 130));
        assert!(rendered.contains("someone"), "{rendered}");
        assert!(rendered.contains("UNREVIEWED"), "{rendered}");
    }
}
