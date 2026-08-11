//! `skillmap` — the binary.
//!
//! Two subcommands so far, and they are a pair: `lock` records what the skills in
//! a project can do today, `ci` fails when that changes. Neither is useful alone.
//!
//! ```text
//! $ skillmap ci
//! ✗ example-skill  capability escalation vs skillmap.lock
//!     + fs.read.credential   scripts/collect.py:17   py.credential-read.dotfile
//!       reads ~/.aws/credentials — added in this update
//! ```
//!
//! `scan` (emit a manifest), the npm wrapper, and reproducible signed releases are
//! T9. This binary exists at T8 because T8's acceptance test is *"a fixture skill
//! that gains `fs.read.credential` in v1.1 causes a failing check"* — and a check
//! nobody can run is not a check.

use skillmap_core::Manifest;
use skillmap_policy::{Outcome, Policy};
use skillmap_resolve::{ClaudeCode, Scope};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

/// Where the rules tree is expected when `--rules` is not given.
///
/// Rules are data files, not compiled in, which is invariant 7 working as
/// intended everywhere except distribution: a shipped binary has no `rules/`
/// beside it. T9 packages them. Until then this defaults to the current
/// directory and says so loudly when nothing loads, rather than scanning with an
/// empty ruleset and reporting a clean result — which is the one failure mode
/// this project cannot have.
const DEFAULT_RULES: &str = ".";

/// Default lockfile name, in the project root.
const DEFAULT_LOCK: &str = "skillmap.lock";

/// Default policy file name, in the project root.
const DEFAULT_POLICY: &str = "policy.toml";

fn main() -> ExitCode {
    match run() {
        Ok(code) => ExitCode::from(code),
        Err(message) => {
            eprintln!("skillmap: {message}");
            ExitCode::from(Outcome::CONFIG_ERROR)
        }
    }
}

/// Parsed command line.
struct Args {
    project: PathBuf,
    rules: PathBuf,
    lock: PathBuf,
    policy: PathBuf,
}

const USAGE: &str = "\
skillmap — a supply-chain auditor for AI agent skills

USAGE:
    skillmap lock [OPTIONS]    write skillmap.lock from the skills in a project
    skillmap ci   [OPTIONS]    fail if capabilities changed, or policy forbids them

OPTIONS:
    --project <DIR>    project root to scan          [default: .]
    --rules <DIR>      directory containing rules/   [default: .]
    --lock <FILE>      lockfile path                 [default: <project>/skillmap.lock]
    --policy <FILE>    policy path                   [default: <project>/policy.toml]

EXIT CODES (ci):
    0  clean
    1  a bundle gained capability it did not have in the lock
    2  a capability is present that policy.toml does not permit
    3  both
    4  the check could not run — bad arguments, no rules, missing lock

Exit 4 is separate on purpose: \"could not run\" must never read as \"ran and
found nothing\".";

fn run() -> Result<u8, String> {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let Some(command) = argv.first() else {
        println!("{USAGE}");
        return Err("no subcommand given".to_owned());
    };

    if command == "--help" || command == "-h" || command == "help" {
        println!("{USAGE}");
        return Ok(0);
    }

    let args = parse_args(argv.get(1..).unwrap_or_default())?;

    match command.as_str() {
        "lock" => write_lock(&args).map(|()| 0),
        "ci" => check(&args),
        other => Err(format!("unknown subcommand `{other}`\n\n{USAGE}")),
    }
}

/// Parse flags. Every flag takes a value; an unknown flag is an error rather than
/// a warning, because a typo'd `--polcy` that silently used the default would
/// mean CI passed against a policy nobody wrote.
fn parse_args(flags: &[String]) -> Result<Args, String> {
    let mut project: Option<PathBuf> = None;
    let mut rules: Option<PathBuf> = None;
    let mut lock: Option<PathBuf> = None;
    let mut policy: Option<PathBuf> = None;

    let mut rest = flags.iter();
    while let Some(flag) = rest.next() {
        let slot = match flag.as_str() {
            "--project" => &mut project,
            "--rules" => &mut rules,
            "--lock" => &mut lock,
            "--policy" => &mut policy,
            other => return Err(format!("unknown option `{other}`\n\n{USAGE}")),
        };
        let Some(value) = rest.next() else {
            return Err(format!("`{flag}` needs a value"));
        };
        *slot = Some(PathBuf::from(value));
    }

    let project = project.unwrap_or_else(|| PathBuf::from("."));
    Ok(Args {
        rules: rules.unwrap_or_else(|| PathBuf::from(DEFAULT_RULES)),
        lock: lock.unwrap_or_else(|| project.join(DEFAULT_LOCK)),
        policy: policy.unwrap_or_else(|| project.join(DEFAULT_POLICY)),
        project,
    })
}

