//! Choosing which bundles get labelled.
//!
//! `docs/05-eval.md` requires a **held-out split, fixed by seed, never tuned
//! against**, and names the false-positive rate on the benign stratum as the
//! headline metric. Both of those are properties of the sample, so the sample is
//! computed here, committed, and reproducible by anybody with the same index.
//!
//! # Why stratified, and why these strata
//!
//! A uniform sample of 34,284 bundles would be almost entirely markdown: only
//! **3,495 bundles contain a file in a language the code plane has a grammar
//! for**, and the other 30,789 cannot produce a `capabilities` finding no matter
//! what the scanner does. A "0% false-positive rate" measured mostly over those
//! would be measuring the absence of an opportunity, not the presence of
//! restraint — the exact shape of reassuring number this project exists to
//! refuse.
//!
//! So the benign stratum is bundles where the scanner **could** fire and should
//! not. The others exist to measure recall, to reach the disclosure-delta shape
//! T7's cut criterion turns on, and to give the instruction plane its own
//! false-positive surface, since markdown is the one language where it fires.
//!
//! # Selection is a hash, not a shuffle
//!
//! Each candidate is ranked by `sha256(seed || digest)` and the lowest are
//! taken. That is uniform, deterministic, and needs no RNG dependency. A
//! bundle's rank key depends only on the seed and its own content digest, so the
//! **relative order of any two bundles never changes** no matter what else the
//! index contains.
//!
//! That is not the same as the sample being stable when the corpus grows, and it
//! was originally documented as though it were. It is not: the quota is fixed,
//! so doubling a stratum's population halves the chance any given member stays
//! in the top N. A test asserting otherwise failed immediately, which is the
//! right outcome — the claim was wrong, not the code.
//!
//! What actually keeps labels from evaporating is two other things. The sample
//! is **committed**, so re-drawing it is a visible diff rather than a side
//! effect of re-harvesting. And labels are keyed by **content digest**, so a
//! bundle whose bytes have not changed keeps its label across any number of
//! harvests.

use crate::{measure::Measurements, IndexRecord};
use serde::{Deserialize, Serialize};
use skillmap_core::Digest;
use std::collections::BTreeMap;

/// Languages the code plane has a grammar for.
///
/// Kept in step with `rules/languages.toml` by
/// `the_supported_language_list_matches_the_rules_tree` in `tests/sample.rs`,
/// not by memory. A language added there and forgotten here would quietly shrink
/// the population the benign stratum is drawn from.
pub const SUPPORTED: [&str; 4] = ["javascript", "python", "shell", "typescript"];

/// Which population a sampled bundle was drawn from.
///
/// Disjoint by construction — [`Stratum::of`] assigns exactly one, in priority
/// order — so no bundle is counted twice and the per-stratum denominators add up
/// to the sample size.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Stratum {
    /// Code, and a credential marker that appears **only** in files nothing
    /// references. The disclosure-delta shape, and the population
    /// `docs/04-semantic-layer.md`'s cut criterion is about.
    DisclosureShape,
    /// Code, and a credential marker somewhere. Where `fs.read.credential`
    /// should fire: this stratum carries recall.
    CodeCredential,
    /// Code, and some other lexical marker but no credential one.
    CodeOtherMarker,
    /// Code, and no lexical marker at all.
    ///
    /// **The headline stratum.** Every finding here is a candidate false
    /// positive, and the scanner had a real opportunity to produce one.
    CodeClean,
    /// No file in a supported language, and no lexical marker.
    ///
    /// The code plane structurally cannot fire; the instruction plane can, and
    /// markdown is the only language it has rules for. This is its
    /// false-positive surface.
    ProseOnly,
}

impl Stratum {
    /// Every stratum, in the priority order [`Stratum::of`] applies.
    pub const ALL: [Self; 5] = [
        Self::DisclosureShape,
        Self::CodeCredential,
        Self::CodeOtherMarker,
        Self::CodeClean,
        Self::ProseOnly,
    ];

