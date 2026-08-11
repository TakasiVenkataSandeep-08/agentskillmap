//! Mechanical measurement of a harvested bundle.
//!
//! Two very different kinds of number live here, and keeping them apart is the
//! whole integrity of the report:
//!
//! **Structural facts** come from the parser. File counts, bytes per load phase,
//! whether anything is unreferenced, which languages appear — these are exact,
//! and they are the same numbers the manifest reports.
//!
//! **Lexical counts** come from substring matching over file text. They are not
//! parsing. `"~/.aws/credentials"` inside a comment, inside a docstring warning
//! people *not* to read credentials, or inside a URL, all count the same. They
//! establish nothing about reachability and carry no provenance, so they can
//! never be a finding (invariants 4 and 5) and never enter a `Manifest`.
//!
//! They are still worth measuring. The question T3 has to answer is whether this
//! ecosystem is worth building a scanner for, and "how many bundles so much as
//! mention a credential path" is a legitimate, cheap upper bound on that — as
//! long as the report says it is an upper bound, which [`crate::report`] does.
//! T4's rule engine replaces every one of these with a structural answer.

#![allow(
    clippy::integer_division,
    reason = "integer division is the deliberate policy, not an oversight. This               project has no floats anywhere: they would print differently on               different platforms in an artifact that must be byte-identical, and               a rate expressed as a float is one edit away from being a score               (invariant 1). Shares are carried as parts-per-million and rates as               tenths of a percent, and the truncation is intended."
)]

use serde::{Deserialize, Serialize};
use skillmap_core::{LoadPhase, Manifest};
use skillmap_parse::inventory::WalkedFile;
use std::collections::BTreeMap;

/// Everything measured about one bundle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Measurements {
    /// Exact, from the parser.
    pub structure: Structure,
    /// Substring matches. An upper bound, never a finding.
    pub lexical: Lexical,
    /// Exact, from the repository and frontmatter.
    pub governance: Governance,
}

/// Structural facts, all exact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Structure {
    /// Files in the bundle.
    pub files: u64,
    /// Total bytes hashed across the bundle.
    pub total_bytes: u64,
    /// Bytes in files reachable only through the `reference` phase.
    pub reference_bytes: u64,
    /// Bytes in files nothing points at.
    pub unreferenced_bytes: u64,
    /// Size of the always-loaded frontmatter description.
    pub description_bytes: u64,
    /// Whether any file is unreferenced.
    pub has_unreferenced: bool,
    /// Whether any file is executable code rather than prose.
    pub has_scripts: bool,
    /// Count of files per `parsed_as` language.
    pub languages: BTreeMap<String, u64>,
    /// Count of `unresolved` entries per reason.
    pub unresolved: BTreeMap<String, u64>,
}

impl Structure {
    /// Description bytes as a fraction of total bundle bytes, in **parts per
    /// million**, or `None` for an empty bundle.
    ///
    /// Integer parts-per-million rather than a float on purpose. This number is
    /// serialized, diffed between snapshots, and compared across machines; a
    /// float would print differently on different platforms and would be the one
    /// float in a project that has none anywhere else (invariant 1's neighbour).
    /// The report divides it back down for display.
    #[must_use]
    pub fn description_share_ppm(&self) -> Option<u64> {
        if self.total_bytes == 0 {
            return None;
        }
        Some(self.description_bytes.saturating_mul(1_000_000) / self.total_bytes)
    }
}

/// Substring matches over bundle text. Upper bounds, not findings.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Lexical {
    /// Mentions a conventional credential path.
    pub credential_paths: bool,
    /// Mentions a secret-bearing environment variable name.
    pub secret_env: bool,
    /// Mentions outbound network machinery.
    pub network: bool,
    /// Mentions writing agent configuration.
    pub agent_config_write: bool,
    /// Mentions dynamic evaluation or subprocess execution.
    pub dynamic_eval: bool,
    /// Mentions install-time fetching, e.g. `postinstall` or `curl | sh`.
    pub install_fetch: bool,
    /// Mentions an encode/decode chain.
    pub encoding_chain: bool,
    /// Which of the above appear only in files the body never points at.
    ///
    /// The disclosure-delta shape, measured lexically: machinery that exists in
    /// a bundle but in a file no documented path reaches. It is the single most
    /// interesting column in the report, and still only a lead, not a finding.
    pub only_in_unreferenced: Vec<String>,
}

