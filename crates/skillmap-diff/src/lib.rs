#![warn(missing_docs)]

//! `skillmap.lock`, and capability escalation between a lock and a fresh scan.
//!
//! This crate produces the line the whole project exists for:
//!
//! ```text
//! ✗ example-skill  capability escalation vs skillmap.lock
//!     + fs.read.credential   scripts/collect.py:17   py.credential-read.dotfile
//!       reads ~/.aws/credentials — added in this update
//! ```
//!
//! `AGENTS.md`: *"this skill gained credential access in the update you're about
//! to merge"* — everything upstream exists to make that sentence trustworthy.
//!
//! # Why the lock is not the manifest
//!
//! A lock is read by humans in a pull request. A manifest carries every byte
//! span, every snippet hash, every unresolved entry — hundreds of lines per
//! bundle that would bury the four words a reviewer needs. So the lock holds the
//! capability *set* and the content digest, and nothing else; the manifest is
//! regenerated on demand when somebody wants the evidence.
//!
//! That split is also why the lock is safe to commit and the manifest is not
//! especially useful to: a lock diff is short enough to actually read.

use serde::{Deserialize, Serialize};
use skillmap_core::Manifest;
use std::collections::{BTreeMap, BTreeSet};

/// One bundle's entry in `skillmap.lock`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LockEntry {
    /// Which resolver discovered it, e.g. `claude-code`.
    pub resolver: String,
    /// Merkle digest over the bundle's bytes. Changes whenever content does.
    pub content_digest: String,
    /// Capability terms, sorted and deduplicated.
    ///
    /// Wire names rather than the enum, because a lock outlives the binary that
    /// wrote it: a term this build does not recognise must round-trip rather than
    /// fail to parse, or upgrading skillmap would silently rewrite locks.
    pub capabilities: Vec<String>,
    /// The manifest schema version this entry was written against.
    pub schema_version: String,
}

/// A project's lockfile.
///
/// Keyed by `target.root` — the bundle's path relative to its resolver's
/// discovery root, which is the one identifier that is stable across machines.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Lock {
    /// Bundles, keyed by root.
    pub bundles: BTreeMap<String, LockEntry>,
}

impl Lock {
    /// Build a lock from freshly scanned manifests.
    #[must_use]
    pub fn from_manifests(manifests: &[Manifest]) -> Self {
        let bundles = manifests
            .iter()
            .map(|manifest| {
                let mut capabilities: Vec<String> = manifest
                    .capabilities
                    .iter()
                    .map(|capability| capability.capability.as_str().to_owned())
                    .collect();
                capabilities.sort();
                capabilities.dedup();

                (
                    manifest.target.root.clone(),
                    LockEntry {
                        resolver: manifest.target.resolver.clone(),
                        content_digest: manifest.target.content_digest.to_wire(),
                        capabilities,
                        schema_version: manifest.schema_version.clone(),
                    },
                )
            })
            .collect();
        Self { bundles }
    }

    /// Render as canonical JSON: sorted keys, two-space indent, LF, trailing
    /// newline — the same framing the manifest uses.
    ///
    /// A lockfile is diffed on every change, so its serialization has to be as
    /// stable as the manifest's. Reusing the framing means a reviewer reading a
    /// lock diff sees only what actually changed.
    ///
    /// # Errors
    ///
    /// Returns the serializer's error, which cannot occur for this type.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        let value = serde_json::to_value(self)?;
        Ok(serde_json::to_string_pretty(&value)? + "\n")
    }

    /// Parse a lockfile.
    ///
    /// # Errors
    ///
    /// Returns the parser's error if the text is not a lock this build can read.
    pub fn from_json(text: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(text)
    }
}

