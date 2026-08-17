//! `skillmap` — the binary.
//!
//! Three subcommands. `lock` records what the skills in a project can do today,
//! `ci` fails when that changes, `scan` prints the manifest behind both.
//!
//! ```text
//! $ skillmap ci
//! ✗ example-skill  capability escalation vs skillmap.lock
//!     + fs.read.credential   scripts/collect.py:17   py.credential-read.dotfile
//!       reads ~/.aws/credentials — added in this update
//! ```
//!
//! # Rules come from inside the binary
//!
//! Rules are data (invariant 7), which through T8 meant `--rules` had to point
//! at a checkout — fine for this repository's own CI and useless to everybody
//! else. T9 bakes `rules/` and `queries/` in at build time. `--rules` survives as
//! an override for developing against an edited tree, and `skillmap rules` prints
//! what the running binary actually carries, because "which rules did this
//! version have" is the first question anyone asks about a finding they disagree
//! with.

use skillmap_core::Manifest;
use skillmap_policy::{Outcome, Policy};
use skillmap_resolve::{ClaudeCode, Scope};
use skillmap_rules::RuleSet;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

/// Default lockfile name, in the project root.
const DEFAULT_LOCK: &str = "skillmap.lock";

/// Default policy file name, in the project root.
const DEFAULT_POLICY: &str = "policy.toml";

/// Where a **user-scope** lock lives: `~/.skillmap/user.lock`.
///
/// Deliberately NOT in the project. `~/.claude/skills` is machine state — a
/// different set on every developer's laptop — so committing a lock of it would
/// make `skillmap ci` fail for everyone except whoever generated it, and would
/// put a byte-identical manifest of a per-machine directory into version
/// control. Invariant 2's most obvious failure mode, and the reason T2 deferred
/// this until the answer was decided rather than guessed.
const USER_LOCK_DIR: &str = ".skillmap";
const USER_LOCK: &str = "user.lock";

/// Where a **user-scope** policy lives: `~/.skillmap/policy.toml`.
///
/// It follows the lock rather than the project for the same reason the lock
/// does. A machine-wide check must not change its answer depending on which
/// directory you happened to run it from, and `<project>/policy.toml` describes
/// what THAT project accepts from its own dependencies — a different question
/// from what you accept on your own machine.
///
/// Absent means unconstrained, not empty: `Policy::load` returns `Option`, and
/// the distinction is already load-bearing elsewhere.
const USER_POLICY: &str = "policy.toml";

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
    /// Which install location to scan.
    scope: Scope,
    /// The directory discovery starts from: the project root for
    /// [`Scope::Project`], the user's home directory for [`Scope::User`].
    base: PathBuf,
    /// `None` means the rules baked into this binary.
    rules: Option<PathBuf>,
    lock: PathBuf,
    policy: PathBuf,
    /// Model id for the semantic pass. `None` means it does not run.
    ///
    /// A model id rather than a bare `--advisory` flag, because invariant 6
    /// requires the manifest to pin which model produced the advisory branch,
    /// and a default chosen by the binary would drift under readers who never
    /// typed it.
    advisory_model: Option<String>,
    /// How `scan` renders. JSON is the default because a pipeline is the
    /// contract; the human form exists because a person reading 103 lines of
    /// JSON for two findings is not being served by it.
    format: Format,
}

/// How `scan` renders its manifests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Format {
    /// A canonical JSON array. The default: machine-readable at every count.
    Json,
    /// A few lines a person can read. Not a parseable format and not intended
    /// as one.
    Human,
}

const USAGE: &str = "\
skillmap — a capability differ for AI agent skills

USAGE:
    skillmap lock  [OPTIONS]   write skillmap.lock from the skills in a project
    skillmap ci    [OPTIONS]   fail if capabilities changed, or policy forbids them
    skillmap scan  [OPTIONS]   print what each skill can do, as JSON or for a person
    skillmap rules             list the rules this binary carries
    skillmap version           print the version