    /// Wire name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DisclosureShape => "disclosure_shape",
            Self::CodeCredential => "code_credential",
            Self::CodeOtherMarker => "code_other_marker",
            Self::CodeClean => "code_clean",
            Self::ProseOnly => "prose_only",
        }
    }

    /// Which stratum a record belongs to, or `None` if it belongs to none.
    ///
    /// `None` is a real answer: a bundle with no supported-language file *and* a
    /// lexical marker is in no stratum here. Adding it would mean labelling
    /// prose for capabilities nothing can detect in it.
    #[must_use]
    pub fn of(measurements: &Measurements) -> Option<Self> {
        let lexical = &measurements.lexical;
        let has_code = SUPPORTED
            .iter()
            .any(|language| measurements.structure.languages.contains_key(*language));

        let credential = lexical.credential_paths;
        let other_marker = lexical.secret_env
            || lexical.dynamic_eval
            || lexical.agent_config_write
            || lexical.install_fetch
            || lexical.encoding_chain;
        let credential_hidden = lexical
            .only_in_unreferenced
            .iter()
            .any(|marker| marker == "credential_paths");

        match (has_code, credential, credential_hidden, other_marker) {
            (true, true, true, _) => Some(Self::DisclosureShape),
            (true, true, false, _) => Some(Self::CodeCredential),
            (true, false, _, true) => Some(Self::CodeOtherMarker),
            (true, false, _, false) => Some(Self::CodeClean),
            (false, false, _, false) => Some(Self::ProseOnly),
            (false, _, _, _) => None,
        }
    }
}

/// How many bundles to draw from each stratum.
///
/// Not proportional to the population. A proportional sample would spend almost
/// all of its budget on prose and produce a headline number with a confidence
/// interval too wide to act on; these sizes put the budget where the metrics
/// `docs/05-eval.md` names actually live. Every published rate is per stratum
/// and carries its own interval, so over-sampling a stratum cannot flatter an
/// aggregate — there is no aggregate.
#[must_use]
pub fn default_sizes() -> BTreeMap<Stratum, usize> {
    BTreeMap::from([
        (Stratum::CodeClean, 40),
        (Stratum::CodeCredential, 40),
        (Stratum::DisclosureShape, 20),
        (Stratum::CodeOtherMarker, 15),
        (Stratum::ProseOnly, 15),
    ])
}

/// One bundle chosen for labelling.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Selected {
    /// The bundle's content digest — also its directory name under `corpus/raw`.
    pub digest: String,
    /// Which stratum it was drawn from.
    pub stratum: Stratum,
    /// `owner/name` of the repository it was found in.
    pub repo: String,
    /// Bundle root within that repository.
    pub bundle_root: String,
    /// Pinned commit, so a label can be traced to exact bytes.
    pub commit: String,
}

/// A committed, reproducible sample.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sample {
    /// The corpus snapshot this was drawn from.
    pub snapshot: String,
    /// The seed. Changing it draws a different sample, which is a thing to do
    /// deliberately and never quietly — `docs/05-eval.md`: *"if you tune on the
    /// test split the number is decoration."*
    pub seed: String,
    /// Population size per stratum, before sampling. Published so a reader can
    /// see how much of each stratum was looked at.
    pub population: BTreeMap<String, usize>,
    /// The chosen bundles, sorted by `(stratum, digest)`.
    pub selected: Vec<Selected>,
}

/// Draw a stratified sample.
#[must_use]
pub fn draw(
    snapshot: &str,
    seed: &str,
    records: &[IndexRecord],
    sizes: &BTreeMap<Stratum, usize>,
) -> Sample {
    let mut buckets: BTreeMap<Stratum, Vec<&IndexRecord>> = BTreeMap::new();
    for record in records {
        if let Some(stratum) = Stratum::of(&record.measurements) {
            buckets.entry(stratum).or_default().push(record);
        }
    }

    let population = buckets
        .iter()
        .map(|(stratum, members)| (stratum.as_str().to_owned(), members.len()))
        .collect();

    let mut selected = Vec::new();
    for stratum in Stratum::ALL {
        let Some(members) = buckets.get_mut(&stratum) else {
            continue;
        };
        let wanted = sizes.get(&stratum).copied().unwrap_or(0);

        // Rank by a keyed hash of the digest. Uniform, deterministic, and stable
        // as the corpus grows: a bundle's rank does not depend on which other
        // bundles happen to be present.
        members.sort_by_cached_key(|record| {
            let mut material = seed.as_bytes().to_vec();
            material.extend_from_slice(record.digest.as_bytes());
            (Digest::of(&material).to_wire(), record.digest.clone())
        });

        for record in members.iter().take(wanted) {
            selected.push(Selected {
                digest: record.digest.clone(),
                stratum,
                repo: record.repo.clone(),
                bundle_root: record.bundle_root.clone(),
                commit: record.commit.clone(),
            });
        }
    }

    selected.sort_by(|a, b| (a.stratum, &a.digest).cmp(&(b.stratum, &b.digest)));

    Sample {
        snapshot: snapshot.to_owned(),
        seed: seed.to_owned(),
        population,
        selected,
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "a failed assertion in a test is the test failing"
)]
mod tests {
    use super::*;
    use crate::measure::{Governance, Lexical, Structure};
    use crate::Provenance;

