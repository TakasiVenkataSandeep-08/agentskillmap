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

use skillmap_eval::{baseline_path, metrics, Baseline};
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

    // The corpus suite and the headline metric both need T3. Saying so on every
    // run is the point: a number nobody measured must never look measured.
    println!(
        "\ncorpus suite:      NOT RUN — no labeled corpus exists (T3 has not been\n\
         \x20                  harvested). There is therefore no held-out split, no\n\
         \x20                  precision/recall against ground truth, and no\n\
         \x20                  false-positive rate on a benign stratum, which\n\
         \x20                  docs/05-eval.md names as the headline metric."
    );

    let path = baseline_path(&root);
    if bless {
        let baseline = Baseline {
            note: "Fixture and adversarial suites only. These are NOT the published \
                   numbers docs/05-eval.md requires: there is no labeled corpus yet, so \
                   no precision, recall, or benign-stratum false-positive rate has been \
                   measured. Re-bless with `cargo run -p skillmap-eval -- --bless`."
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
