//! `skillmap hook` — the check that runs without being remembered.
//!
//! Every other command in this tool has the same defect: somebody has to run it.
//! Skills update themselves, which is the whole premise of the product, and the
//! answer so far has been "re-run the differ" — which nobody does monthly, or
//! ever. A differ you must remember is a differ nobody runs.
//!
//! So `skillmap hook install` registers a `SessionStart` hook in the agent's own
//! configuration, and the agent runs the check at the start of every session.
//!
//! # This writes to the user's agent configuration, which is a thing skillmap
//! # reports on
//!
//! `fs.write.agent_config` is a capability term here, and
//! `instruction.directs_outside_write` is a signal that fires on prose telling
//! an agent to do exactly this. Writing `~/.claude/settings.json` is not
//! exempted by being us. It is made acceptable by being:
//!
//! - **explicit** — it happens on `hook install` and never on package install,
//! - **previewed** — the exact JSON is printed before anything is written,
//! - **backed up** — the previous file is copied to `settings.json.bak`,
//! - **idempotent** — installing twice changes nothing,
//! - **reversible** — `hook uninstall` removes precisely what was added.
//!
//! # The hook can never fail a session
//!
//! `hook run` always exits 0, whatever it finds. This repository's own
//! development hooks already argue the case: *"a hook that fights the author
//! gets disabled"*. A session-start check that could abort a session because a
//! skill changed would be turned off within a day, and then the drift it exists
//! to catch goes unwatched — which is worse than not shipping it.
//!
//! Findings go to stdout for the human to read. The exit code stays 0.
//!
//! # On `serde_json::to_string_pretty` appearing here
//!
//! `AGENTS.md` forbids that call escaping outside `skillmap-core::canonical` —
//! for **manifest types**, because `canonicalize()` is what enforces sorted keys
//! and declared array orders, and a second serialization path is a byte-identity
//! leak. None of that applies to somebody's `settings.json`: it is not a
//! manifest, nothing diffs it byte-for-byte, and no invariant governs its shape.
//! `scripts/hooks/check_library_lints.py` exempts this crate for the same
//! reason.

use std::path::{Path, PathBuf};

/// A path as the user should read it: forward slashes, always.
///
/// The same courtesy the rest of the CLI extends. `Path::display` uses the
/// platform separator, and these messages name a file the user may go and edit —
/// a mixed-separator path reads as corruption rather than as Windows.
fn shown(path: &Path) -> String {
    path.display().to_string().replace('\\', "/")
}

/// The event to hook. `SessionStart` fires once when a session begins, which is
/// the moment a changed skill is about to be loaded and the last moment the
/// change is still news.
const EVENT: &str = "SessionStart";

/// The command registered. A stable entry point on purpose: it is written into
/// the user's configuration, so changing what the check does must not require
/// every installed hook to be rewritten.
const COMMAND: &str = "skillmap hook run";

/// Where Claude Code keeps user-level settings.
///
/// Only this agent for now. The others read `SKILL.md` but each has its own
/// configuration format, and writing a guessed schema into somebody's agent
/// config is worse than not supporting them — a wrong guess is a broken agent,
/// not a missing feature. The shape below takes the path as an argument so a
/// second agent is a table entry rather than a rewrite.
pub fn settings_path(home: &Path) -> PathBuf {
    home.join(".claude").join("settings.json")
}

/// Read the settings file, or an empty object when it does not exist.
///
/// A malformed file is an error rather than something to overwrite. Somebody's
/// agent configuration is not ours to replace because we could not parse it.
fn read_settings(path: &Path) -> Result<serde_json::Value, String> {
    match std::fs::read_to_string(path) {
        Ok(text) => serde_json::from_str(&text).map_err(|error| {
            format!(
                "{} is not valid JSON ({error}). Fix or move it — refusing to \
                 overwrite an agent configuration this cannot read.",
                shown(path)
            )
        }),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(serde_json::json!({})),
        Err(error) => Err(format!("cannot read {}: {error}", shown(path))),
    }
}

/// Whether our command is already registered for the event.
fn installed_in(settings: &serde_json::Value) -> bool {
    settings
        .get("hooks")
        .and_then(|hooks| hooks.get(EVENT))
        .and_then(serde_json::Value::as_array)
        .is_some_and(|entries| {
            entries.iter().any(|entry| {
                entry
                    .get("hooks")
                    .and_then(serde_json::Value::as_array)
                    .is_some_and(|inner| {
                        inner.iter().any(|hook| {
                            hook.get("command").and_then(serde_json::Value::as_str) == Some(COMMAND)
                        })
                    })
            })
        })
}

