//! Invariant 11: *"Precision and recall against the labeled corpus are computed
//! per release and **published in the README**."*
//!
//! The publishing half had no gate. The README's `Measured` table is
//! hand-written prose, nothing recomputes it, and across the coverage work it
//! drifted three separate times — a precision total that was two rule-sets out
//! of date, an unresolved rate from before a rule changed it, a denominators
//! block that introduced itself as "the denominators" while listing six of
//! eight terms. Each was caught by a throwaway script, which is another way of
//! saying each could have been missed.
//!
//! The checks split by what CI can actually see:
//!
//! - **`corpus/labels.toml` is committed**, so the denominators, the term list
//!   and the table's internal arithmetic are verified on every push.
//! - **`corpus/raw/` is gitignored**, so comparing each rate against a computed
//!   report only runs where the archive exists. That test skips rather than
//!   passes vacuously when it does not, and says so — a green tick for a check
//!   that did not run is the failure mode this repository keeps naming.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "a failed assertion in a test is the test failing, which is the point"
)]

use skillmap_eval::corpus;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

/// The `Measured against ground truth` section, which is the part invariant 11
/// is about. Numbers elsewhere in the README describe the harvest, not the eval.
fn measured_section() -> String {
    let text = std::fs::read_to_string(repo_root().join("README.md"))
        .expect("the README must exist; invariant 11 publishes into it");
    let start = text
        .find("### Measured against ground truth")
        .expect("the README must carry a ground-truth section");
    text[start..].to_owned()
}

/// One `| `term` precision | 13/13 (100%, …) |` row.
#[derive(Debug)]
struct Row {
    term: String,
    kind: String,
    hits: usize,
    total: usize,
    percent: f64,
}

/// Parse the metric table without a regex crate.
///
/// Deliberately strict about shape: a row this cannot parse is invisible to
/// every assertion below, so a silently unparsed table would make the whole
/// file pass while checking nothing.
fn rows() -> Vec<Row> {
    let mut found = Vec::new();
    for line in measured_section().lines() {
        let line = line.trim();
        if !line.starts_with("| `") {
            continue;
        }
        let mut cells = line.trim_matches('|').split('|');
        let (Some(label), Some(value)) = (cells.next(), cells.next()) else {
            continue;
        };
        let label = label.trim();
        let Some((term, kind)) = label.rsplit_once(' ') else {
            continue;
        };
        if kind != "precision" && kind != "recall" {
            continue;
        }
        let term = term.trim().trim_matches('`');

        // `**13/18 (72.2%, 95% CI …)**` → hits, total, percent.
        let value = value.trim().trim_matches('*');
        let Some((fraction, rest)) = value.split_once(" (") else {
            continue;
        };
        let Some((hits, total)) = fraction.split_once('/') else {
            continue;
        };
        let percent = rest
            .split('%')
            .next()
            .and_then(|number| number.parse::<f64>().ok());
        let (Ok(hits), Ok(total), Some(percent)) =
            (hits.trim().parse(), total.trim().parse(), percent)
        else {
            continue;
        };
        found.push(Row {
            term: term.to_owned(),
            kind: kind.to_owned(),
            hits,
            total,
            percent,
        });
    }
    found
}

fn labels() -> Option<corpus::Labels> {
    match corpus::Labels::load(&repo_root().join("corpus/labels.toml")) {
        Ok(labels) => Some(labels),
        Err(corpus::Error::Absent(_)) => None,
        Err(error) => panic!("corpus/labels.toml is present and unusable: {error}"),
    }
}

#[test]
fn the_table_parses_at_all() {
    // The check that keeps the others honest. Every assertion below iterates
    // parsed rows, so a table this parser cannot read would leave them all
    // trivially satisfied — the "zero cases, all passing" shape.
    let rows = rows();
    assert!(
        rows.len() >= 8,
        "parsed only {} metric rows from the README; the table shape changed and \
         every other check in this file is now vacuous",
        rows.len()
    );
    for row in &rows {
        assert!(
            row.total > 0,
            "{} {} has a zero denominator",
            row.term,
            row.kind
        );
        assert!(
            row.hits <= row.total,
            "{} {} is impossible",
            row.term,
            row.kind
        );
    }
}

#[test]
fn every_scored_term_appears_in_the_table() {
    // Invariant 11 says the numbers are published. A term the corpus scores and
    // the README omits is unpublished, however good its rate is.
    let Some(labels) = labels() else { return };
    let rows = rows();

    let mut missing = Vec::new();
    for term in &labels.terms_labelled {
        for kind in ["precision", "recall"] {
            if !rows.iter().any(|row| &row.term == term && row.kind == kind) {
                missing.push(format!("{term} {kind}"));
            }
        }
    }
    assert!(
        missing.is_empty(),
        "these are scored by the corpus and absent from the README table: {missing:?}"
    );
}

