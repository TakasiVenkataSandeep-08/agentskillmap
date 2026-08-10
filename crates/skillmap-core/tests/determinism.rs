//! Invariant 2, as an executable claim.
//!
//! T1's acceptance criterion: *"a hand-built manifest with shuffled input
//! ordering serializes byte-identically across 1,000 randomized field-insertion
//! orders."* That is what [`shuffled_input_order_is_byte_identical`] checks.

#![allow(
    clippy::unwrap_used,
    reason = "a failed unwrap in a test is the test failing"
)]

mod common;

use skillmap_core::{Advisory, Manifest};

/// xorshift64*, seeded fixed.
///
/// A local PRNG rather than a `rand` dependency: this is the entire use of
/// randomness in the crate, and `SECURITY.md` promises a dependency tree this
/// project can defend line by line. A fixed seed also means a failure reproduces
/// exactly instead of once in a thousand CI runs.
struct Rng(u64);

impl Rng {
    const fn new(seed: u64) -> Self {
        Self(seed)
    }

    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }

    /// Fisher-Yates, so every permutation is reachable.
    fn shuffle<T>(&mut self, items: &mut [T]) {
        let len = items.len();
        for i in (1..len).rev() {
            let j = usize::try_from(self.next_u64() % (i as u64 + 1)).unwrap_or(0);
            items.swap(i, j);
        }
    }
}

/// Shuffle every array whose order the schema declares.
fn shuffle_all(manifest: &mut Manifest, rng: &mut Rng) {
    rng.shuffle(&mut manifest.inventory);
    rng.shuffle(&mut manifest.disclosure.declared_capabilities);
    rng.shuffle(&mut manifest.disclosure.trigger_terms);
    rng.shuffle(&mut manifest.capabilities);
    for capability in &mut manifest.capabilities {
        rng.shuffle(&mut capability.evidence);
        if let Some(detail) = &mut capability.detail {
            if let Some(paths) = &mut detail.paths {
                rng.shuffle(paths);
            }
            if let Some(hosts) = &mut detail.hosts {
                rng.shuffle(hosts);
            }
        }
    }
    rng.shuffle(&mut manifest.instructions);
    for instruction in &mut manifest.instructions {
        rng.shuffle(&mut instruction.evidence);
    }
    rng.shuffle(&mut manifest.unresolved);
    if let Advisory::Enabled(run) = &mut manifest.advisory {
        rng.shuffle(&mut run.findings);
        for finding in &mut run.findings {
            rng.shuffle(&mut finding.evidence);
        }
    }
    rng.shuffle(&mut manifest.diagnostics);
}

#[test]
fn shuffled_input_order_is_byte_identical() {
    let base = common::maximal();
    let expected = base.to_canonical_json().unwrap();
    let mut rng = Rng::new(0x5EED_5EED_5EED_5EED);

    for round in 0..1_000 {
        let mut shuffled = base.clone();
        shuffle_all(&mut shuffled, &mut rng);
        assert_eq!(
            shuffled.to_canonical_json().unwrap(),
            expected,
            "round {round}: a different input ordering produced different bytes"
        );
    }
}

#[test]
fn canonical_json_is_a_fixed_point() {
    let once = common::maximal().to_canonical_json().unwrap();
    let twice = Manifest::from_json(&once)
        .unwrap()
        .to_canonical_json()
        .unwrap();
    assert_eq!(
        once, twice,
        "serialize -> parse -> serialize must be a fixed point"
    );
}

#[test]
fn parse_round_trips_to_an_equal_value() {
    let mut expected = common::maximal();
    expected.canonicalize();
    let parsed = Manifest::from_json(&expected.to_canonical_json().unwrap()).unwrap();
    assert_eq!(parsed, expected);
}

#[test]
fn canonicalize_is_idempotent() {
    let mut once = common::maximal();
    once.canonicalize();
    let mut twice = once.clone();
    twice.canonicalize();
    assert_eq!(once, twice);
}

#[test]
fn framing_is_two_space_lf_with_trailing_newline() {
    let json = common::maximal().to_canonical_json().unwrap();
    assert!(
        !json.contains('\r'),
        "CRLF would change the artifact's own hash"
    );
    assert!(
        json.ends_with("}\n"),
        "must end with exactly one trailing newline"
    );
    assert!(!json.ends_with("\n\n"));
    assert!(
        json.contains("\n  \"advisory\": {"),
        "top-level keys must be indented by exactly two spaces"
    );
}

#[test]
fn top_level_keys_are_sorted() {
    let json = common::maximal().to_canonical_json().unwrap();
    let keys: Vec<&str> = json
        .lines()
        .filter_map(|line| line.strip_prefix("  \""))
        .filter_map(|rest| rest.split('"').next())
        .collect();
    let mut sorted = keys.clone();
    sorted.sort_unstable();
    assert_eq!(keys, sorted, "object keys must be sorted at every level");
    assert_eq!(keys.first(), Some(&"advisory"));
    assert_eq!(keys.last(), Some(&"unresolved"));
}

#[test]
fn no_floats_reach_the_artifact() {
    // Invariant 1: a float in this artifact is a score wearing a disguise, and
    // floats are also the classic cross-platform formatting hazard for a document
    // that must be byte-identical everywhere.
    let json = common::maximal().to_canonical_json().unwrap();
    let value: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert!(
        !contains_float(&value),
        "no float may appear anywhere in a manifest"
    );
}

fn contains_float(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::Number(n) => n.is_f64(),
        serde_json::Value::Array(items) => items.iter().any(contains_float),
        serde_json::Value::Object(map) => map.values().any(contains_float),
        _ => false,
    }
}

#[test]
fn empty_detail_is_dropped_rather_than_rendered() {
    let json = common::maximal().to_canonical_json().unwrap();
    assert!(
        !json.contains("\"detail\": {}"),
        "an empty detail object must be dropped, not serialized as {{}}"
    );
}

#[test]
fn advisory_pinning_violations_are_rejected_at_parse() {
    let base = common::maximal().to_canonical_json().unwrap();

    // Ran, but unpinned: the advisory branch would not be reproducible.
    let unpinned = base
        .replace("\"model\": \"claude-sonnet-5\",\n", "")
        .replace(
            "    \"prompt_sha256\": \"sha256:",
            "    \"unused_prompt_sha256\": \"sha256:",
        );
    assert!(
        Manifest::from_json(&unpinned).is_err(),
        "enabled advisory without model must not parse"
    );

    // Did not run, yet carries findings: "not checked" and "checked, found
    // nothing" have to stay distinguishable in a diff.
    let disabled_with_findings = base.replace("\"enabled\": true", "\"enabled\": false");
    assert!(
        Manifest::from_json(&disabled_with_findings).is_err(),
        "disabled advisory carrying findings or pinning must not parse"
    );
}

#[test]
fn unknown_fields_are_rejected() {
    let json = common::maximal().to_canonical_json().unwrap().replace(
        "\"schema_version\"",
        "\"risk_score\": 7,\n  \"schema_version\"",
    );
    assert!(
        Manifest::from_json(&json).is_err(),
        "unknown top-level fields must be rejected, matching additionalProperties: false"
    );
}
