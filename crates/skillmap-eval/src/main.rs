//! `skillmap-eval` — run the suites, print the report, gate the build.
//!
//! Becomes `skillmap eval` at T9. Exit code is 0 iff every case that could run
//! passed and nothing regressed against `eval/baseline.json`.
//!
//! Re-bless the baseline with `--bless` after an intentional change, and read the
//! diff: a shrinking baseline is a suite that checks less than it did.

#![allow(
    clippy::print_stderr,
    clippy::print_stdout,
    reason = "this is the command-line entry point; stdout is its interface"
)]

use skillmap_eval::{baseline_path, corpus, metrics, Baseline};
use std::path::{Path, PathBuf};

fn main() -> std::process::ExitCode {
    let root = repo_root();
    let bless = std::env::args().any(|arg| arg == "--bless");

    let report = match skillmap_eval::run(&root) {
        Ok(report) => report,
        Err(error) => {
            eprintln!("error: {error}");
            return std::process::ExitCode::FAILURE;
        }
    };

    println!("skillmap eval\n=============\n");

    println!(
        "fixture suite:     {} passed, {} failed",
        report.metrics.fixtures_passed, report.metrics.fixtures_failed
    );
    println!(
        "adversarial suite: {} passed, {} failed",
        report.metrics.adversarial_passed, report.metrics.adversarial_failed
    );

    let pending = report.pending();
    if !pending.is_empty() {
        println!("\npending ({}), declared but not runnable:", pending.len());
        for outcome in pending {
            println!(
                "  {:<28} {}",
                outcome.id,
                outcome.pending.as_deref().unwrap_or("?")
            );
        }
    }

    let failures = report.failures();
    if !failures.is_empty() {
        println!("\nfailures:");
        for outcome in failures {
            println!("  {} — {}", outcome.id, outcome.description);
            for failure in &outcome.failures {
                println!("      {failure}");
            }
        }
    }

    // The corpus suite. It runs where the labels and the archive both exist,
    // and says which one is missing where they do not — a fresh clone has
    // neither, and "no labels here" must not read as "nothing to measure".
    print!("\n{}", corpus_section(&root));

    let path = baseline_path(&root);
    if bless {
        let baseline = Baseline {
            note: "Fixture and adversarial suites only. These are NOT the published \
                   numbers docs/05-eval.md requires: precision, recall and the \
                   per-stratum false-positive rate are computed over corpus/labels.toml \
                   and published in the README, which \
                   crates/skillmap-eval/tests/published.rs checks against a fresh \
                   recompute. They are deliberately not recorded here, because these \
                   metrics are the fixture and adversarial counts alone. Re-bless with \
                   `cargo run -p skillmap-eval -- --bless`."
                .to_owned(),
            corpus_snapshot: None,
            metrics: report.metrics,
        };
        let rendered = match metrics::to_json(&baseline) {
            Ok(rendered) => rendered,
            Err(error) => {
                eprintln!("\nerror: cannot render baseline: {error}");
                return std::process::ExitCode::FAILURE;
            }
        };
        if let Err(error) = write(&path, &rendered) {
            eprintln!("\nerror: cannot write {}: {error}", path.display());
            return std::process::ExitCode::FAILURE;
        }
        println!("\nblessed {}", path.display());
        return std::process::ExitCode::SUCCESS;
    }

    let baseline: Baseline = match std::fs::read_to_string(&path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
    {
        Some(baseline) => baseline,
        None => {
            eprintln!(
                "\nerror: cannot read {}. Bless it with \
                 `cargo run -p skillmap-eval -- --bless`.",
                path.display()
            );
            return std::process::ExitCode::FAILURE;
        }
    };

    let regressions = metrics::regressions(&baseline, &report.metrics);
    if regressions.is_empty() && report.passed() {
        println!("\nno regression against {}", path.display());
        return std::process::ExitCode::SUCCESS;
    }

    println!("\nREGRESSION:");
    for regression in &regressions {
        println!("  {regression}");
    }
    std::process::ExitCode::FAILURE
}

fn write(path: &Path, body: &str) -> Result<(), std::io::Error> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, body)
}

/// The repository root, relative to this crate.
fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

/// The corpus-suite section of the report.
///
/// Three outcomes, and keeping them distinct is the whole job. Labels present
/// and archive present: real numbers. Labels present, archive absent: the labels
/// name bundles this machine does not have, which is the normal state of a fresh
/// clone and is not a measurement of anything. No labels: the suite has never
/// been run here.
///
/// A single "corpus suite: 0 findings" line would collapse all three into the
/// most flattering one.
fn corpus_section(root: &Path) -> String {
    let labels_path = root.join("corpus").join("labels.toml");
    let labels = match corpus::Labels::load(&labels_path) {
        Ok(labels) => labels,
        Err(corpus::Error::Absent(_)) => {
            return "corpus suite:      NOT RUN — corpus/labels.toml does not exist.\n\
                    \x20                  There is therefore no ground truth, no held-out\n\
                    \x20                  split, and no precision, recall or benign-stratum\n\
                    \x20                  false-positive rate — which docs/05-eval.md names\n\
                    \x20                  as the headline metric.\n"
                .to_owned();
        }
        Err(error) => return format!("corpus suite:      ERROR — {error}\n"),
    };

    // How much of the draw has been labelled at all. Read from the sample rather
    // than assumed, so "we labelled eleven" and "the sample is a hundred and
    // thirty" cannot drift apart.
    let sample = std::fs::read_to_string(root.join("corpus").join("sample.json"))
        .ok()
        .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok());
    let sample_size = sample
        .as_ref()
        .and_then(|value| {
            value
                .get("selected")
                .and_then(serde_json::Value::as_array)
                .map(Vec::len)
        })
        .unwrap_or(0);
    // Stratum weights, so the report can say the rows are not poolable.
    let population: std::collections::BTreeMap<String, usize> = sample
        .as_ref()
        .and_then(|value| value.get("population"))
        .and_then(serde_json::Value::as_object)
        .map(|map| {
            map.iter()
                .filter_map(|(name, count)| {
                    count
                        .as_u64()
                        .and_then(|n| usize::try_from(n).ok())
                        .map(|n| (name.clone(), n))
                })
                .collect()
        })
        .unwrap_or_default();

    let rules = skillmap_rules::load(root);
    corpus::render(&corpus::run(
        &labels,
        &root.join("corpus"),
        &rules,
        sample_size,
        population,
    ))
}
