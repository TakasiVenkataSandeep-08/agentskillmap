//! T8's acceptance test, run as a process.
//!
//! `docs/00-tasks.md`: *"a fixture skill that gains `fs.read.credential` in v1.1
//! causes a failing check whose output a reviewer can act on in under ten
//! seconds. **This is the product**"*.
//!
//! Every assertion below is one clause of that sentence. It runs the real
//! binary against the real rules tree rather than calling the library, because
//! the exit code is half of what CI consumes and a library test cannot observe
//! one.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "a failed unwrap in a test is the test failing"
)]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// The repository root — where `rules/` and `fixtures/` live.
fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .unwrap()
}

/// A scratch path unique to this test run.
fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("skillmap-t8-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir.join(name)
}

fn skillmap(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_skillmap"))
        .args(args)
        .current_dir(repo_root())
        .output()
        .unwrap()
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// Lock v1.0, then check v1.1 against it.
fn lock_v1_0(name: &str) -> PathBuf {
    let lock = scratch(name);
    let output = skillmap(&[
        "lock",
        "--project",
        "fixtures/projects/v1.0",
        "--rules",
        ".",
        "--lock",
        &lock.display().to_string(),
    ]);
    assert!(
        output.status.success(),
        "`skillmap lock` failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    lock
}

#[test]
fn a_skill_that_gains_credential_access_fails_the_check() {
    let lock = lock_v1_0("escalation.lock");
    let output = skillmap(&[
        "ci",
        "--project",
        "fixtures/projects/v1.1",
        "--rules",
        ".",
        "--lock",
        &lock.display().to_string(),
    ]);

    assert_eq!(
        output.status.code(),
        Some(1),
        "escalation must exit 1; got {:?}\n{}",
        output.status.code(),
        stdout(&output)
    );

    let report = stdout(&output);

    // The four things a reviewer needs, and the ten seconds are spent on this
    // line: which bundle, what it gained, where to look, and which rule said so.
    assert!(
        report.contains("✗ example-skill  capability escalation vs skillmap.lock"),
        "missing the headline:\n{report}"
    );
    assert!(
        report.contains("+ fs.read.credential"),
        "missing the capability:\n{report}"
    );
    assert!(
        report.contains("scripts/collect.py:17"),
        "missing the file and line:\n{report}"
    );
    assert!(
        report.contains("py.credential-read.dotfile"),
        "missing the rule id — without it the finding cannot be appealed:\n{report}"
    );
    assert!(
        report.contains("added in this update"),
        "missing the reason this is different from a standing capability:\n{report}"
    );

    // Short enough to actually read. A report that scrolls is a report that gets
    // skimmed, and the whole claim is that this one does not.
    assert!(
        report.lines().count() <= 8,
        "the report is {} lines; the budget is ten seconds:\n{report}",
        report.lines().count()
    );
}

#[test]
fn the_same_bundle_against_its_own_lock_is_silent() {
    // Half of `docs/05-eval.md`'s argument: a check that cannot stay quiet gets
    // muted, and then it protects nobody.
    let lock = lock_v1_0("clean.lock");
    let output = skillmap(&[
        "ci",
        "--project",
        "fixtures/projects/v1.0",
        "--rules",
        ".",
        "--lock",
        &lock.display().to_string(),
    ]);

    assert_eq!(output.status.code(), Some(0), "{}", stdout(&output));
    assert!(
        !stdout(&output).contains('✗'),
        "no failure marker on an unchanged bundle:\n{}",
        stdout(&output)
    );
}

#[test]
fn a_capability_outside_policy_exits_two_even_when_the_lock_agrees() {
    // The two questions are independent: this bundle has held the capability
    // since the lock was written, so the diff has nothing to say, and the policy
    // still does. If these shared an exit code a consumer could not tell them
    // apart.
    let lock = scratch("policy.lock");
    let policy = scratch("empty-policy.toml");
    std::fs::write(&policy, "[allow]\ncapabilities = []\n").unwrap();

    let written = skillmap(&[
        "lock",
        "--project",
        "fixtures/projects/v1.1",
        "--rules",
        ".",
        "--lock",
        &lock.display().to_string(),
    ]);
    assert!(written.status.success());

    let output = skillmap(&[
        "ci",
        "--project",
        "fixtures/projects/v1.1",
        "--rules",
        ".",
        "--lock",
        &lock.display().to_string(),
        "--policy",
        &policy.display().to_string(),
    ]);

    assert_eq!(output.status.code(), Some(2), "{}", stdout(&output));
    assert!(
        stdout(&output).contains("capability not allowed by policy.toml"),
        "{}",
        stdout(&output)
    );

    // And allowing it makes the run clean, which is what makes the allowlist a
    // real answer rather than a way to turn the tool off.
    //
    // Both terms, because the bundle genuinely has both: `~/.aws/credentials` is
    // a credential path AND outside the bundle, and each is reported by its own
    // rule. This listed only the credential term until `fs.read.outside_bundle`
    // shipped, at which point the "allowed" run correctly exited 2 — the policy
    // did not permit everything the scan found. Widening the taxonomy widens
    // what an allowlist has to say, which is the system working rather than a
    // test needing appeasement.
    std::fs::write(
        &policy,
        "[allow]\ncapabilities = [\"fs.read.credential\", \"fs.read.outside_bundle\"]\n",
    )
    .unwrap();
    let allowed = skillmap(&[
        "ci",
        "--project",
        "fixtures/projects/v1.1",
        "--rules",
        ".",
        "--lock",
        &lock.display().to_string(),
        "--policy",
        &policy.display().to_string(),
    ]);
    assert_eq!(allowed.status.code(), Some(0), "{}", stdout(&allowed));
}

#[test]
fn a_missing_lock_exits_four_and_says_what_to_run() {
    // Invariant 3 at the process boundary: "could not run" must never be
    // reported with the same code as "ran and found nothing".
    let output = skillmap(&[
        "ci",
        "--project",
        "fixtures/projects/v1.1",
        "--rules",
        ".",
        "--lock",
        &scratch("absent.lock").display().to_string(),
    ]);

    assert_eq!(output.status.code(), Some(4));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("skillmap lock"), "{stderr}");
}

#[test]
fn an_empty_ruleset_refuses_to_report_a_clean_scan() {
    // The worst possible outcome for this tool is a confident silence produced
    // by a scanner that loaded no rules. `--rules` pointed at a directory with
    // no `rules/` must fail loudly, not pass.
    let empty = scratch("no-rules-here");
    std::fs::create_dir_all(&empty).unwrap();

    let output = skillmap(&[
        "ci",
        "--project",
        "fixtures/projects/v1.0",
        "--rules",
        &empty.display().to_string(),
        "--lock",
        &scratch("clean.lock").display().to_string(),
    ]);

    assert_eq!(output.status.code(), Some(4));
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("no rules loaded"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