/// Discover and scan every bundle in the project.
///
/// Rule-loading diagnostics and undiscoverable directories both go to stderr and
/// neither is swallowed: a run that could not look at a bundle must not read the
/// same as a run that looked and found nothing (invariant 3).
fn scan(args: &Args) -> Result<Vec<Manifest>, String> {
    let rules = skillmap_rules::load(&args.rules);
    for diagnostic in &rules.diagnostics {
        eprintln!(
            "skillmap: rule diagnostic [{}] {}{}",
            diagnostic.code.as_str(),
            diagnostic.file.as_deref().unwrap_or(""),
            diagnostic
                .note
                .as_deref()
                .map(|note| format!(": {note}"))
                .unwrap_or_default()
        );
    }
    if rules.rules.is_empty() {
        return Err(format!(
            "no rules loaded from {}/rules — every bundle would scan clean, which \
             would be a lie. Pass --rules <dir> pointing at a checkout of this \
             repository.",
            args.rules.display()
        ));
    }

    let discovery = skillmap_resolve::discover(&ClaudeCode, &args.project, Scope::Project)
        .map_err(|error| format!("cannot discover skills: {error}"))?;

    for skipped in &discovery.skipped {
        eprintln!(
            "skillmap: skipped {} — {}",
            skipped.path.display(),
            skipped.reason
        );
    }

    let mut manifests = Vec::new();
    for bundle in &discovery.bundles {
        let path = bundle.path();
        let manifest = skillmap_scan::analyze(&path, &rules)
            .map_err(|error| format!("cannot analyze {}: {error}", path.display()))?;
        manifests.push(manifest);
    }

    if manifests.is_empty() && discovery.skipped.is_empty() {
        eprintln!(
            "skillmap: no skills found under {}/.claude/skills",
            args.project.display()
        );
    }
    Ok(manifests)
}

/// `skillmap lock` — record the current capability set.
fn write_lock(args: &Args) -> Result<(), String> {
    let manifests = scan(args)?;
    let lock = skillmap_diff::Lock::from_manifests(&manifests);
    let json = lock
        .to_json()
        .map_err(|error| format!("cannot serialize the lock: {error}"))?;
    std::fs::write(&args.lock, json)
        .map_err(|error| format!("cannot write {}: {error}", args.lock.display()))?;

    println!(
        "wrote {} — {} bundle(s)",
        args.lock.display(),
        lock.bundles.len()
    );
    Ok(())
}

/// `skillmap ci` — the check.
fn check(args: &Args) -> Result<u8, String> {
    let manifests = scan(args)?;

    let lock = read_lock(&args.lock)?;
    let policy = Policy::load(&args.policy).map_err(|error| error.to_string())?;

    let delta = skillmap_diff::diff(&lock, &manifests);
    // An absent policy.toml is an absent opinion, so the policy half simply does
    // not run — and says so, because a check that quietly stopped checking is the
    // failure mode this project exists to describe.
    let violations = match &policy {
        Some(policy) => skillmap_policy::violations(policy, &manifests),
        None => {
            eprintln!(
                "skillmap: no {} — checking escalation against the lock only, \
                 nothing against an allowlist",
                args.policy.display()
            );
            Vec::new()
        }
    };
    let outcome = skillmap_policy::decide(&delta, &violations);

    if !delta.is_empty() {
        print!("{}", skillmap_diff::render(&delta, &manifests));
    }
    print!("{}", skillmap_policy::render(&violations));

    match outcome {
        Outcome::Clean => println!(
            "✓ {} bundle(s), no capability change{}",
            manifests.len(),
            if policy.is_some() {
                " and nothing outside policy.toml"
            } else {
                ""
            }
        ),
        // Both failure modes end with the one action a reviewer can take, because
        // "done when" is measured in seconds and hunting for the command is where
        // they go.
        _ => println!(
            "\nAccept these changes with `skillmap lock`, or widen policy.toml. \
             Both are reviewable diffs."
        ),
    }

    Ok(outcome.exit_code())
}

/// Read the lock, treating absence as a configuration error rather than as an
/// empty baseline.
///
/// An empty baseline would report every existing bundle as newly added and fail
/// the very first run in a repository, which teaches people that the check cries
/// wolf. Saying "run `skillmap lock`" costs one line and is actionable.
fn read_lock(path: &Path) -> Result<skillmap_diff::Lock, String> {
    match std::fs::read_to_string(path) {
        Ok(text) => skillmap_diff::Lock::from_json(&text).map_err(|error| {
            format!(
                "{} is not a lockfile this build can read: {error}",
                path.display()
            )
        }),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Err(format!(
            "{} does not exist. Run `skillmap lock` and commit it — the check \
             compares against it, and without one there is nothing to compare to.",
            path.display()
        )),
        Err(error) => Err(format!("cannot read {}: {error}", path.display())),
    }
}
