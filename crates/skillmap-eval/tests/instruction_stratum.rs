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
//! **Three of the five signals have no precision, and that is deliberate.**
//! `corpus/labels.toml` records `capabilities = []` on the original strata
//! meaning *the annotator found no capability term*; every note there describes
//! code behaviour — what was read, what was called. No annotator judged the
//! prose. So an empty `capabilities` array is "not looked for" with respect to
//! `instruction.*`, not "not present", and scoring against it would book every
//! genuine detection as a false positive. That is the exact failure the header
//! of `labels.toml` warns about and `gate.rs` enforces the pairing for.
//!
//! What *is* measurable without new labels is the firing rate: how often each
//! signal fires on bundles a human read and judged benign. That is a candidate
//! false-positive rate — an upper bound, since a benign bundle may legitimately
//! contain prose that instructs a config write. Reported as such.
//!
//! **Two signals are different, and the difference is the point.**
//! `instruction.exec_directive` (T10) and `instruction.directs_outside_write`
//! (T11) have real precision and recall, because each had a stratum drawn and
//! labelled *before* its rule was written. The contrast with the other three is
//! the argument for doing that for each of them: draw, label, then write, and a
//! rate follows. Until then they have a firing rate and nothing more.
//!
//! T11 also shows the cost of getting the term wrong first. Three candidate
//! shapes were drawn for, found to be reference material rather than
//! instruction, and withdrawn before a label was written — so "draw, label,
//! then write" is two days per signal only when the term is right.

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
        // T11's signal, and the only non-zero entry here that is **not** a
        // false positive. All three firings were read: each is
        // `cp -r <skill> ~/.openclaw/workspace/skills/`, installing a skill
        // into the agent's workspace directory, and each is a genuine
        // directive.
        //
        // That is consistent rather than surprising. `code_clean` means "no
        // credential marker", not "harmless" — the README says so explicitly —
        // and these bundles were never read for instruction signals, so their
        // empty `capabilities` arrays say nothing about this term. The rate
        // against ground truth lives in
        // `the_outside_write_signal_is_scored_against_its_own_ground_truth`.
        ("instruction.directs_outside_write", 3),
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