OPTIONS:
    --scope <WHICH>    project | user                [default: project]
                       `project` scans <project>/.claude/skills and locks to a
                       file you commit. `user` scans ~/.claude/skills — skills
                       that apply to EVERY project, and that nobody re-reviews —
                       and locks to ~/.skillmap/user.lock, which is machine
                       state and must not be committed.
                       A CI runner has no ~/.claude/skills, so `--scope user`
                       there finds nothing. That is a local check, not a gate.
    --format <WHICH>   json | human                  [default: json]
                       `json` is a canonical ARRAY of manifests — valid at any
                       count, including one and zero. `human` is a few lines per
                       skill for reading, not for parsing.
    --project <DIR>    project root to scan          [default: .]
    --lock <FILE>      lockfile path                 [default: <project>/skillmap.lock,
                       or ~/.skillmap/user.lock under --scope user]
    --policy <FILE>    policy path                   [default: <project>/policy.toml]
    --rules <DIR>      load rules from a checkout instead of the ones built in.
                       For developing against an edited rules tree; a release
                       carries its own and needs no argument.
    --advisory <MODEL> run the semantic pass with this model, e.g. claude-sonnet-5.
                       OFF unless given. This is the only part of skillmap that
                       makes a network call, it needs ANTHROPIC_API_KEY, and it
                       needs a build with --features anthropic. Its findings are
                       tier `advisory` and never change what the other tiers say.

EXIT CODES (ci):
    0  clean
    1  a bundle gained capability it did not have in the lock
    2  a capability is present that policy.toml does not permit
    3  both
    4  the check could not run — bad arguments, no rules, missing lock

Exit 4 is separate on purpose: \"could not run\" must never read as \"ran and
found nothing\".";

/// Version of the binary, from the crate version.
const VERSION: &str = env!("CARGO_PKG_VERSION");

fn run() -> Result<u8, String> {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let Some(command) = argv.first() else {
        println!("{USAGE}");
        return Err("no subcommand given".to_owned());
    };

    match command.as_str() {
        "--help" | "-h" | "help" => {
            println!("{USAGE}");
            return Ok(0);
        }
        "--version" | "-V" | "version" => {
            println!("skillmap {VERSION}");
            return Ok(0);
        }
        _ => {}
    }

    let args = parse_args(argv.get(1..).unwrap_or_default())?;

    match command.as_str() {
        "lock" => write_lock(&args).map(|()| 0),
        "ci" => check(&args),
        "scan" => emit_manifests(&args).map(|()| 0),
        "rules" => list_rules(&args).map(|()| 0),
        other => Err(format!("unknown subcommand `{other}`\n\n{USAGE}")),
    }
}

/// A path as the user should read it: forward slashes, always.
///
/// `Path::display` uses the platform separator, so a path built by joining a
/// forward-slash argument with a constant came out as
/// `…/scratchpad/ux/proj\skillmap.lock` — one backslash in an otherwise
/// forward-slash path, which reads as corruption rather than as Windows. The
/// manifest already normalises paths for exactly this reason (invariant 2); this
/// is the same courtesy for messages, which are not byte-compared but are read.
fn shown(path: &Path) -> String {
    path.display().to_string().replace('\\', "/")
}