/// One difference between a lock and a fresh scan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Change {
    /// A capability the lock did not record. **This is the escalation case.**
    CapabilityAdded {
        /// Bundle root.
        root: String,
        /// The term gained.
        capability: String,
    },
    /// A capability the lock recorded that the scan no longer finds.
    CapabilityRemoved {
        /// Bundle root.
        root: String,
        /// The term lost.
        capability: String,
    },
    /// Content changed without any capability changing.
    ///
    /// Not an escalation, and deliberately still reported: it is the difference
    /// between "this update did nothing interesting" and "this update changed
    /// code and happened not to trip a rule", and only one of those is worth a
    /// reviewer's attention.
    ContentChanged {
        /// Bundle root.
        root: String,
        /// Digest recorded in the lock.
        before: String,
        /// Digest the scan computed.
        after: String,
        /// Whether the fresh scan could read any code in this bundle.
        ///
        /// `Some(false)` is the case this field exists for: the bytes moved and
        /// **nothing analysed them**, so the capability diff above is silent for
        /// a reason that has nothing to do with the bundle being unchanged. 89.8%
        /// of published skills ship no file this build has a grammar for, and
        /// over 390 random corpus bundles 90.3% land here.
        ///
        /// `None` when the comparison had no scan on either side — [`compare`]
        /// of two committed locks cannot know, and inventing an answer there
        /// would manufacture a failure nothing observed.
        analysed: Option<bool>,
    },
    /// A bundle present now and absent from the lock.
    BundleAdded {
        /// Bundle root.
        root: String,
        /// Everything it can do.
        capabilities: Vec<String>,
    },
    /// A bundle in the lock that the scan did not find.
    BundleRemoved {
        /// Bundle root.
        root: String,
    },
}

impl Change {
    /// The bundle this change concerns.
    #[must_use]
    pub fn root(&self) -> &str {
        match self {
            Self::CapabilityAdded { root, .. }
            | Self::CapabilityRemoved { root, .. }
            | Self::ContentChanged { root, .. }
            | Self::BundleAdded { root, .. }
            | Self::BundleRemoved { root } => root,
        }
    }

    /// Whether this change grants the bundle something it could not do before.
    ///
    /// A new bundle counts as an escalation only if it arrives with capabilities:
    /// installing a skill that can do nothing is not a privilege change, and
    /// failing CI for it would train people to ignore the check.
    #[must_use]
    pub fn is_escalation(&self) -> bool {
        match self {
            Self::CapabilityAdded { .. } => true,
            Self::BundleAdded { capabilities, .. } => !capabilities.is_empty(),
            Self::CapabilityRemoved { .. }
            | Self::ContentChanged { .. }
            | Self::BundleRemoved { .. } => false,
        }
    }
}

/// Everything that changed between a lock and a scan.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Delta {
    /// Changes, in a stable order.
    pub changes: Vec<Change>,
}

impl Delta {
    /// Whether anything changed at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.changes.is_empty()
    }

    /// Content changes in bundles whose code nothing could read.
    ///
    /// Separate from [`Delta::escalations`] and never folded into it, because
    /// gating on these by default would fail CI on every routine prose edit and
    /// train people to ignore the check — the failure mode `hook.rs` argues at
    /// length. It is opt-in through `policy.toml`, and it is the only thing this
    /// differ can offer for the nine skills in ten whose code it cannot read.
    #[must_use]
    pub fn unanalysed_content_changes(&self) -> Vec<&Change> {
        self.changes
            .iter()
            .filter(|change| {
                matches!(
                    change,
                    Change::ContentChanged {
                        analysed: Some(false),
                        ..
                    }
                )
            })
            .collect()
    }

    /// Only the changes that grant new capability.
    #[must_use]
    pub fn escalations(&self) -> Vec<&Change> {
        self.changes
            .iter()
            .filter(|change| change.is_escalation())
            .collect()
    }
}

/// Compare a lock against freshly scanned manifests.
///
/// Ordering is by `(root, kind, capability)` over strings, so the same inputs
/// always produce the same report — a diff tool whose output reordered between
/// runs would make every CI log a false alarm.
#[must_use]
pub fn diff(lock: &Lock, manifests: &[Manifest], code_languages: &BTreeSet<String>) -> Delta {
    let mut delta = compare(lock, &Lock::from_manifests(manifests));
    // `compare` works on two locks and cannot know what was readable, so the
    // answer is filled in here, where a scan exists. Anything the scan did not
    // produce a manifest for keeps `None` rather than guessing.
    let readable: BTreeMap<&str, bool> = manifests
        .iter()
        .map(|manifest| {
            (
                manifest.target.root.as_str(),
                manifest.code_files_read(code_languages) > 0,
            )
        })
        .collect();
    for change in &mut delta.changes {
        if let Change::ContentChanged { root, analysed, .. } = change {
            *analysed = readable.get(root.as_str()).copied();
        }
    }
    delta
}

