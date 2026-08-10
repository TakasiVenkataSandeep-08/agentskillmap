//! Invariant 2, as an executable claim.
//!
//! T1's acceptance criterion: *"a hand-built manifest with shuffled input
//! ordering serializes byte-identically across 1,000 randomized field-insertion
//! orders."* That is [`shuffled_input_order_is_byte_identical`].
//!
//! The shuffle suite is only as strong as the fixture underneath it. With no two
//! elements tied on their declared sort keys, the declared keys alone are already
//! total and the whole tiebreak mechanism could be deleted with every test still
//! green. [`the_fixture_actually_contains_ties`] is what stops that from silently
//! becoming true again.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "a failed unwrap, panic, or out-of-bounds index in a test is the test \
              failing, which is the point. Invariant 10 bans these in library code, \
              where hostile input is the normal case and a crash is a DoS on \
              somebody's CI; a test binary has no CI to take down but its own. \
              serde_json's Value indexing in particular is the readable way to \
              assert on manifest structure."
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
        rng.shuffle(capability.evidence.as_mut_slice());
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
        rng.shuffle(instruction.evidence.as_mut_slice());
    }
    rng.shuffle(&mut manifest.unresolved);
    if let Advisory::Enabled(run) = &mut manifest.advisory {
        rng.shuffle(&mut run.findings);
        for finding in &mut run.findings {
            rng.shuffle(finding.evidence.as_mut_slice());
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
    expected.canonicalize().unwrap();
    let parsed = Manifest::from_json(&expected.to_canonical_json().unwrap()).unwrap();
    assert_eq!(parsed, expected);
}

#[test]
fn canonicalize_is_idempotent() {
    let mut once = common::maximal();
    once.canonicalize().unwrap();
    let mut twice = once.clone();
    twice.canonicalize().unwrap();
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

/// The manifest as a `Value`, for mutation-based negative cases.
fn base_value() -> serde_json::Value {
    serde_json::from_str(&common::maximal().to_canonical_json().unwrap()).unwrap()
}

#[test]
fn advisory_pinning_violations_are_rejected_at_parse() {
    // Built by editing the parsed JSON, not by string replacement. A rename-based
    // mutation trips `deny_unknown_fields` first and never reaches the pinning
    // logic it claims to test, so such an assertion would pass no matter what
    // that logic did — including if it were deleted outright.
    let base = base_value();
    let with_advisory = |advisory: serde_json::Value| {
        let mut manifest = base.clone();
        manifest["advisory"] = advisory;
        serde_json::to_string(&manifest).unwrap()
    };

    // Ran, but no model: the advisory branch is not reproducible.
    let mut advisory = base["advisory"].clone();
    advisory.as_object_mut().unwrap().remove("model");
    assert!(
        Manifest::from_json(&with_advisory(advisory)).is_err(),
        "enabled advisory without `model` must not parse"
    );

    // Ran, but no prompt hash: same problem.
    let mut advisory = base["advisory"].clone();
    advisory.as_object_mut().unwrap().remove("prompt_sha256");
    assert!(
        Manifest::from_json(&with_advisory(advisory)).is_err(),
        "enabled advisory without `prompt_sha256` must not parse"
    );

    // Did not run, yet carries findings. "Not checked" and "checked, found
    // nothing" have to stay distinguishable in a diff.
    let mut advisory = base["advisory"].clone();
    {
        let object = advisory.as_object_mut().unwrap();
        object.insert("enabled".to_owned(), serde_json::Value::Bool(false));
        object.remove("model");
        object.remove("prompt_sha256");
    }
    assert!(
        advisory["findings"]
            .as_array()
            .is_some_and(|f| !f.is_empty()),
        "this case only means anything while findings are non-empty"
    );
    assert!(
        Manifest::from_json(&with_advisory(advisory)).is_err(),
        "disabled advisory carrying findings must not parse"
    );

    // Did not run, yet pins a model.
    let mut advisory = base["advisory"].clone();
    {
        let object = advisory.as_object_mut().unwrap();
        object.insert("enabled".to_owned(), serde_json::Value::Bool(false));
        object.insert("findings".to_owned(), serde_json::Value::Array(vec![]));
        object.remove("prompt_sha256");
    }
    assert!(
        Manifest::from_json(&with_advisory(advisory)).is_err(),
        "disabled advisory pinning a model must not parse"
    );

    // The legal disabled shape still parses, or all four assertions above could
    // be passing for the wrong reason.
    let disabled = serde_json::json!({ "enabled": false, "findings": [] });
    assert!(
        Manifest::from_json(&with_advisory(disabled)).is_ok(),
        "a correctly disabled advisory must still parse"
    );
}

#[test]
fn a_finding_with_no_evidence_cannot_be_parsed() {
    // The schema declares minItems: 1 on every evidence array. Invariant 4: a
    // finding nobody can point at cannot be regression-tested. The types must not
    // be able to accept one, or skillmap-core can emit a manifest that fails its
    // own schema — which the golden fixture would never catch, since every list
    // in it is populated.
    let base = base_value();
    for pointer in [
        "/capabilities/0/evidence",
        "/instructions/0/evidence",
        "/advisory/findings/0/evidence",
    ] {
        let mut mutated = base.clone();
        *mutated.pointer_mut(pointer).unwrap() = serde_json::Value::Array(vec![]);
        assert!(
            Manifest::from_json(&serde_json::to_string(&mutated).unwrap()).is_err(),
            "an empty evidence array at {pointer} must be rejected"
        );
    }
}

#[test]
fn line_zero_cannot_be_parsed() {
    // The schema declares minimum: 1 on every start_line. Line 0 does not exist.
    let base = base_value();
    for pointer in [
        "/capabilities/0/evidence/0/start_line",
        "/advisory/findings/0/evidence/0/start_line",
    ] {
        let mut mutated = base.clone();
        *mutated.pointer_mut(pointer).unwrap() = serde_json::json!(0);
        assert!(
            Manifest::from_json(&serde_json::to_string(&mutated).unwrap()).is_err(),
            "start_line 0 at {pointer} must be rejected"
        );
    }
}

#[test]
fn unknown_fields_are_rejected() {
    let mut mutated = base_value();
    mutated
        .as_object_mut()
        .unwrap()
        .insert("risk_score".to_owned(), serde_json::json!(7));
    assert!(
        Manifest::from_json(&serde_json::to_string(&mutated).unwrap()).is_err(),
        "unknown top-level fields must be rejected, matching additionalProperties: false"
    );
}

#[test]
fn absent_optional_sort_keys_order_before_present_ones() {
    // docs/02-manifest-schema.md: an absent `start_byte` in `unresolved`, and an
    // absent `file` in `diagnostics`, each sort BEFORE any present one. Both rules
    // are only reachable on a tie in the preceding keys, so this asserts on the
    // fixture's deliberately tied pairs rather than on the arrays as a whole.
    let manifest = base_value();

    let unresolved: Vec<_> = manifest["unresolved"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|u| u["file"] == "scripts/run.sh" && u["reason"] == "dynamic_dispatch")
        .collect();
    assert_eq!(unresolved.len(), 2, "fixture must carry the tied pair");
    assert!(
        unresolved[0].get("start_byte").is_none(),
        "an absent start_byte must sort before a present one"
    );

    let diagnostics: Vec<_> = manifest["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|d| d["code"] == "policy_load_error")
        .collect();
    assert_eq!(diagnostics.len(), 2, "fixture must carry the tied pair");
    assert!(
        diagnostics[0].get("file").is_none(),
        "an absent file must sort before a present one"
    );
}

#[test]
fn the_fixture_actually_contains_ties() {
    // Guards the guard. Every tie-dependent assertion in this file — and the
    // shuffle suite's ability to detect a missing tiebreak at all — is vacuous if
    // no two elements agree on their declared sort keys. Removing a tie from the
    // fixture must fail loudly here rather than quietly turning other tests into
    // no-ops.
    let manifest = common::maximal();

    let evidence_keys: Vec<_> = manifest
        .capabilities
        .iter()
        .flat_map(|c| c.evidence.iter())
        .map(|e| (e.file.clone(), e.start_byte))
        .collect();
    assert!(
        has_duplicate(&evidence_keys),
        "fixture must contain two strict evidence entries tied on (file, start_byte)"
    );

    let unresolved_keys: Vec<_> = manifest
        .unresolved
        .iter()
        .map(|u| (u.file.clone(), u.reason.as_str()))
        .collect();
    assert!(
        has_duplicate(&unresolved_keys),
        "fixture must contain two unresolved entries tied on (file, reason)"
    );

    let diagnostic_keys: Vec<_> = manifest
        .diagnostics
        .iter()
        .map(|d| d.code.as_str())
        .collect();
    assert!(
        has_duplicate(&diagnostic_keys),
        "fixture must contain two diagnostics tied on code"
    );

    let Advisory::Enabled(run) = &manifest.advisory else {
        panic!("fixture advisory must be enabled");
    };
    let finding_keys: Vec<_> = run
        .findings
        .iter()
        .map(|f| {
            (
                f.kind.as_str(),
                f.evidence.first().map(|e| (e.file.clone(), e.start_line)),
            )
        })
        .collect();
    assert!(
        has_duplicate(&finding_keys),
        "fixture must contain two advisory findings tied on (kind, first evidence)"
    );
}

#[test]
fn tiebreak_ignores_struct_field_declaration_order() {
    // The discriminating case, and the reason this test exists separately from
    // the fixture-based one below: a tied pair where sorted-key rendering and
    // declaration-order rendering disagree about which element comes first.
    //
    // `Capability`'s fields are declared (capability, reachability, detail,
    // evidence) but sort to (capability, detail, evidence, reachability). So for
    // two capabilities tied on the declared key `(capability, first evidence)`:
    //
    //   declaration order -> `reachability` decides: "observed" < "present" -> A
    //   sorted-key order  -> `detail` decides:       ["a"] < ["z"]           -> B
    //
    // If the tiebreak ever renders with `serde_json::to_string` on the struct
    // instead of going through `serde_json::Value`, this flips and the assertion
    // fails. Nothing else in the suite catches that: it needs a tie whose two
    // renderings disagree, which no semantically ordinary fixture produces.
    let mut manifest = common::maximal();
    let shared = manifest.capabilities[0].evidence.first().unwrap().clone();

    let build = |reachability, path: &str| skillmap_core::Capability {
        capability: skillmap_core::CapabilityTerm::NetEgress,
        reachability,
        detail: Some(skillmap_core::Detail {
            paths: Some(vec![path.to_owned()]),
            hosts: None,
        }),
        evidence: skillmap_core::NonEmpty::of(shared.clone(), []),
    };

    let a = build(skillmap_core::Reachability::Observed, "z");
    let b = build(skillmap_core::Reachability::Present, "a");

    for input in [vec![a.clone(), b.clone()], vec![b.clone(), a.clone()]] {
        manifest.capabilities = input;
        let rendered = manifest.to_canonical_json().unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&rendered).unwrap();
        let first = &parsed["capabilities"][0];
        assert_eq!(
            first["detail"]["paths"][0], "a",
            "tied capabilities must be ordered by their canonical (sorted-key) \
             rendering, where `detail` decides — not by declaration order, where \
             `reachability` would"
        );
        assert_eq!(first["reachability"], "present");
    }
}

fn has_duplicate<T: Ord + Clone>(items: &[T]) -> bool {
    let mut sorted = items.to_vec();
    sorted.sort();
    let before = sorted.len();
    sorted.dedup();
    sorted.len() < before
}

#[test]
fn tiebreak_uses_canonical_rendering_not_field_declaration_order() {
    // The tiebreak renders each element through serde_json::Value, whose map is a
    // BTreeMap, so the comparison sees keys in sorted order — the same canonical
    // form the artifact itself is written in.
    //
    // Rendering with serde_json::to_string directly on the struct instead would
    // compare fields in DECLARATION order, and then moving a field in manifest.rs
    // would silently reorder tied elements in every manifest in every repo. This
    // asserts the property that rules that out: tied elements come out in
    // sorted-key-rendering order.
    // The two evidence entries at scripts/collect.py:610 — two rules firing at one
    // site. They agree on the WHOLE declared key `(file, start_byte)`, so nothing
    // but the tiebreak can separate them. (The tied `unresolved` pair is not usable
    // here: it is separated by the declared key itself, via the absent-start_byte
    // rule, before the tiebreak is ever consulted.)
    let manifest = base_value();

    let tied: Vec<String> = manifest["capabilities"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|c| c["evidence"].as_array().unwrap())
        .filter(|e| e["file"] == "scripts/collect.py" && e["start_byte"] == 610)
        // Value::to_string is key-sorted, so this is each element's canonical form.
        .map(std::string::ToString::to_string)
        .collect();
    assert_eq!(tied.len(), 2, "fixture must carry the fully tied pair");

    let mut expected = tied.clone();
    expected.sort();
    assert_eq!(
        tied, expected,
        "tied elements must be emitted in canonical-rendering order"
    );
}
