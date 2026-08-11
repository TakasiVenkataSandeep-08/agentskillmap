#![warn(missing_docs)]

//! The corpus harvester — build step 1, and the project's kill gate.
//!
//! This crate collects real `SKILL.md` bundles, measures them mechanically, and
//! writes a report. `docs/01-corpus-scan.md` is the specification; the short
//! version is that it answers whether the problem this project exists to solve
//! is real, **before** the scanner is finished:
//!
//! > If the base rates come back boring — 2% ship scripts, nobody touches
//! > credentials — the risk is theoretical and you have killed a bad idea for the
//! > cost of a weekend. Publish either way.
//!
//! # This is the only crate that touches the network
//!
//! Invariant 9 says no network *at scan time*, and names this subcommand as one
//! of two sanctioned exceptions. That is why the HTTP dependency lives here and
//! nowhere else, why it is used only for authenticated GETs against the GitHub
//! REST API, and why bundle contents come down through `git clone` rather than
//! through an in-process transfer.
//!
//! # Measurements are research statistics, not findings
//!
//! Nothing this crate computes is a manifest finding. The lexical counts in
//! [`measure`] match strings; they do not parse, do not establish reachability,
//! and carry no provenance. They exist to size the ecosystem and to tell T4 which
//! languages are worth writing grammars for. Presenting them as tier-`proven`
//! capabilities would blend an assurance tier (invariant 5) and overstate what
//! was actually established (invariant 4), so the report labels every one of them
//! as lexical, and none of them ever reaches a `Manifest`.

pub mod archive;
pub mod github;
pub mod measure;
pub mod report;

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// How a bundle came to be in the corpus.
///
/// Recorded per bundle because it decides which population a number describes.
/// `docs/01-corpus-scan.md` is blunt about this: *"A corpus drawn only from 'top
/// 50 skills' listicles measures the curated head, not the ecosystem. Report head
/// and tail separately or the base rates are meaningless."*
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Provenance {
    /// `anthropics/skills` — the baseline for what "good" looks like.
    Baseline,
    /// A curated "awesome" list or marketplace listing.
    CuratedList,
    /// GitHub code search for `path:**/SKILL.md`.
    CodeSearch,
    /// Named explicitly by the operator.
    Explicit,
}

impl Provenance {
    /// Every variant, in a fixed order.
    pub const ALL: &'static [Self] = &[
        Self::Baseline,
        Self::CuratedList,
        Self::CodeSearch,
        Self::Explicit,
    ];

    /// The wire form. Sort keys are defined over this rather than over variant
    /// order, so reordering the enum cannot silently reorder a report.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Baseline => "baseline",
            Self::CuratedList => "curated_list",
            Self::CodeSearch => "code_search",
            Self::Explicit => "explicit",
        }
    }

    /// Whether this provenance samples the **curated head** of the ecosystem.
    ///
    /// Baseline, curated lists and operator-named repositories are all selected
    /// by a human who already thought the skill was worth mentioning. Only code
    /// search reaches the tail, and conflating the two is the single easiest way
    /// to publish a number that means nothing.
    #[must_use]
    pub const fn is_head(&self) -> bool {
        match self {
            Self::Baseline | Self::CuratedList | Self::Explicit => true,
            Self::CodeSearch => false,
        }
    }
}

/// A repository to harvest, at a pinned commit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepoRef {
    /// Repository owner or organisation.
    pub owner: String,
    /// Repository name.
    pub name: String,
    /// The exact commit harvested.
    ///
    /// Pinned, not a branch: the corpus has to be reproducible later, and
    /// "whatever main pointed at that week" is not. It doubles as the fetch
    /// cache key, which is what makes a re-run skip the network entirely.
    pub commit: String,
    /// How this repository was discovered.
    pub provenance: Provenance,
    /// Star count **read from the API**, never from secondary sources.
    ///
    /// `docs/01-corpus-scan.md`: star counts across blog coverage of this
    /// ecosystem are wildly inconsistent for the same repositories. Do not
    /// publish a number that came from a listicle.
    pub stars: Option<u64>,
}

impl RepoRef {
    /// `owner/name`.
    #[must_use]
    pub fn slug(&self) -> String {
        format!("{}/{}", self.owner, self.name)
    }

    /// A stable, filesystem-safe cache key including the commit.
    #[must_use]
    pub fn cache_key(&self) -> String {
        format!("{}__{}__{}", self.owner, self.name, self.commit)
    }
}

/// Materializes a repository into a local directory.
///
/// A trait so the pipeline can be tested end to end without a network: the tests
/// supply a fetcher backed by the local fixture corpus. Nothing about the
/// measurement, archiving, indexing or reporting path differs between the two.
pub trait Fetcher {
    /// Place `repo` at the pinned commit into `into`, which does not yet exist.
    ///
    /// # Errors
    ///
    /// [`Error`] if the repository cannot be retrieved.
    fn fetch(&self, repo: &RepoRef, into: &Path) -> Result<(), Error>;
}