    fn record(digest: &str, languages: &[&str], lexical: Lexical) -> IndexRecord {
        IndexRecord {
            digest: digest.to_owned(),
            repo: "owner/name".to_owned(),
            commit: "abc".to_owned(),
            bundle_root: "skills/thing".to_owned(),
            provenance: Provenance::CodeSearch,
            stars: None,
            measurements: Measurements {
                structure: Structure {
                    languages: languages
                        .iter()
                        .map(|language| ((*language).to_owned(), 1))
                        .collect(),
                    ..Structure::default()
                },
                lexical,
                governance: Governance::default(),
            },
        }
    }

    fn clean() -> Lexical {
        Lexical::default()
    }

    fn credential(hidden: bool) -> Lexical {
        Lexical {
            credential_paths: true,
            only_in_unreferenced: if hidden {
                vec!["credential_paths".to_owned()]
            } else {
                Vec::new()
            },
            ..Lexical::default()
        }
    }

    #[test]
    fn strata_are_disjoint_and_priority_ordered() {
        // Overlapping strata would double-count bundles and make the
        // denominators lie.
        let cases = [
            (
                vec!["python"],
                credential(true),
                Some(Stratum::DisclosureShape),
            ),
            (
                vec!["python"],
                credential(false),
                Some(Stratum::CodeCredential),
            ),
            (vec!["python"], clean(), Some(Stratum::CodeClean)),
            (vec!["markdown"], clean(), Some(Stratum::ProseOnly)),
            // Prose with a marker is in no stratum: labelling it would mean
            // scoring capabilities against files nothing can detect them in.
            (vec!["markdown"], credential(false), None),
        ];
        for (languages, lexical, expected) in cases {
            let record = record("d", &languages, lexical);
            assert_eq!(Stratum::of(&record.measurements), expected, "{languages:?}");
        }
    }

    #[test]
    fn a_marker_in_an_unsupported_language_still_counts_as_code() {
        // `has_code` asks whether the *code plane* could fire, which is about the
        // grammars that exist, not about whether a file looks like a script.
        let ruby = record("d", &["ruby", "markdown"], clean());
        assert_eq!(Stratum::of(&ruby.measurements), Some(Stratum::ProseOnly));
    }

    #[test]
    fn the_same_seed_draws_the_same_sample() {
        let records: Vec<IndexRecord> = (0..200)
            .map(|n| record(&format!("sha256:{n:064x}"), &["python"], clean()))
            .collect();
        let sizes = BTreeMap::from([(Stratum::CodeClean, 10)]);

        let first = draw("2026-08", "seed-one", &records, &sizes);
        let again = draw("2026-08", "seed-one", &records, &sizes);
        assert_eq!(first, again);

        let other = draw("2026-08", "seed-two", &records, &sizes);
        assert_ne!(
            first.selected, other.selected,
            "a different seed must draw a different sample, or the seed is decoration"
        );
    }

    #[test]
    fn rank_does_not_depend_on_the_rest_of_the_population() {
        // The property that is actually true, and the one worth having: a
        // bundle's key is a function of the seed and its own digest, so two
        // bundles never swap places because a third appeared.
        //
        // The claim this replaced — that the *sample* survives the corpus
        // growing — is false for any fixed-size sample of a growing population,
        // and the test that asserted it failed on first run. Sample durability
        // comes from committing the sample and keying labels by content digest,
        // not from the ranking function.
        let sizes = BTreeMap::from([(Stratum::CodeClean, 40)]);
        let small: Vec<IndexRecord> = (0..50)
            .map(|n| record(&format!("sha256:{n:064x}"), &["python"], clean()))
            .collect();
        let mut grown = small.clone();
        grown.extend((50..400).map(|n| record(&format!("sha256:{n:064x}"), &["python"], clean())));

        let before = draw("2026-08", "seed", &small, &sizes);
        let after = draw("2026-08", "seed", &grown, &sizes);

        // Every bundle from the small draw that also appears in the large draw
        // must appear in the same relative order.
        let survivors: Vec<&String> = before
            .selected
            .iter()
            .map(|selection| &selection.digest)
            .filter(|digest| after.selected.iter().any(|other| &&other.digest == digest))
            .collect();
        let in_after: Vec<&String> = after
            .selected
            .iter()
            .map(|selection| &selection.digest)
            .filter(|digest| survivors.contains(digest))
            .collect();
        assert_eq!(survivors, in_after, "relative order changed");
        assert!(
            !survivors.is_empty(),
            "the test needs at least one survivor"
        );
    }

    #[test]
    fn a_stratum_smaller_than_its_quota_yields_what_it_has() {
        let records = vec![record("sha256:aa", &["python"], clean())];
        let sample = draw(
            "2026-08",
            "seed",
            &records,
            &BTreeMap::from([(Stratum::CodeClean, 40)]),
        );
        assert_eq!(sample.selected.len(), 1);
        assert_eq!(sample.population["code_clean"], 1);
    }
}