/// Parse flags. Every flag takes a value; an unknown flag is an error rather than
/// a warning, because a typo'd `--polcy` that silently used the default would
/// mean CI passed against a policy nobody wrote.
fn parse_args(flags: &[String]) -> Result<Args, String> {
    let mut project: Option<PathBuf> = None;
    let mut rules: Option<PathBuf> = None;
    let mut lock: Option<PathBuf> = None;
    let mut policy: Option<PathBuf> = None;
    let mut advisory_model: Option<String> = None;
    let mut scope = Scope::Project;
    let mut format = Format::Json;

    let mut rest = flags.iter();
    while let Some(flag) = rest.next() {
        let slot = match flag.as_str() {
            "--project" => &mut project,
            "--rules" => &mut rules,
            "--lock" => &mut lock,
            "--policy" => &mut policy,
            // Not a PathBuf, so it cannot share the slot machinery below.
            "--format" => {
                let Some(value) = rest.next() else {
                    return Err("--format needs a value: json or human".to_owned());
                };
                format = match value.as_str() {
                    "json" => Format::Json,
                    "human" => Format::Human,
                    other => {
                        return Err(format!("unknown format `{other}` — expected json or human"))
                    }
                };
                continue;
            }
            "--scope" => {
                let Some(value) = rest.next() else {
                    return Err(format!("`{flag}` needs `project` or `user`"));
                };
                scope = match value.as_str() {
                    "project" => Scope::Project,
                    "user" => Scope::User,
                    other => {
                        return Err(format!(
                            "unknown scope `{other}`; expected `project` or `user`"
                        ))
                    }
                };
                continue;
            }
            "--advisory" => {
                let Some(value) = rest.next() else {
                    return Err(format!(
                        "`{flag}` needs a model id, e.g. --advisory claude-sonnet-5"
                    ));
                };
                advisory_model = Some(value.clone());
                continue;
            }
            other => return Err(format!("unknown option `{other}`\n\n{USAGE}")),
        };
        let Some(value) = rest.next() else {
            return Err(format!("`{flag}` needs a value"));
        };
        *slot = Some(PathBuf::from(value));
    }

    let project = project.unwrap_or_else(|| PathBuf::from("."));

    // The discovery root, and the lock that describes it, both follow the scope.
    let base = match scope {
        Scope::Project => project.clone(),
        Scope::User => home_dir().ok_or_else(|| {
            "cannot find a home directory: neither HOME nor USERPROFILE is set,              so --scope user has nowhere to look. Refusing rather than scanning              an empty set and reporting it clean."
                .to_owned()
        })?,
    };
    let default_lock = match scope {
        Scope::Project => project.join(DEFAULT_LOCK),
        Scope::User => base.join(USER_LOCK_DIR).join(USER_LOCK),
    };
    let default_policy = match scope {
        Scope::Project => project.join(DEFAULT_POLICY),
        Scope::User => base.join(USER_LOCK_DIR).join(USER_POLICY),
    };

    Ok(Args {
        rules,
        lock: lock.unwrap_or(default_lock),
        policy: policy.unwrap_or(default_policy),
        advisory_model,
        format,
        scope,
        base,
        project,
    })
}

/// The user's home directory, from the environment.
///
/// No `dirs` crate: two variables and a filter do not justify a dependency in a
/// tree whose `SECURITY.md` promises a minimal one, and this is the same trade
/// the argument parser already makes.
fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .filter(|path| !path.as_os_str().is_empty())
}

