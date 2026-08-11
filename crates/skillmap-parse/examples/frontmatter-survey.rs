//! Survey why the strict frontmatter parser refuses real bundles.
//!
//! `docs/00-tasks.md` deferred the question "is refusing non-subset YAML
//! tenable?" to T3's corpus, and the answer came back **28% refused**. A refusal
//! rate without a breakdown is not actionable — widening a parser on a guess is
//! how the subset got chosen too narrowly in the first place — so this walks an
//! archive of bundles and tallies the actual error messages.
//!
//! ```text
//! cargo run -p skillmap-parse --example frontmatter-survey -- corpus/raw
//! ```
//!
//! Reads only local files. No network, no token.

#![allow(
    clippy::print_stdout,
    clippy::print_stderr,
    reason = "a diagnostic example; stdout is its entire output"
)]
#![allow(
    clippy::integer_division,
    reason = "percentages are integer arithmetic throughout this project; see the               note in skillmap-corpus::measure"
)]

use skillmap_parse::frontmatter;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

fn main() {
    let root = std::env::args()
        .nth(1)
        .map_or_else(|| PathBuf::from("corpus/raw"), PathBuf::from);

    let mut total = 0u64;
    let mut refused = 0u64;
    // Keyed by a normalised message so counts aggregate, with one real example
    // kept per class so the fix can be checked against actual text.
    let mut classes: BTreeMap<String, (u64, String)> = BTreeMap::new();

    for skill in find_skill_files(&root) {
        let Ok(text) = std::fs::read_to_string(&skill) else {
            continue;
        };
        total += 1;
        if let Err(error) = frontmatter::parse(&text) {
            refused += 1;
            let class = classify(&error.message);
            let entry = classes
                .entry(class)
                .or_insert_with(|| (0, error.message.clone()));
            entry.0 += 1;
        }
    }

    println!("SKILL.md files scanned: {total}");
    println!("refused: {refused} ({}%)\n", share(refused, total));

    let mut ranked: Vec<(&String, &(u64, String))> = classes.iter().collect();
    ranked.sort_by(|a, b| b.1 .0.cmp(&a.1 .0).then(a.0.cmp(b.0)));

    println!("{:>7}  {:<34}  example", "count", "class");
    for (class, (count, example)) in ranked {
        let share = share(*count, refused);
        println!(
            "{count:>7}  {class:<34}  {share:>3}%  {}",
            example.chars().take(90).collect::<String>()
        );
    }
}

/// `numerator` as a whole-number percentage of `denominator`.
fn share(numerator: u64, denominator: u64) -> u64 {
    numerator
        .saturating_mul(100)
        .checked_div(denominator)
        .unwrap_or(0)
}

/// Bucket an error message into a failure class.
///
/// The messages carry specifics — a key name, a token — so raw counts would be
/// one bucket per bundle. This groups by the constraint that was violated, which
/// is the unit a parser change actually addresses.
fn classify(message: &str) -> String {
    let lowered = message.to_lowercase();
    for (needle, class) in [
        ("duplicate key", "duplicate-key"),
        ("nested structures", "flow-mapping"),
        ("nested flow", "nested-flow-sequence"),
        ("anchors", "anchor"),
        ("aliases", "alias"),
        ("merge keys", "merge-key"),
        ("directives", "directive"),
        ("document end", "document-end"),
        ("never closed", "unterminated-block"),
        ("not closed on the same line", "multiline-flow-sequence"),
        ("does not follow", "orphan-list-item"),
        ("block scalar", "unterminated-block-scalar"),
        ("empty key", "empty-key"),
        ("first line", "no-opening-fence"),
        ("file is empty", "empty-file"),
        ("expected `key: value`", "unrecognised-line"),
    ] {
        if lowered.contains(needle) {
            return class.to_owned();
        }
    }
    "other".to_owned()
}

/// Every `SKILL.md` beneath `root`.
fn find_skill_files(root: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut work = vec![root.to_path_buf()];
    while let Some(dir) = work.pop() {
        if dir.file_name().is_some_and(|name| name == ".git") {
            continue;
        }
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        // A directory holding a SKILL.md *is* the bundle: its reference files are
        // part of it, not further bundles. Stopping here mirrors the harvester's
        // own discovery and avoids walking a million reference files to find
        // nothing — on a 34,000-bundle archive that is the difference between
        // seconds and half an hour.
        let skill = dir.join("SKILL.md");
        if skill.is_file() {
            found.push(skill);
            continue;
        }
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                work.push(path);
            }
        }
    }
    found.sort();
    found
}
