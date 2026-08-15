//! T5's remaining "done when": *"false-positive rate on the benign stratum is
//! measured and published, per signal."*
//!
//! Nothing had ever counted an instruction finding over the corpus. The reason
//! is structural rather than an oversight: `Manifest::instructions` is a
//! different field from `Manifest::capabilities`, and `corpus::run` iterates the
//! latter. Every scored term, every precision figure, and the `unmeasured` tally
//! that exists precisely to catch claims without ground truth — all of it walks
//! past the instruction plane without looking at it.
//!
//! **This file does not compute precision, and the omission is deliberate.**
//! `corpus/labels.toml` records `capabilities = []` on the benign stratum
//! meaning *the annotator found no capability term*; every note in that stratum
//! describes code behaviour — what was read, what was called. No annotator ever
//! judged the prose. So an empty `capabilities` array is "not looked for" with
//! respect to `instruction.*`, not "not present", and scoring against it would
//! book every genuine detection as a false positive. That is the exact failure
//! the header of `labels.toml` warns about and `gate.rs` enforces the pairing
//! for.
//!
//! What *is* measurable without new labels is the firing rate: how often each
//! signal fires on bundles a human read and judged benign. That is a candidate
//! false-positive rate — an upper bound, since a benign bundle may legitimately
//! contain prose that instructs a config write. Reported as such.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "a failed assertion in a test is the test failing, which is the point"
)]

use skillmap_eval::corpus;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

/// Every signal that ships a rule, read from the rules tree rather than listed
/// here. A signal that ships and is absent from this list would otherwise go
/// unmeasured in the file whose whole job is to measure them.
fn shipped_signals() -> Vec<String> {
    let mut found = Vec::new();
    let dir = repo_root().join("rules").join("markdown");
    for entry in std::fs::read_dir(&dir).expect("the markdown rules directory must exist") {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|ext| ext == "toml") {
            let text = std::fs::read_to_string(&path).unwrap();
            for line in text.lines() {
                // `capability = "instruction.exfil"`, whitespace-aligned in the
                // rule files, so split on `=` rather than matching a fixed
                // prefix.
                if let Some((key, value)) = line.split_once('=') {
                    if key.trim() == "capability" {
                        let signal = value.trim().trim_matches('"');
                        if signal.starts_with("instruction.") {
                            found.push(signal.to_owned());
                        }
                    }
                }
            }
        }
    }
    found.sort();
    found.dedup();
    found
}

#[test]
fn instruction_signals_are_counted_on_every_stratum() {
    let root = repo_root();
    let labels = match corpus::Labels::load(&root.join("corpus/labels.toml")) {
        Ok(labels) => labels,
        Err(corpus::Error::Absent(_)) => return,
        Err(error) => panic!("corpus/labels.toml is present and unusable: {error}"),
    };

    let rules = skillmap_rules::load(&root);
    assert!(rules.diagnostics.is_empty(), "{:?}", rules.diagnostics);

    let signals = shipped_signals();
    assert!(
        !signals.is_empty(),
        "no instruction signal ships, so this measurement is vacuous"
    );

    // stratum -> signal -> bundles firing (once per bundle, not per evidence
    // site: the denominator is bundles).
    let mut fired: BTreeMap<String, BTreeMap<String, usize>> = BTreeMap::new();
    let mut population: BTreeMap<String, usize> = BTreeMap::new();
    let mut unscannable = 0usize;
    // Evidence for anything that fires, so a hit can be adjudicated by reading
    // rather than by rerunning the scanner.
    let mut sites: Vec<String> = Vec::new();

    for label in &labels.labels {
        if label.verdict != corpus::Verdict::Labelled {
            continue;
        }
        let dir = corpus::bundle_dir(&root.join("corpus"), &label.digest);
        let Ok(manifest) = skillmap_scan::analyze(&dir, &rules) else {
            unscannable += 1;
            continue;
        };
        *population.entry(label.stratum.clone()).or_insert(0) += 1;

        let mut seen: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
        for instruction in &manifest.instructions {
            let signal = instruction.signal.as_str().to_owned();
            if seen.insert(signal.clone()) {
                *fired
                    .entry(label.stratum.clone())
                    .or_default()
                    .entry(signal.clone())
                    .or_insert(0) += 1;
                if let Some(first) = instruction.evidence.first() {
                    sites.push(format!(
                        "    {}  {:<34} {}:{}",
                        &label.digest[7..19],
                        signal,
                        first.file,
                        first.start_line
                    ));
                }
            }
        }
    }

    if population.is_empty() {
        eprintln!(
            "SKIPPED: corpus/raw/ is absent, so no instruction signal could be \
             counted. T5's benign-stratum rate is unmeasured by this run."
        );
        return;
    }

    println!("\ninstruction signals over the labelled corpus");
    println!("  bundles that could not be scanned: {unscannable}");
    for (stratum, total) in &population {
        println!("\n  {stratum} ({total} bundles)");
        for signal in &signals {
            let hits = fired
                .get(stratum)
                .and_then(|by_signal| by_signal.get(signal))
                .copied()
                .unwrap_or(0);
            let rate = corpus::Rate::new(hits, *total);
            println!("    {signal:<38} {}", rate.render());
        }
    }
    if sites.is_empty() {
        println!("\n  no instruction signal fired on any labelled bundle.");
    } else {
        println!("\n  evidence, for adjudication:");
        for site in &sites {
            println!("{site}");
        }
    }
    println!();

    // ── The adjudication ────────────────────────────────────────────────────
    //
    // Every firing above was read and judged. Both are false positives, and
    // both are the *documented* failure mode of their rule rather than a
    // surprise — which is why the count is asserted here instead of being left
    // as printed output nobody diffs.
    //
    // `973fc8647f97` — `instruction.config_mutation` on a test-results
    // document whose Recommendations list reads "Add to AGENTS.md workflow -
    // Integrate into daily routine". The actor is the human maintainer and the
    // genre is a roadmap, not the skill directing the agent to rewrite its own
    // configuration.
    //
    // `c85ba6933c82` — `instruction.exfil` on a network-DLP skill's "The
    // Problem" section, warning that a compromised skill can POST workspace
    // contents to an external server. Prose *against* the behaviour. The rule's
    // own `false_positive_notes` predicted exactly this, and the negative
    // fixture guarding it is this repository's threat-model text — the same
    // shape, written by us.
    //
    // A rise here means a rule widened and nobody adjudicated the new hits.
    let benign = fired.get("code_clean").cloned().unwrap_or_default();
    let known: BTreeMap<&str, usize> = [
        ("instruction.config_mutation", 1),
        ("instruction.exfil", 1),
        ("instruction.fetch_as_instruction", 0),
        // T10's signal. Zero on the benign stratum, which is the number a rule
        // matching a shape present in a third of all shell-fence bundles puts
        // at risk. Its own strata are scored separately below, where a rate
        // against ground truth is available.
        ("instruction.exec_directive", 0),
    ]
    .into_iter()
    .collect();

    for signal in &signals {
        let observed = benign.get(signal).copied().unwrap_or(0);
        let adjudicated = known.get(signal.as_str()).copied().unwrap_or_else(|| {
            panic!(
                "{signal} ships a rule and has no adjudicated benign-stratum count. \
                 Read its firings above, judge each, and record the number here — \
                 an unadjudicated signal is unpublished, which is what T5 forbids."
            )
        });
        assert_eq!(
            observed, adjudicated,
            "{signal} fires on {observed} benign bundles; {adjudicated} were read and \
             judged. Adjudicate the difference and update this table — the published \
             false-positive rate is derived from these numbers"
        );
    }
}