/// The block this adds, as it will appear in the file.
fn block() -> serde_json::Value {
    serde_json::json!({ "hooks": [ { "type": "command", "command": COMMAND } ] })
}

/// Report whether the hook is registered, and where.
pub fn status(home: &Path) -> Result<(), String> {
    let path = settings_path(home);
    let settings = read_settings(&path)?;
    if installed_in(&settings) {
        println!("installed  {EVENT} -> `{COMMAND}`  in {}", shown(&path));
    } else {
        println!("not installed  ({})", shown(&path));
        println!("  `skillmap hook install` registers it.");
    }
    Ok(())
}

/// Register the hook, preserving everything else in the file.
///
/// Returns the path written, or `None` when it was already present — installing
/// twice must not append a second copy, or the check runs twice per session and
/// the user's first instinct is to delete both.
pub fn install(home: &Path) -> Result<Option<PathBuf>, String> {
    let path = settings_path(home);
    let mut settings = read_settings(&path)?;
    if installed_in(&settings) {
        println!("already installed  {EVENT} -> `{COMMAND}`");
        return Ok(None);
    }

    let entries = settings
        .as_object_mut()
        .ok_or_else(|| format!("{} is not a JSON object", shown(&path)))?
        .entry("hooks")
        .or_insert_with(|| serde_json::json!({}))
        .as_object_mut()
        .ok_or_else(|| format!("`hooks` in {} is not an object", shown(&path)))?
        .entry(EVENT)
        .or_insert_with(|| serde_json::json!([]));
    entries
        .as_array_mut()
        .ok_or_else(|| format!("`hooks.{EVENT}` in {} is not an array", shown(&path)))?
        .push(block());

    let rendered = serde_json::to_string_pretty(&settings)
        .map_err(|error| format!("cannot render settings: {error}"))?;

    println!("adding to {}:\n", shown(&path));
    println!(
        "{}\n",
        serde_json::to_string_pretty(&serde_json::json!({ "hooks": { EVENT: [block()] } }))
            .unwrap_or_else(|_| String::from("  (unrenderable)"))
    );

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("cannot create {}: {error}", shown(parent)))?;
    }
    // A backup before touching somebody's agent configuration. Cheap, and the
    // difference between a reversible change and an apology.
    if path.exists() {
        let backup = path.with_extension("json.bak");
        std::fs::copy(&path, &backup)
            .map_err(|error| format!("cannot back up to {}: {error}", shown(&backup)))?;
        println!("previous file backed up to {}", shown(&backup));
        println!("note: JSON keys are rewritten in sorted order.");
    }
    std::fs::write(&path, rendered + "\n")
        .map_err(|error| format!("cannot write {}: {error}", shown(&path)))?;
    Ok(Some(path))
}

/// Remove exactly what `install` added, and nothing else.
pub fn uninstall(home: &Path) -> Result<bool, String> {
    let path = settings_path(home);
    let mut settings = read_settings(&path)?;
    if !installed_in(&settings) {
        println!("not installed  ({})", shown(&path));
        return Ok(false);
    }

    if let Some(entries) = settings
        .get_mut("hooks")
        .and_then(|hooks| hooks.get_mut(EVENT))
        .and_then(serde_json::Value::as_array_mut)
    {
        // Drop only entries whose sole command is ours. An entry a user has
        // added their own commands to is left alone: removing a line they wrote
        // because it sits beside a line we wrote would be worse than leaving
        // ours behind.
        entries.retain(|entry| {
            let ours = entry
                .get("hooks")
                .and_then(serde_json::Value::as_array)
                .is_some_and(|inner| {
                    inner.len() == 1
                        && inner.iter().all(|hook| {
                            hook.get("command").and_then(serde_json::Value::as_str) == Some(COMMAND)
                        })
                });
            !ours
        });
    }

    let rendered = serde_json::to_string_pretty(&settings)
        .map_err(|error| format!("cannot render settings: {error}"))?;
    std::fs::write(&path, rendered + "\n")
        .map_err(|error| format!("cannot write {}: {error}", shown(&path)))?;
    println!("removed  {EVENT} -> `{COMMAND}`  from {}", shown(&path));
    Ok(true)
}