/// Repository and frontmatter facts, all exact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Governance {
    /// A LICENSE file exists in the bundle.
    pub has_license: bool,
    /// Frontmatter carries any version marker.
    pub has_version: bool,
    /// Frontmatter keys beyond `name` and `description`, sorted.
    ///
    /// The input to the format-spread question: whether the ecosystem uses
    /// anything a single parser cannot absorb.
    pub extra_frontmatter_keys: Vec<String>,
    /// Whether the frontmatter parsed at all under the strict subset parser.
    ///
    /// A `false` here is a measurement in its own right: it says the ecosystem
    /// uses YAML this project deliberately refuses, and it is the number that
    /// decides whether that refusal is tenable.
    pub frontmatter_parsed: bool,
}

/// A lexical marker set: a column name and the substrings that set it.
struct Marker {
    name: &'static str,
    needles: &'static [&'static str],
}

/// Every lexical marker, in report order.
///
/// Data, not code, in the same spirit as invariant 7 — extending coverage here is
/// editing a list. These are not detection rules and are not a substitute for
/// them; a rule triple under `rules/` is what T4 will use.
const MARKERS: &[Marker] = &[
    Marker {
        name: "credential_paths",
        needles: &[
            "~/.aws",
            ".aws/credentials",
            "~/.ssh",
            ".ssh/id_",
            "~/.config/gh",
            "~/.kube",
            ".netrc",
            ".docker/config.json",
            "keychain",
            ".env",
        ],
    },
    Marker {
        name: "secret_env",
        needles: &[
            "_TOKEN",
            "_KEY",
            "_SECRET",
            "_PASSWORD",
            "GITHUB_TOKEN",
            "OPENAI_API",
            "ANTHROPIC_API",
        ],
    },
    Marker {
        name: "network",
        needles: &[
            "http://",
            "https://",
            "requests.get",
            "requests.post",
            "urllib",
            "httpx",
            "axios",
            "fetch(",
            "curl ",
            "wget ",
        ],
    },
    Marker {
        name: "agent_config_write",
        needles: &[
            "CLAUDE.md",
            "settings.json",
            ".claude/hooks",
            "statusline",
            ".mcp.json",
        ],
    },
    Marker {
        name: "dynamic_eval",
        needles: &[
            "eval(",
            "exec(",
            "os.system",
            "subprocess",
            "child_process",
            "Function(",
            "source ",
            "sh -c",
            "bash -c",
        ],
    },
    Marker {
        name: "install_fetch",
        needles: &[
            "postinstall",
            "curl -fsSL",
            "curl | sh",
            "curl | bash",
            "iwr ",
            "install.sh",
        ],
    },
    Marker {
        name: "encoding_chain",
        needles: &[
            "base64",
            "b64decode",
            "atob(",
            "fromCharCode",
            "codecs.decode",
        ],
    },
];

/// Measure one parsed bundle.
///
/// `manifest` supplies the structural facts; `files` supplies the text the
/// lexical pass matches over. Both come from the same walk, so they cannot
/// disagree about which files exist.
#[must_use]
pub fn measure(
    manifest: &Manifest,
    files: &[WalkedFile],
    frontmatter_parsed: bool,
    extra_keys: Vec<String>,
    has_version: bool,
) -> Measurements {
    Measurements {
        structure: structure(manifest),
        lexical: lexical(manifest, files),
        governance: Governance {
            has_license: files.iter().any(|file| {
                let name = file.path.rsplit('/').next().unwrap_or(&file.path);
                name.eq_ignore_ascii_case("LICENSE")
                    || name.to_ascii_uppercase().starts_with("LICENSE.")
            }),
            has_version,
            extra_frontmatter_keys: extra_keys,
            frontmatter_parsed,
        },
    }
}