/// Compare two locks directly.
///
/// [`diff`] is this with a scan on one side. Exposed separately because the
/// comparison is the whole of the logic and a caller with two committed locks —
/// a reviewer asking what a pull request changed, without re-running the
/// scanner — should not have to fabricate manifests to ask.
#[must_use]
pub fn compare(before: &Lock, after: &Lock) -> Delta {
    let mut changes = Vec::new();

    let roots: BTreeSet<&str> = before
        .bundles
        .keys()
        .chain(after.bundles.keys())
        .map(String::as_str)
        .collect();

    for root in roots {
        match (before.bundles.get(root), after.bundles.get(root)) {
            (None, Some(now)) => changes.push(Change::BundleAdded {
                root: root.to_owned(),
                capabilities: now.capabilities.clone(),
            }),
            (Some(_), None) => changes.push(Change::BundleRemoved {
                root: root.to_owned(),
            }),
            (Some(before), Some(now)) => {
                let was: BTreeSet<&str> = before.capabilities.iter().map(String::as_str).collect();
                let is: BTreeSet<&str> = now.capabilities.iter().map(String::as_str).collect();

                for capability in is.difference(&was) {
                    changes.push(Change::CapabilityAdded {
                        root: root.to_owned(),
                        capability: (*capability).to_owned(),
                    });
                }
                for capability in was.difference(&is) {
                    changes.push(Change::CapabilityRemoved {
                        root: root.to_owned(),
                        capability: (*capability).to_owned(),
                    });
                }
                if before.content_digest != now.content_digest {
                    changes.push(Change::ContentChanged {
                        root: root.to_owned(),
                        before: before.content_digest.clone(),
                        after: now.content_digest.clone(),
                        analysed: None,
                    });
                }
            }
            (None, None) => {}
        }
    }

    changes.sort_by_key(|change| (change.root().to_owned(), sort_rank(change), detail(change)));
    Delta { changes }
}

/// Escalations first within a bundle, because that is what a reviewer is looking
/// for and burying it under a digest change wastes the ten seconds this report
/// is supposed to take.
fn sort_rank(change: &Change) -> u8 {
    match change {
        Change::BundleAdded { .. } => 0,
        Change::CapabilityAdded { .. } => 1,
        Change::CapabilityRemoved { .. } => 2,
        Change::BundleRemoved { .. } => 3,
        Change::ContentChanged { .. } => 4,
    }
}

/// A stable tiebreak within one rank.
fn detail(change: &Change) -> String {
    match change {
        Change::CapabilityAdded { capability, .. }
        | Change::CapabilityRemoved { capability, .. } => capability.clone(),
        Change::BundleAdded { capabilities, .. } => capabilities.join(","),
        Change::ContentChanged { after, .. } => after.clone(),
        Change::BundleRemoved { .. } => String::new(),
    }
}

/// Render a delta as the report a reviewer reads in a failing check.
///
/// The format is the one `README.md` promises, and the constraint on it is
/// `docs/00-tasks.md`'s "done when": a reviewer must be able to act on this in
/// **under ten seconds**. That is why each escalation carries the file, the line,
/// and the rule that fired — enough to open the right file and judge it — and why
/// nothing else is included.
///
/// Evidence comes from the manifests rather than the lock, because the lock
/// deliberately does not carry any. A capability with no matching manifest still
/// prints; losing the line entirely would be worse than losing its citation.
#[must_use]
pub fn render(delta: &Delta, manifests: &[Manifest]) -> String {
    use std::fmt::Write as _;

    let by_root: BTreeMap<&str, &Manifest> = manifests
        .iter()
        .map(|manifest| (manifest.target.root.as_str(), manifest))
        .collect();

    let mut out = String::new();
    let mut current = "";

    for change in &delta.changes {
        if change.root() != current {
            current = change.root();
            let marker = if delta
                .changes
                .iter()
                .any(|other| other.root() == current && other.is_escalation())
            {
                "✗"
            } else {
                "·"
            };
            let _ = writeln!(
                out,
                "{marker} {current}  {}",
                if marker == "✗" {
                    "capability escalation vs skillmap.lock"
                } else {
                    "changed, no new capability"
                }
            );
        }

        match change {
            Change::CapabilityAdded { root, capability } => {
                let _ = writeln!(
                    out,
                    "    + {capability}{}",
                    citation(&by_root, root, capability)
                );
                if let Some(detail) = describe(&by_root, root, capability) {
                    let _ = writeln!(out, "      {detail} — added in this update");
                }
            }
            Change::BundleAdded {
                root, capabilities, ..
            } => {
                for capability in capabilities {
                    let _ = writeln!(
                        out,
                        "    + {capability}{}",
                        citation(&by_root, root, capability)
                    );
                }
                if capabilities.is_empty() {
                    let _ = writeln!(out, "    + new bundle, no capabilities detected");
                }
            }
            Change::CapabilityRemoved { capability, .. } => {
                let _ = writeln!(out, "    - {capability}");
            }
            Change::BundleRemoved { .. } => {
                let _ = writeln!(out, "    - bundle no longer present");
            }
            Change::ContentChanged { before, after, .. } => {
                let _ = writeln!(
                    out,
                    "    ~ content changed  {} → {}",
                    short(before),
                    short(after)
                );
            }
        }
    }

    out
}

