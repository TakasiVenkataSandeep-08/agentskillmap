//! `skillmap-corpus` — the research harvester.
//!
//! This is a standalone binary for now. `docs/01-corpus-scan.md` calls for
//! `skillmap corpus` as a subcommand, and it becomes one when the CLI lands at
//! T9; wiring a subcommand into a binary that does not exist yet is not
//! something to fake in the meantime.
//!
//! **This binary makes network requests.** It is one of the two sanctioned
//! exceptions to invariant 9, and it says so on every run.

// The CLI layer is exactly where invariant 10 permits these: there is no caller
// to hand a typed error to, and a non-zero exit with a legible message is the
// correct behaviour for a command-line tool.
#![allow(
    clippy::print_stderr,
    clippy::print_stdout,
    reason = "this is the command-line entry point; stderr and stdout are its interface"
)]

use skillmap_corpus::{
    archive::Archive,
    github::{CodeSearch, GitFetcher, GitHub, Named, CODE_SEARCH_QUERY},
    report, Error, HarvestOptions, Provenance, RepoRef, Source, SourceReport,
};
use std::path::PathBuf;

/// The baseline: what "good" looks like.
const BASELINE: &[&str] = &["anthropics/skills"];

/// Curated lists named in `docs/01-corpus-scan.md`.
///
/// These sample the **head** of the ecosystem by construction — somebody already
/// decided each one was worth listing. The report never pools them with code
/// search results.
const CURATED: &[&str] = &["ComposioHQ/awesome-claude-skills"];

fn main() -> std::process::ExitCode {
    match run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("\nerror: {error}");
            std::process::ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), Error> {
    let options = parse_args();

    eprintln!(
        "skillmap-corpus: this command makes network requests to api.github.com \
         and runs `git clone`.\n\
         It is a research tool. The scanner itself never touches the network \
         (invariant 9).\n"
    );

    // Fail fast, before any work, with an explanation rather than a 403 later.
    let token = skillmap_corpus::github_token()?;
    let client = GitHub::new(token);
    let archive = Archive::open(&options.corpus_dir)?;

    let mut repos: Vec<RepoRef> = Vec::new();
    // What each source yielded, recorded so the report can print it. A source
    // returning nothing is a fact about the harvest, and the first real run
    // sampled zero from code search without saying so anywhere.
    let mut sources: Vec<SourceReport> = Vec::new();

    for (slugs, provenance) in [
        (BASELINE, Provenance::Baseline),
        (CURATED, Provenance::CuratedList),
    ] {
        eprintln!(
            "resolving {} source(s) [{}]",
            slugs.len(),
            provenance.as_str()
        );
        let found = Named {
            client: &client,
            slugs: slugs.iter().map(|slug| (*slug).to_owned()).collect(),
            provenance,
        }
        .repos(options.limit)?;
        sources.push(SourceReport {
            provenance,
            query: slugs.join(", "),
            repositories: found.len() as u64,
        });
        repos.extend(found);
    }

    eprintln!("searching code for `{CODE_SEARCH_QUERY}` (this is the only tail sample)");
    let found = CodeSearch { client: &client }.repos(options.limit)?;
    if found.is_empty() {
        eprintln!(
            "
WARNING: code search returned zero repositories.
             That is the only source that reaches the tail of the ecosystem, so this
             corpus is entirely curated head and its base rates cannot be read as
             ecosystem rates. The report says so too. Check the token is valid and
             that `{CODE_SEARCH_QUERY}` still matches on the REST search index.
"
        );
    }
    sources.push(SourceReport {
        provenance: Provenance::CodeSearch,
        query: CODE_SEARCH_QUERY.to_owned(),
        repositories: found.len() as u64,
    });
    repos.extend(found);

    // Deduplicate by slug, keeping the first (curated) provenance for anything
    // that turns up in both. Recording a curated repository as tail would
    // overstate how much of the tail was actually reached.
    let mut seen = std::collections::BTreeSet::new();
    repos.retain(|repo| seen.insert(repo.slug()));

    eprintln!("harvesting {} repositories\n", repos.len());
    let index =
        report::harvest_with_sources(&repos, &GitFetcher, &archive, &options.snapshot, sources)?;

    let json = report::index_json(&index)?;
    let markdown = report::report(&index);
    archive.write_outputs(&json, &markdown)?;

    println!(
        "{} bundles indexed, {} repositories skipped",
        index.records.len(),
        index.skipped.len()
    );
    println!("wrote {}/index.json", options.corpus_dir.display());
    println!("wrote {}/report.md", options.corpus_dir.display());

    if index.records.is_empty() {
        println!(
            "\nThe corpus is empty. Read report.md before concluding anything: an \
             empty result is a finding about the harvest, not about the ecosystem."
        );
    }
    Ok(())
}

/// Minimal argument parsing.
///
/// No `clap`: three options do not justify a dependency in the one crate whose
/// dependency tree is already the hardest to defend.
fn parse_args() -> HarvestOptions {
    let mut options = HarvestOptions::default();
    let mut args = std::env::args().skip(1);

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--corpus-dir" => {
                if let Some(value) = args.next() {
                    options.corpus_dir = PathBuf::from(value);
                }
            }
            "--limit" => {
                if let Some(value) = args.next().and_then(|raw| raw.parse().ok()) {
                    options.limit = value;
                }
            }
            "--snapshot" => {
                if let Some(value) = args.next() {
                    options.snapshot = value;
                }
            }
            "--help" | "-h" => {
                println!(
                    "skillmap-corpus — harvest and measure real SKILL.md bundles\n\n\
                     USAGE:\n    skillmap-corpus [--corpus-dir DIR] [--limit N] [--snapshot LABEL]\n\n\
                     OPTIONS:\n    \
                     --corpus-dir DIR   where to write raw/, index.json, report.md (default: corpus)\n    \
                     --limit N          maximum repositories per source (default: 200)\n    \
                     --snapshot LABEL   label for this snapshot, e.g. 2026-08 (default: unlabelled)\n\n\
                     ENVIRONMENT:\n    \
                     GITHUB_TOKEN       required; no scopes needed for public data\n\n\
                     Makes network requests. The scanner itself never does."
                );
                std::process::exit(0);
            }
            _ => {}
        }
    }
    options
}