/// Load rules, from `--rules` if given and from the binary otherwise.
///
/// Diagnostics go to stderr either way, and an empty ruleset is fatal. A scanner
/// that loaded nothing reports every project clean, which is the single worst
/// output this tool can produce, so it refuses rather than producing a confident
/// silence.
fn rules(args: &Args) -> Result<RuleSet, String> {
    let set = match &args.rules {
        Some(dir) => skillmap_rules::load(dir),
        None => skillmap_rules::embedded(),
    };

    for diagnostic in &set.diagnostics {
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

    if set.rules.is_empty() {
        return Err(match &args.rules {
            Some(dir) => format!(
                "no rules loaded from {}/rules — every bundle would scan clean, \
                 which would be a lie. Drop --rules to use the ones built into \
                 this binary.",
                shown(dir)
            ),
            None => "this binary carries no rules, which should be impossible — \
                     the build refuses to produce one. Please report it."
                .to_owned(),
        });
    }
    Ok(set)
}

/// `skillmap rules` — what this binary can detect.
///
/// Exists because the rules are no longer visible on disk beside the tool. The
/// first question about a finding somebody disagrees with is which rule produced
/// it and what that rule claims, and a released binary has to be able to answer
/// that without a checkout.
fn list_rules(args: &Args) -> Result<(), String> {
    let set = rules(args)?;
    println!("skillmap {VERSION} — {} rules", set.rules.len());
    for rule in &set.rules {
        let claim = match rule.claim {
            skillmap_rules::Claim::Capability(term) => term.as_str(),
            skillmap_rules::Claim::Instruction(signal) => signal.as_str(),
        };
        println!("  {:<32} {:<12} {claim}", rule.id, rule.language);
    }
    Ok(())
}

/// `skillmap scan` — the manifest, canonically serialized.
///
/// A JSON **array** of manifests, or a short human summary.
///
/// The array is not cosmetic. This printed one object per bundle, concatenated,
/// which is valid JSON for one skill and two top-level objects for two — so
/// `skillmap scan | jq .` failed on every project with more than one skill,
/// which is most of them.
fn emit_manifests(args: &Args) -> Result<(), String> {
    let manifests = scan(args)?;
    match args.format {
        Format::Json => {
            let json = Manifest::many_to_canonical_json(&manifests)
                .map_err(|error| format!("cannot serialize the manifests: {error}"))?;
            print!("{json}");
        }
        Format::Human => print!("{}", render_human(&manifests)),
    }
    Ok(())
}

/// What a person needs from a manifest, in a few lines.
///
/// The JSON is right for a pipeline and wrong for a reader: one small skill with
/// two findings renders 103 lines of it. This is deliberately not a second
/// serialization format — nothing parses it, nothing round-trips through it, and
/// it says so by omitting every field a machine would want and keeping the ones
/// a person reads.
///
/// Unresolved entries are summarised rather than listed, and never omitted. A
/// short report that hides them would say "clean" where the tool means "clean,
/// and I could not read four things" — which is the distinction invariant 3
/// exists to preserve.
fn render_human(manifests: &[Manifest]) -> String {
    use std::fmt::Write as _;
    let mut out = String::new();
    if manifests.is_empty() {
        out.push_str("no skills found\n");
        return out;
    }
    for manifest in manifests {
        // The root repeats the name for a top-level skill, which is the common
        // case; print it only when it adds something.
        if manifest.target.root == manifest.target.name {
            let _ = writeln!(out, "{}", manifest.target.name);
        } else {
            let _ = writeln!(out, "{}  ({})", manifest.target.name, manifest.target.root);
        }
        if manifest.capabilities.is_empty() && manifest.instructions.is_empty() {
            let _ = writeln!(out, "    nothing detected");
        }
        for capability in &manifest.capabilities {
            let evidence = capability.evidence.first();
            let (file, line) = evidence.map_or(("?", 0), |first| {
                (first.file.as_str(), first.start_line.get())
            });
            let detail = capability
                .detail
                .as_ref()
                .map(|detail| {
                    let mut parts: Vec<String> = detail.paths.clone().unwrap_or_default();
                    parts.extend(detail.hosts.clone().unwrap_or_default());
                    parts.join(", ")
                })
                .unwrap_or_default();
            let _ = writeln!(
                out,
                "    {:<26} {}:{}  [{}]{}",
                capability.capability.as_str(),
                file,
                line,
                // The wire name, not a friendlier synonym. A reader who sees
                // "runs" here and "observed" in the JSON has been given two
                // vocabularies for one field.
                capability.reachability.as_str(),
                if detail.is_empty() {
                    String::new()
                } else {
                    format!("  {detail}")
                }
            );
        }
        for instruction in &manifest.instructions {
            let (file, line) = instruction.evidence.first().map_or(("?", 0), |first| {
                (first.file.as_str(), first.start_line.get())
            });
            let _ = writeln!(
                out,
                "    {:<26} {}:{}  [prose]",
                instruction.signal.as_str(),
                file,
                line
            );
        }
        if !manifest.unresolved.is_empty() {
            let _ = writeln!(
                out,
                "    {} thing(s) the analysis could not resolve — see --format json",
                manifest.unresolved.len()
            );
        }
    }
    out
}

/// Build the semantic pass's provider, if `--advisory` asked for one.
///
/// Returns `None` when the flag was absent — the default, and the only state in
/// which `skillmap` makes no network call at all (invariant 9).
///
/// When the flag is present but the binary was built without the `anthropic`
/// feature, this is an error rather than a silently disabled pass. Somebody who
/// typed `--advisory` and got a manifest with `"enabled": false` would reasonably
/// read it as "checked, found nothing".
#[allow(
    unused_variables,
    reason = "args is unused in the default build, where there is no provider to construct"
)]
fn advisory_provider(args: &Args) -> Result<Option<Box<dyn skillmap_scan::Provider>>, String> {
    #[cfg(feature = "anthropic")]
    {
        let Some(model) = &args.advisory_model else {
            return Ok(None);
        };
        eprintln!(
            "skillmap: semantic pass enabled with `{model}`. This is the only \
             network call skillmap makes. Its findings are tier `advisory` and \
             change nothing the other tiers report."
        );
        let provider = skillmap_semantic::provider::Anthropic::from_env(model)
            .map_err(|error| error.to_string())?;
        Ok(Some(Box::new(provider)))
    }

    #[cfg(not(feature = "anthropic"))]
    {
        match &args.advisory_model {
            None => Ok(None),
            Some(_) => Err(
                "--advisory needs a build with the model provider compiled in. \
                 Rebuild with `cargo build --release -p skillmap-cli --features \
                 skillmap-semantic/anthropic`. Released binaries ship without it, \
                 so that a default install of a supply-chain auditor cannot make \
                 a network call."
                    .to_owned(),
            ),
        }
    }
}