/// `   file:line   rule_id` for a capability, if the manifest has evidence.
fn citation(by_root: &BTreeMap<&str, &Manifest>, root: &str, capability: &str) -> String {
    by_root
        .get(root)
        .and_then(|manifest| {
            manifest
                .capabilities
                .iter()
                .find(|entry| entry.capability.as_str() == capability)
        })
        .and_then(|entry| entry.evidence.first())
        .map(|evidence| {
            format!(
                "   {}:{}   {}",
                evidence.file, evidence.start_line, evidence.rule_id
            )
        })
        .unwrap_or_default()
}

/// A short human line naming what the capability touches, when the manifest says.
fn describe(by_root: &BTreeMap<&str, &Manifest>, root: &str, capability: &str) -> Option<String> {
    let entry = by_root
        .get(root)?
        .capabilities
        .iter()
        .find(|entry| entry.capability.as_str() == capability)?;
    let detail = entry.detail.as_ref()?;
    let paths = detail.paths.as_ref().filter(|paths| !paths.is_empty());
    let hosts = detail.hosts.as_ref().filter(|hosts| !hosts.is_empty());

    match (paths, hosts) {
        (Some(paths), _) => Some(format!("reads {}", paths.join(", "))),
        (None, Some(hosts)) => Some(format!("contacts {}", hosts.join(", "))),
        (None, None) => None,
    }
}

/// The first eight hex characters of a digest, for a line a human reads.
fn short(digest: &str) -> String {
    digest
        .strip_prefix("sha256:")
        .unwrap_or(digest)
        .chars()
        .take(8)
        .collect()
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    reason = "a failed unwrap in a test is the test failing"
)]
mod tests {
    use super::*;

    fn entry(digest: &str, capabilities: &[&str]) -> LockEntry {
        LockEntry {
            resolver: "claude-code".to_owned(),
            content_digest: format!("sha256:{digest}"),
            capabilities: capabilities.iter().map(|term| (*term).to_owned()).collect(),
            schema_version: "1.0.0".to_owned(),
        }
    }

    fn lock(entries: &[(&str, LockEntry)]) -> Lock {
        Lock {
            bundles: entries
                .iter()
                .map(|(root, entry)| ((*root).to_owned(), entry.clone()))
                .collect(),
        }
    }

    #[test]
    fn a_gained_capability_is_an_escalation() {
        let before = lock(&[("skill", entry("aaaa", &[]))]);
        let after = lock(&[("skill", entry("bbbb", &["fs.read.credential"]))]);

        let delta = compare(&before, &after);
        assert_eq!(
            delta.escalations(),
            vec![&Change::CapabilityAdded {
                root: "skill".to_owned(),
                capability: "fs.read.credential".to_owned(),
            }]
        );
    }

    #[test]
    fn losing_a_capability_is_reported_but_is_not_an_escalation() {
        // Reported, because "this update removed the credential read" is worth
        // seeing. Not an escalation, because failing CI on a skill becoming less
        // capable would be absurd, and absurd checks get disabled.
        let before = lock(&[("skill", entry("aaaa", &["net.egress"]))]);
        let after = lock(&[("skill", entry("bbbb", &[]))]);

        let delta = compare(&before, &after);
        assert!(delta.escalations().is_empty());
        assert!(delta
            .changes
            .iter()
            .any(|change| matches!(change, Change::CapabilityRemoved { .. })));
    }

    #[test]
    fn content_can_change_without_any_capability_changing() {
        let before = lock(&[("skill", entry("aaaa", &["net.egress"]))]);
        let after = lock(&[("skill", entry("bbbb", &["net.egress"]))]);

        let delta = compare(&before, &after);
        assert!(!delta.is_empty(), "an edited skill is not a non-event");
        assert!(delta.escalations().is_empty());
    }

    #[test]
    fn an_unchanged_lock_produces_nothing() {
        let same = lock(&[("skill", entry("aaaa", &["net.egress"]))]);
        assert!(compare(&same, &same).is_empty());
    }