#[test]
fn the_exec_directive_signal_is_scored_against_its_own_ground_truth() {
    // T10 phase 2. Unlike the three signals above, this one has ground truth:
    // `fence_directive` and `fence_control` were drawn and labelled for it
    // *before* this rule existed, which is the ordering the whole corpus
    // discipline is built around.
    //
    // Scored only over those two strata. The other four were never read for
    // this term, so a firing there is neither a true nor a false positive — it
    // is unmeasured, and counting it either way would invent a number.
    let root = repo_root();
    let labels = match corpus::Labels::load(&root.join("corpus/labels.toml")) {
        Ok(labels) => labels,
        Err(corpus::Error::Absent(_)) => return,
        Err(error) => panic!("corpus/labels.toml is present and unusable: {error}"),
    };
    let rules = skillmap_rules::load(&root);
    assert!(rules.diagnostics.is_empty(), "{:?}", rules.diagnostics);

    const TERM: &str = "instruction.exec_directive";
    let (mut tp, mut fp, mut fn_, mut tn) = (0usize, 0usize, 0usize, 0usize);
    let mut missed = Vec::new();
    let mut spurious = Vec::new();
    let mut scanned = 0usize;

    for label in &labels.labels {
        if label.verdict != corpus::Verdict::Labelled {
            continue;
        }
        if !matches!(label.stratum.as_str(), "fence_directive" | "fence_control") {
            continue;
        }
        let dir = corpus::bundle_dir(&root.join("corpus"), &label.digest);
        let Ok(manifest) = skillmap_scan::analyze(&dir, &rules) else {
            continue;
        };
        scanned += 1;
        let fired = manifest
            .instructions
            .iter()
            .any(|entry| entry.signal.as_str() == TERM);
        let truth = label.capabilities.iter().any(|term| term == TERM);
        match (fired, truth) {
            (true, true) => tp += 1,
            (true, false) => {
                fp += 1;
                spurious.push(format!("{} [{}]", &label.digest[7..19], label.stratum));
            }
            (false, true) => {
                fn_ += 1;
                missed.push(format!("{} [{}]", &label.digest[7..19], label.stratum));
            }
            (false, false) => tn += 1,
        }
    }

    if scanned == 0 {
        eprintln!("SKIPPED: corpus/raw/ is absent, so {TERM} could not be scored.");
        return;
    }

    let precision = corpus::Rate::new(tp, tp + fp);
    let recall = corpus::Rate::new(tp, tp + fn_);
    println!("\n{TERM} over its own strata ({scanned} bundles)");
    println!("  precision {}", precision.render());
    println!("  recall    {}", recall.render());
    println!("  tp {tp}  fp {fp}  fn {fn_}  tn {tn}");
    if !spurious.is_empty() {
        println!("  false positives: {spurious:?}");
    }
    if !missed.is_empty() {
        println!("  missed: {missed:?}");
    }
    println!();

    // Recorded rather than asserted at a threshold: a number that only has to
    // beat a bar invites tuning the bar. What is asserted is that the measured
    // result has not moved without somebody updating this line and the README
    // beside it.
    assert_eq!(
        (tp, fp, fn_, tn),
        (31, 0, 4, 45),
        "the exec_directive score moved. Re-read the changed bundles, update the \
         README's published rate, and change these numbers deliberately"
    );
}