/// Score one signal over the strata drawn for it.
///
/// Returns `(true positive, false positive, false negative, true negative)`
/// plus the digests behind the two error cells, and `None` when the archive is
/// absent so a caller can skip loudly instead of asserting on zeros.
///
/// Scoped to named strata on purpose. A signal's own strata were labelled for
/// it; every other stratum was not, so a firing there is unmeasured rather than
/// wrong, and counting it either way would invent a number.
#[allow(clippy::type_complexity, reason = "a score and its two error lists")]
fn score_signal(
    term: &str,
    strata: &[&str],
) -> Option<(usize, usize, usize, usize, Vec<String>, Vec<String>)> {
    let root = repo_root();
    let labels = match corpus::Labels::load(&root.join("corpus/labels.toml")) {
        Ok(labels) => labels,
        Err(corpus::Error::Absent(_)) => return None,
        Err(error) => panic!("corpus/labels.toml is present and unusable: {error}"),
    };
    let rules = skillmap_rules::load(&root);
    assert!(rules.diagnostics.is_empty(), "{:?}", rules.diagnostics);

    let (mut tp, mut fp, mut fn_, mut tn) = (0usize, 0usize, 0usize, 0usize);
    let (mut missed, mut spurious) = (Vec::new(), Vec::new());
    let mut scanned = 0usize;

    for label in &labels.labels {
        if label.verdict != corpus::Verdict::Labelled || !strata.contains(&label.stratum.as_str()) {
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
            .any(|entry| entry.signal.as_str() == term);
        let truth = label.capabilities.iter().any(|have| have == term);
        let tag = format!("{} [{}]", &label.digest[7..19], label.stratum);
        match (fired, truth) {
            (true, true) => tp += 1,
            (true, false) => {
                fp += 1;
                spurious.push(tag);
            }
            (false, true) => {
                fn_ += 1;
                missed.push(tag);
            }
            (false, false) => tn += 1,
        }
    }
    if scanned == 0 {
        return None;
    }
    Some((tp, fp, fn_, tn, missed, spurious))
}

fn report_score(term: &str, tp: usize, fp: usize, fn_: usize, tn: usize) {
    let precision = corpus::Rate::new(tp, tp + fp);
    let recall = corpus::Rate::new(tp, tp + fn_);
    println!("\n{term} over its own strata");
    println!("  precision {}", precision.render());
    println!("  recall    {}", recall.render());
    println!("  tp {tp}  fp {fp}  fn {fn_}  tn {tn}\n");
}

#[test]
fn the_outside_write_signal_is_scored_against_its_own_ground_truth() {
    // T11. Eighty prose-only bundles across two strata, drawn and hand-labelled
    // before this rule was written — the ordering the whole corpus discipline
    // rests on.
    const TERM: &str = "instruction.directs_outside_write";
    let Some((tp, fp, fn_, tn, missed, spurious)) =
        score_signal(TERM, &["prose_outside_write", "prose_control"])
    else {
        eprintln!("SKIPPED: corpus/raw/ is absent, so {TERM} could not be scored.");
        return;
    };
    report_score(TERM, tp, fp, fn_, tn);
    if !spurious.is_empty() {
        println!("  false positives: {spurious:?}");
    }
    if !missed.is_empty() {
        println!("  missed: {missed:?}");
    }

    assert_eq!(
        (tp, fp, fn_, tn),
        (37, 1, 0, 42),
        "the directs_outside_write score moved. Re-read the changed bundles, update \
         the README's published rate, and change these numbers deliberately"
    );
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

/// The denominators the published rates do not carry, printed on demand.
///
/// **Ignored on purpose, and it asserts nothing.** It is a measuring
/// instrument, not a gate: every number here is a distribution that moves
/// whenever a rule or a label changes, so asserting on one would produce a test
/// that fails for being out of date rather than because anything is wrong.
///
/// It exists because a critical reading of this project kept needing figures
/// that were nowhere in the repository, and re-deriving them by hand each time
/// invites getting them wrong. What it answers:
///
/// - **How often is the analysis incomplete?** 84% of scanned bundles carry at
///   least one `unresolved` entry, ~4.5 computed targets each. That figure
///   belongs beside "zero false positives" every time it is quoted; the first
///   without the second overstates what was established.
/// - **How strong is the reachability claim?** Roughly 40% of reported
///   capabilities are `present` rather than `observed` — the code is there and
///   nothing proved it runs.
/// - **What is named but not analysed?** The `parsed_as` tally shows JSON, Rust
///   and YAML files sitting in the inventory with no grammar behind them.
///
/// Run it with `cargo test -p skillmap-eval --test instruction_stratum --
/// --ignored --nocapture`.
#[test]
#[ignore = "diagnostic, not a gate: prints distributions for review"]
fn reachability_and_coverage_distribution() {
    let root = repo_root();
    let Ok(labels) = corpus::Labels::load(&root.join("corpus/labels.toml")) else {
        return;
    };
    let rules = skillmap_rules::load(&root);
    let mut reach: BTreeMap<String, usize> = BTreeMap::new();
    let mut unres: BTreeMap<String, usize> = BTreeMap::new();
    let mut parsed: BTreeMap<String, usize> = BTreeMap::new();
    let (mut bundles, mut with_unres) = (0usize, 0usize);
    for label in &labels.labels {
        if label.verdict != corpus::Verdict::Labelled {
            continue;
        }
        let dir = corpus::bundle_dir(&root.join("corpus"), &label.digest);
        let Ok(m) = skillmap_scan::analyze(&dir, &rules) else {
            continue;
        };
        bundles += 1;
        if !m.unresolved.is_empty() {
            with_unres += 1;
        }
        for c in &m.capabilities {
            *reach.entry(format!("{:?}", c.reachability)).or_insert(0) += 1;
        }
        for u in &m.unresolved {
            *unres.entry(format!("{:?}", u.reason)).or_insert(0) += 1;
        }
        for f in &m.inventory {
            *parsed.entry(f.parsed_as.clone()).or_insert(0) += 1;
        }
    }
    println!("\n  bundles scanned: {bundles}, with >=1 unresolved: {with_unres}");
    println!("  reachability of reported capabilities: {reach:?}");
    println!("  unresolved reasons: {unres:?}");
    println!("  files by parsed_as: {parsed:?}");
}

#[test]
fn the_remote_instruction_signal_is_scored_against_its_own_ground_truth() {
    // T13. This one carries a caveat the other two do not, and it belongs next
    // to the number rather than in a document nobody opens.
    //
    // The rule was REWRITTEN AFTER the phase 1 strata were labelled, which
    // inverts the ordering the corpus discipline rests on: labels before rules.
    // The four phase 1 strata are therefore in-sample for it, and a precision
    // measured only there would be fitted rather than observed.
    //
    // `instr_remote_instructions_holdout` exists to answer that. Thirty bundles
    // nobody had read, drawn by `scripts/draw_instruction_validation.py` from
    // the 100 unlabelled bundles matching the narrowed shape, and judged against
    // the term definition fixed before phase 1 rather than against the patterns.
    // Both are asserted below, separately, so the honest number stays visible
    // beside the flattering one.
    const TERM: &str = "instruction.fetch_as_instruction";

    let Some((tp, fp, fn_, tn, missed, spurious)) =
        score_signal(TERM, &["instr_remote_instructions_holdout"])
    else {
        eprintln!("SKIPPED: corpus/raw/ is absent, so {TERM} could not be scored.");
        return;
    };
    report_score(
        &format!("{TERM} [held out, unread when drawn]"),
        tp,
        fp,
        fn_,
        tn,
    );
    if !spurious.is_empty() {
        println!("  false positives: {spurious:?}");
    }
    if !missed.is_empty() {
        println!("  missed: {missed:?}");
    }
    assert_eq!(
        (tp, fp, fn_, tn),
        (29, 0, 0, 1),
        "the held-out remote-instruction score moved. This is the only figure for \
         this term that was not fitted, so change it deliberately and re-read the \
         bundles before touching the published rate"
    );

    let Some((tp, fp, fn_, tn, _, spurious)) = score_signal(
        TERM,
        &[
            "instr_config_mutation",
            "instr_exfil",
            "instr_fetch_instruction",
            "instr_control",
        ],
    ) else {
        return;
    };
    report_score(
        &format!("{TERM} [phase 1 strata, IN SAMPLE]"),
        tp,
        fp,
        fn_,
        tn,
    );
    if !spurious.is_empty() {
        println!("  false positives: {spurious:?}");
    }
    assert_eq!(
        (tp, fp, fn_, tn),
        (18, 0, 8, 119),
        "the in-sample remote-instruction score moved. Recall here is the honest \
         half of this pair: eight of twenty-six real cases are still missed"
    );
}

#[test]
fn the_config_mutation_signal_is_scored_against_its_own_ground_truth() {
    // T13 measured this rule and held it back rather than repairing it, on the
    // grounds that ~64% precision is far below the 97-100% the other instruction
    // signals hold. That comparison was against the wrong thing: the rule it
    // would have replaced was at 60%, so refusing the repair left users with the
    // worse of the two. The repair ships, and this pins what it actually scores.
    //
    // Not gated on the held-out stratum, because that stratum was drawn and
    // labelled for the remote-instruction term only. Scoring this one there
    // would read silence as ground truth, which is the failure that took
    // published precision from 113/113 to 113/119 during T11.
    const TERM: &str = "instruction.config_mutation";
    let Some((tp, fp, fn_, tn, missed, spurious)) = score_signal(
        TERM,
        &[
            "instr_config_mutation",
            "instr_exfil",
            "instr_fetch_instruction",
            "instr_control",
        ],
    ) else {
        eprintln!("SKIPPED: corpus/raw/ is absent, so {TERM} could not be scored.");
        return;
    };
    report_score(TERM, tp, fp, fn_, tn);
    if !spurious.is_empty() {
        println!("  false positives: {spurious:?}");
    }
    if !missed.is_empty() {
        println!("  missed: {missed:?}");
    }
    assert_eq!(
        (tp, fp, fn_, tn),
        (38, 15, 11, 81),
        "the config_mutation score moved. It is the weakest shipped signal and          the one most worth watching: what remains in `fp` is security scanners          enumerating the shapes they detect, which no pattern reaches"
    );
}