/// Discovers repositories to harvest.
pub trait Source {
    /// Yield at most `limit` repositories.
    ///
    /// # Errors
    ///
    /// [`Error`] if discovery fails. Returning zero repositories is not an error.
    fn repos(&self, limit: usize) -> Result<Vec<RepoRef>, Error>;
}

/// Everything that can go wrong harvesting.
#[derive(Debug)]
pub enum Error {
    /// `GITHUB_TOKEN` is absent.
    MissingToken,
    /// A network or API call failed.
    Api {
        /// What was being requested.
        context: String,
        /// The underlying message.
        message: String,
    },
    /// `git` could not be run, or exited non-zero.
    Git {
        /// What was being cloned.
        context: String,
        /// The underlying message.
        message: String,
    },
    /// A filesystem operation failed.
    Io {
        /// The path involved.
        path: PathBuf,
        /// Why.
        source: std::io::Error,
    },
    /// A bundle could not be parsed at all.
    Parse {
        /// Which bundle.
        context: String,
        /// Why.
        message: String,
    },
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingToken => write!(
                f,
                "GITHUB_TOKEN is not set.\n\n\
                 The corpus harvester needs an authenticated GitHub token: \
                 unauthenticated search is capped at 60 requests/hour, which is \
                 not enough to enumerate this ecosystem, and a half-finished \
                 harvest produces base rates that are worse than none.\n\n\
                 Create a token with no scopes at all — public code search and \
                 public repository metadata need no permissions — and export it \
                 as GITHUB_TOKEN."
            ),
            Self::Api { context, message } => write!(f, "GitHub API ({context}): {message}"),
            Self::Git { context, message } => write!(f, "git ({context}): {message}"),
            Self::Io { path, source } => write!(f, "{}: {source}", path.display()),
            Self::Parse { context, message } => write!(f, "parsing {context}: {message}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

/// Where corpus output goes and how much of it to gather.
#[derive(Debug, Clone)]
pub struct HarvestOptions {
    /// Root for `raw/`, `index.json`, and `report.md`.
    pub corpus_dir: PathBuf,
    /// Maximum repositories to pull from each source.
    pub limit: usize,
    /// A label for this corpus snapshot, e.g. `2026-08`.
    ///
    /// Deliberately operator-supplied rather than a wall-clock timestamp: the
    /// report and index must be reproducible from the same inputs, and a clock
    /// reading would make every re-run diff against itself. The reproducible
    /// clock is the pinned commit recorded per repository.
    pub snapshot: String,
}

impl Default for HarvestOptions {
    fn default() -> Self {
        Self {
            corpus_dir: PathBuf::from("corpus"),
            limit: 200,
            snapshot: "unlabelled".to_owned(),
        }
    }
}

/// One harvested bundle, as it appears in `corpus/index.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexRecord {
    /// The bundle's `content_digest`. Records are deduplicated on this: the same
    /// bundle vendored into three repositories is one row, not three.
    pub digest: String,
    /// `owner/name` of the repository it was found in.
    pub repo: String,
    /// The pinned commit.
    pub commit: String,
    /// Bundle root within the repository, forward-slashed.
    pub bundle_root: String,
    /// How it was discovered.
    pub provenance: Provenance,
    /// Stars on the containing repository, from the API.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stars: Option<u64>,
    /// What was measured about it.
    pub measurements: measure::Measurements,
}

impl IndexRecord {
    /// Total order for the index: digest, then repository, then bundle root.
    ///
    /// Keyed on strings only, so the file is byte-stable across machines and
    /// across edits to any enum in this crate.
    #[must_use]
    pub fn sort_key(&self) -> (&str, &str, &str) {
        (&self.digest, &self.repo, &self.bundle_root)
    }
}

/// The full result of a harvest.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Index {
    /// The snapshot label this corpus was gathered under.
    pub snapshot: String,
    /// One record per distinct bundle, sorted.
    pub records: Vec<IndexRecord>,
    /// Repositories that were reached but yielded no bundle, and why.
    ///
    /// Present so "we found nothing there" and "we never looked" stay
    /// distinguishable — the same reasoning as `unresolved` in a manifest.
    pub skipped: Vec<Skipped>,
}

/// A repository that produced no usable bundle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Skipped {
    /// `owner/name`.
    pub repo: String,
    /// Why nothing came of it.
    pub reason: String,
}

/// Read `GITHUB_TOKEN`, failing fast and legibly when it is absent.
///
/// # Errors
///
/// [`Error::MissingToken`] if the variable is unset or empty.
pub fn github_token() -> Result<String, Error> {
    match std::env::var("GITHUB_TOKEN") {
        Ok(token) if !token.trim().is_empty() => Ok(token),
        _ => Err(Error::MissingToken),
    }
}