/// Structural facts straight out of the manifest.
fn structure(manifest: &Manifest) -> Structure {
    let mut languages: BTreeMap<String, u64> = BTreeMap::new();
    let mut reference_bytes = 0u64;
    let mut unreferenced_bytes = 0u64;
    let mut total_bytes = 0u64;

    for entry in &manifest.inventory {
        *languages.entry(entry.parsed_as.clone()).or_default() += 1;
        total_bytes = total_bytes.saturating_add(entry.size);
        match entry.load_phase {
            LoadPhase::Reference => reference_bytes = reference_bytes.saturating_add(entry.size),
            LoadPhase::Unreferenced => {
                unreferenced_bytes = unreferenced_bytes.saturating_add(entry.size);
            }
            LoadPhase::Always | LoadPhase::OnTrigger => {}
        }
    }

    let mut unresolved: BTreeMap<String, u64> = BTreeMap::new();
    for entry in &manifest.unresolved {
        *unresolved
            .entry(entry.reason.as_str().to_owned())
            .or_default() += 1;
    }

    // "Scripts" means a language that executes, not merely a non-markdown file.
    // A bundle shipping a JSON config is not shipping code.
    const EXECUTABLE: &[&str] = &[
        "python",
        "javascript",
        "typescript",
        "shell",
        "ruby",
        "rust",
        "go",
    ];

    Structure {
        files: manifest.inventory.len() as u64,
        total_bytes,
        reference_bytes,
        unreferenced_bytes,
        description_bytes: manifest.disclosure.description_bytes,
        has_unreferenced: manifest.disclosure.unreferenced_files > 0,
        has_scripts: languages
            .keys()
            .any(|lang| EXECUTABLE.contains(&lang.as_str())),
        languages,
        unresolved,
    }
}

/// Whether `needle` appears in `text` at a word boundary.
///
/// A plain `contains` is too loose for needles that end in a letter or digit, and
/// the failure is not hypothetical: `.env` matches inside `os.environ`, which made
/// a bundle whose only real credential reference sat in an unreferenced file look
/// as though it also referenced one from a file the body points at. That wiped out
/// the "only in unreferenced files" column — the single most interesting number in
/// the report — without changing any total, so it would have been invisible in
/// aggregate. `_KEY` inside `_KEYWORD` and `_TOKEN` inside `_TOKENIZER` are the
/// same shape of error.
///
/// Only the trailing edge needs checking. Leading edges are already constrained by
/// the needles themselves, which start with `.`, `~`, `_`, or a distinctive word.
/// A following `_` counts as a boundary, so `API_KEY_ID` still matches `_KEY`.
///
/// This is still substring matching, and still an upper bound. It is merely a less
/// wrong one.
fn contains_marker(text: &str, needle: &str) -> bool {
    let needs_boundary = needle
        .chars()
        .last()
        .is_some_and(|last| last.is_ascii_alphanumeric());
    if !needs_boundary {
        return text.contains(needle);
    }

    let mut from = 0usize;
    while let Some(offset) = text.get(from..).and_then(|rest| rest.find(needle)) {
        let start = from.saturating_add(offset);
        let end = start.saturating_add(needle.len());
        let next = text.get(end..).and_then(|rest| rest.chars().next());
        if !next.is_some_and(|ch| ch.is_ascii_alphanumeric()) {
            return true;
        }
        // Needles are ASCII, so `start` indexes an ASCII byte and `start + 1` is a
        // valid char boundary.
        from = start.saturating_add(1);
    }
    false
}

/// Which markers appear anywhere, and which appear *only* in unreferenced files.
fn lexical(manifest: &Manifest, files: &[WalkedFile]) -> Lexical {
    let phase: BTreeMap<&str, LoadPhase> = manifest
        .inventory
        .iter()
        .map(|entry| (entry.path.as_str(), entry.load_phase))
        .collect();

    let mut anywhere: BTreeMap<&str, bool> = BTreeMap::new();
    let mut referenced_too: BTreeMap<&str, bool> = BTreeMap::new();

    for file in files {
        let Some(text) = file.text.as_deref() else {
            continue;
        };
        let is_unreferenced = phase.get(file.path.as_str()) == Some(&LoadPhase::Unreferenced);

        for marker in MARKERS {
            if !marker
                .needles
                .iter()
                .any(|needle| contains_marker(text, needle))
            {
                continue;
            }
            anywhere.insert(marker.name, true);
            if !is_unreferenced {
                referenced_too.insert(marker.name, true);
            }
        }
    }

    let hit = |name: &str| anywhere.get(name).copied().unwrap_or(false);
    let only_unreferenced: Vec<String> = MARKERS
        .iter()
        .filter(|marker| {
            anywhere.get(marker.name).copied().unwrap_or(false)
                && !referenced_too.get(marker.name).copied().unwrap_or(false)
        })
        .map(|marker| marker.name.to_owned())
        .collect();

    Lexical {
        credential_paths: hit("credential_paths"),
        secret_env: hit("secret_env"),
        network: hit("network"),
        agent_config_write: hit("agent_config_write"),
        dynamic_eval: hit("dynamic_eval"),
        install_fetch: hit("install_fetch"),
        encoding_chain: hit("encoding_chain"),
        only_in_unreferenced: only_unreferenced,
    }
}