    #[test]
    fn a_new_bundle_escalates_only_if_it_arrives_able_to_do_something() {
        let empty = Lock::default();

        let inert = compare(&empty, &lock(&[("docs", entry("aaaa", &[]))]));
        assert!(
            inert.escalations().is_empty(),
            "installing a skill that can do nothing is not a privilege change"
        );
        assert!(!inert.is_empty(), "it is still worth reporting");

        let armed = compare(&empty, &lock(&[("tool", entry("aaaa", &["process.exec"]))]));
        assert_eq!(armed.escalations().len(), 1);
    }

    #[test]
    fn a_removed_bundle_is_reported_and_does_not_escalate() {
        let delta = compare(
            &lock(&[("skill", entry("aaaa", &["process.exec"]))]),
            &Lock::default(),
        );
        assert_eq!(
            delta.changes,
            vec![Change::BundleRemoved {
                root: "skill".to_owned()
            }]
        );
        assert!(delta.escalations().is_empty());
    }

    #[test]
    fn escalations_sort_ahead_of_noise_within_a_bundle() {
        // The ten-second budget is spent on the first line under a bundle's
        // header. A digest change printed above the credential read would spend
        // it on nothing.
        let before = lock(&[("skill", entry("aaaa", &["net.egress"]))]);
        let after = lock(&[("skill", entry("bbbb", &["fs.read.credential"]))]);

        let delta = compare(&before, &after);
        assert!(
            matches!(delta.changes.first(), Some(Change::CapabilityAdded { .. })),
            "{:?}",
            delta.changes
        );
        assert!(matches!(
            delta.changes.last(),
            Some(Change::ContentChanged { .. })
        ));
    }

    #[test]
    fn ordering_does_not_depend_on_map_iteration() {
        // Two locks built by inserting the same bundles in opposite orders must
        // produce the same report, or every CI log becomes a false alarm
        // (invariant 2).
        let forward = lock(&[
            ("a", entry("1111", &[])),
            ("b", entry("2222", &[])),
            ("c", entry("3333", &[])),
        ]);
        let mut backward = Lock::default();
        for (root, digest) in [("c", "3333"), ("b", "2222"), ("a", "1111")] {
            backward.bundles.insert(root.to_owned(), entry(digest, &[]));
        }
        let after = lock(&[
            ("a", entry("9999", &["process.exec"])),
            ("b", entry("2222", &[])),
            ("c", entry("8888", &["net.egress"])),
        ]);

        assert_eq!(compare(&forward, &after), compare(&backward, &after));
    }

    #[test]
    fn the_lock_round_trips_through_json() {
        let original = lock(&[("skill", entry("aaaa", &["net.egress", "process.exec"]))]);
        let json = original.to_json().unwrap();

        assert!(json.ends_with('\n'), "trailing newline, like the manifest");
        assert_eq!(Lock::from_json(&json).unwrap(), original);
    }

    #[test]
    fn an_unrecognised_capability_term_survives_a_round_trip() {
        // A lock outlives the binary that wrote it. If an older skillmap dropped
        // terms it did not know, running it once would silently rewrite the lock
        // and the next run of a newer build would report the losses as fresh
        // escalations.
        let future = lock(&[("skill", entry("aaaa", &["gpu.claim.exclusive"]))]);
        let round_tripped = Lock::from_json(&future.to_json().unwrap()).unwrap();
        assert_eq!(round_tripped, future);
        assert!(compare(&future, &round_tripped).is_empty());
    }

    #[test]
    fn a_report_prints_even_when_no_manifest_backs_it() {
        // Evidence lives in the manifests, not the lock. When comparing two
        // committed locks there are none — losing the finding entirely would be
        // far worse than losing its citation.
        let delta = compare(
            &Lock::default(),
            &lock(&[("skill", entry("aaaa", &["fs.read.credential"]))]),
        );
        let report = render(&delta, &[]);

        assert!(report.contains("✗ skill"), "{report}");
        assert!(report.contains("+ fs.read.credential"), "{report}");
    }

    #[test]
    fn a_bundle_with_no_escalation_gets_a_quieter_marker() {
        let before = lock(&[("skill", entry("aaaa", &["net.egress"]))]);
        let after = lock(&[("skill", entry("bbbb", &["net.egress"]))]);

        let report = render(&compare(&before, &after), &[]);
        assert!(!report.contains('✗'), "{report}");
        assert!(report.contains("changed, no new capability"), "{report}");
    }
}