/// Discover and scan every bundle in the project.
///
/// Rule-loading diagnostics and undiscoverable directories both go to stderr and
/// neither is swallowed: a run that could not look at a bundle must not read the
/// same as a run that looked and found nothing (invariant 3).
fn scan(args: &Args) -> Result<Vec<Manifest>, String> {
    let rules = rules(args)?;

    let discovery = skillmap_resolve::discover(&ClaudeCode, &args.base, args.scope)
        .map_err(|error| format!("cannot discover skills: {error}"))?;

    // Said out loud, every run, because a zero here is the quiet failure this
    // scope invites: a CI runner has no `~/.claude/skills`, so `--scope user`
    // there would find nothing, exit 0, and read exactly like a clean bill of
    // health. "Could not look" must never look like "looked and found nothing"
    // (invariant 3).
    eprintln!(
        "skillmap: {} bundle(s) under {} ({:?} scope)",
        discovery.bundles.len(),
        shown(&args.base),
        args.scope
    );

    for skipped in &discovery.skipped {
        eprintln!(
            "skillmap: skipped {} — {}",
            shown(&skipped.path),
            skipped.reason
        );
    }

    let provider = advisory_provider(args)?;

    let mut manifests = Vec::new();
    for bundle in &discovery.bundles {
        let path = bundle.path();
        let manifest = match provider.as_deref() {
            Some(provider) => skillmap_scan::analyze_bundle_advised(
                &path,
                &rules,
                &ClaudeCode,
                provider,
                &skillmap_scan::SemanticLimits::default(),
            ),
            None => skillmap_scan::analyze(&path, &rules),
        }
        .map_err(|error| format!("cannot analyze {}: {error}", shown(&path)))?;
        manifests.push(manifest);
    }

    if manifests.is_empty() && discovery.skipped.is_empty() {
        eprintln!(
            "skillmap: no skills found under {}/.claude/skills",
            shown(&args.project)
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
    // `~/.skillmap/` will not exist on a first user-scope run.
    if let Some(parent) = args.lock.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .map_err(|error| format!("cannot create {}: {error}", shown(parent)))?;
        }
    }
    std::fs::write(&args.lock, json)
        .map_err(|error| format!("cannot write {}: {error}", shown(&args.lock)))?;

    println!(
        "wrote {} — {} bundle(s)",
        shown(&args.lock),
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
                shown(&args.policy)
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
                shown(path)
            )
        }),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Err(format!(
            "{} does not exist. Run `skillmap lock` and commit it — the check \
             compares against it, and without one there is nothing to compare to.",
            shown(path)
        )),
        Err(error) => Err(format!("cannot read {}: {error}", shown(path))),
    }
}