#[test]
fn the_stated_precision_total_is_the_sum_of_the_rows() {
    // The figure that drifted twice, because it is the only one in the document
    // derived from the others and nothing recomputed it. Checkable with no
    // corpus at all: it is pure internal arithmetic over the table.
    let rows = rows();
    let hits: usize = rows
        .iter()
        .filter(|row| row.kind == "precision")
        .map(|row| row.hits)
        .sum();
    let total: usize = rows
        .iter()
        .filter(|row| row.kind == "precision")
        .map(|row| row.total)
        .sum();

    let section = measured_section();
    let claim = section
        .split("Precision is ")
        .nth(1)
        .and_then(|rest| rest.split_whitespace().next())
        .expect("the README must state a total precision");
    assert_eq!(
        claim,
        format!("{hits}/{total}"),
        "the README claims {claim} but its own precision rows sum to {hits}/{total}"
    );
}

#[test]
fn the_denominators_block_matches_the_labels() {
    // `corpus/labels.toml` is committed, so this runs in CI even though the
    // archive it describes does not. The block is the count of bundles carrying
    // each term, and it silently omitted two of eight terms for a whole commit.
    let Some(labels) = labels() else { return };
    let section = measured_section();

    let mut counts: std::collections::BTreeMap<&str, usize> = std::collections::BTreeMap::new();
    // The denominator is the *scored* population, not every labelled bundle.
    // A stratum drawn to study a different term contributes labels that were
    // never read for these ones, so counting them would inflate every
    // denominator here while the README kept quoting the real one — which is
    // exactly what happened when T10's first eleven labels landed and this test
    // started demanding `49/103`.
    let scored = labels.labels.iter().filter(|label| {
        matches!(label.verdict, corpus::Verdict::Labelled)
            && labels.scores_capabilities_for(&label.stratum)
    });
    let mut population = 0usize;
    for label in scored {
        population += 1;
        for term in &label.capabilities {
            if labels.terms_labelled.iter().any(|scored| scored == term) {
                *counts.entry(term.as_str()).or_insert(0) += 1;
            }
        }
    }

    let mut wrong = Vec::new();
    for (term, count) in counts {
        let expected = format!("{count}/{population}");
        // The block writes `net.egress   49/92 bundles`, so the term and its
        // fraction appear on one line.
        let published = section
            .lines()
            .any(|line| line.contains(term) && line.contains(&expected));
        if !published {
            wrong.push(format!("{term} should read {expected}"));
        }
    }
    assert!(
        wrong.is_empty(),
        "the README's denominators disagree with corpus/labels.toml: {wrong:?}"
    );
}

#[test]
fn every_published_rate_matches_a_computed_report() {
    // The strongest check and the one CI cannot run: it needs `corpus/raw/`,
    // which is gitignored because it is 1.7 GB of untrusted third-party code.
    //
    // Skipping is stated rather than silent. A test that passes when it did not
    // run is the "zero cases, all green" failure this repository keeps naming,
    // and the eval's own report says NOT RUN in the same situation.
    let root = repo_root();
    let Some(labels) = labels() else { return };

    let rules = skillmap_rules::load(&root);
    assert!(rules.diagnostics.is_empty(), "{:?}", rules.diagnostics);

    let report = corpus::run(
        &labels,
        &root.join("corpus"),
        &rules,
        labels.labels.len(),
        std::collections::BTreeMap::new(),
    );
    if report.scored == 0 {
        eprintln!(
            "SKIPPED: corpus/raw/ is absent, so no rate could be recomputed. \
             The README's table is unverified by this run."
        );
        return;
    }

    let mut wrong = Vec::new();
    for row in rows() {
        let Some(score) = report.terms.iter().find(|score| score.term == row.term) else {
            continue;
        };
        let rate = if row.kind == "precision" {
            &score.precision
        } else {
            &score.recall
        };
        if rate.hits != row.hits || rate.total != row.total {
            wrong.push(format!(
                "{} {}: README says {}/{}, the corpus says {}/{}",
                row.term, row.kind, row.hits, row.total, rate.hits, rate.total
            ));
            continue;
        }
        // Percentages are compared at one decimal place, because the README
        // writes `100%` where the harness writes `100.0%` and neither is wrong.
        let computed = rate.point().unwrap_or_default() * 100.0;
        if (computed - row.percent).abs() > 0.05 {
            wrong.push(format!(
                "{} {}: README says {:.1}%, the corpus says {computed:.1}%",
                row.term, row.kind, row.percent
            ));
        }
    }
    assert!(
        wrong.is_empty(),
        "the README disagrees with a freshly computed report:\n  {}",
        wrong.join("\n  ")
    );
}