/// The marker column names, in report order.
#[must_use]
pub fn marker_names() -> Vec<&'static str> {
    MARKERS.iter().map(|marker| marker.name).collect()
}

/// Whether a [`Lexical`] set a given marker, by name.
#[must_use]
pub fn lexical_hit(lexical: &Lexical, name: &str) -> bool {
    match name {
        "credential_paths" => lexical.credential_paths,
        "secret_env" => lexical.secret_env,
        "network" => lexical.network,
        "agent_config_write" => lexical.agent_config_write,
        "dynamic_eval" => lexical.dynamic_eval,
        "install_fetch" => lexical.install_fetch,
        "encoding_chain" => lexical.encoding_chain,
        _ => false,
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "a failed unwrap or index in a test is the test failing"
)]
mod tests {
    use super::*;

    #[test]
    fn every_marker_has_an_accessor() {
        // `lexical_hit` matches on names; a marker added to MARKERS without a
        // matching arm would silently read as never hit, and the report would
        // quietly under-count it.
        let set = Lexical {
            credential_paths: true,
            secret_env: true,
            network: true,
            agent_config_write: true,
            dynamic_eval: true,
            install_fetch: true,
            encoding_chain: true,
            only_in_unreferenced: Vec::new(),
        };
        for name in marker_names() {
            assert!(
                lexical_hit(&set, name),
                "marker {name:?} is in MARKERS but lexical_hit has no arm for it"
            );
        }
    }

    #[test]
    fn marker_matching_respects_word_boundaries() {
        // The bug this exists to prevent: `.env` inside `os.environ` marked a
        // referenced file as touching credentials, which emptied the
        // "only in unreferenced" column without changing any total.
        assert!(!contains_marker("profile = os.environ.get('X')", ".env"));
        assert!(contains_marker("read the .env file", ".env"));
        assert!(contains_marker("cat .env", ".env"));
        assert!(contains_marker("open('.env')", ".env"));

        assert!(!contains_marker("SEARCH_KEYWORD = 1", "_KEY"));
        assert!(contains_marker("AWS_SECRET_ACCESS_KEY", "_KEY"));
        assert!(
            contains_marker("API_KEY_ID", "_KEY"),
            "an underscore continues a name and must still count"
        );
        assert!(!contains_marker("_TOKENIZER", "_TOKEN"));
        assert!(contains_marker("GITHUB_TOKEN=x", "_TOKEN"));

        // Needles ending in punctuation need no boundary and must be unaffected.
        assert!(contains_marker("result = eval(expr)", "eval("));
        assert!(contains_marker("path = ~/.aws/credentials", "~/.aws"));

        // Dotted continuations are boundaries, so library prefixes still match.
        assert!(contains_marker("import urllib.request", "urllib"));
        assert!(contains_marker("base64.b64decode(x)", "base64"));
    }

    #[test]
    fn marker_names_are_unique() {
        let mut names = marker_names();
        let before = names.len();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), before);
    }

    #[test]
    fn description_share_is_integer_ppm() {
        let structure = Structure {
            files: 1,
            total_bytes: 4000,
            reference_bytes: 0,
            unreferenced_bytes: 0,
            description_bytes: 100,
            has_unreferenced: false,
            has_scripts: false,
            languages: BTreeMap::new(),
            unresolved: BTreeMap::new(),
        };
        // 100/4000 = 2.5%, i.e. 25_000 ppm. No float anywhere.
        assert_eq!(structure.description_share_ppm(), Some(25_000));
    }

    #[test]
    fn an_empty_bundle_has_no_share_rather_than_a_division_by_zero() {
        let structure = Structure {
            files: 0,
            total_bytes: 0,
            reference_bytes: 0,
            unreferenced_bytes: 0,
            description_bytes: 0,
            has_unreferenced: false,
            has_scripts: false,
            languages: BTreeMap::new(),
            unresolved: BTreeMap::new(),
        };
        assert_eq!(structure.description_share_ppm(), None);
    }
}
