//! Canonical serialization — the only supported way to render a [`Manifest`].
//!
//! Invariant 2: the same bundle produces byte-identical output on any machine,
//! any OS, any locale, any run. Everything in this module exists to make that
//! true rather than aspirational:
//!
//! - **Keys** are sorted at every level. Rendering goes through
//!   [`serde_json::Value`], whose object map is a `BTreeMap` because the
//!   workspace deliberately does not enable serde_json's `preserve_order`
//!   feature. Struct field order therefore cannot leak into the output.
//! - **Arrays** are sorted by the total orders declared in
//!   `docs/02-manifest-schema.md`, with ties broken by each element's own JSON
//!   rendering so the order is genuinely total. A merely *partial* order is a
//!   nondeterminism bug that surfaces only on the one input that has a tie.
//! - **Comparison is byte-wise over UTF-8**, never locale collation. Rust's
//!   `str`/`String` `Ord` is already byte-wise; the point is that nothing here
//!   reaches for anything else. Locale-sensitive comparison would make
//!   "byte-identical on any machine" false on the first non-ASCII path, and it
//!   would fail on the machine with the unusual `LANG` rather than in CI.
//! - **Framing** is two-space indent, LF, UTF-8, no BOM, trailing newline.

use crate::{
    Advisory, AdvisoryFinding, Capability, Detail, Diagnostic, Error, EvidenceAdvisory,
    EvidenceStrict, Instruction, Manifest, Unresolved,
};
use serde::Serialize;

/// The `(file, offset)` pair used as a secondary sort key, owned so it can
/// outlive the element it was read from during decorate-sort-undecorate.
type Head = Option<(String, u64)>;

impl Manifest {
    /// Render to canonical JSON.
    ///
    /// This is the only supported serialization path. Reaching for
    /// [`serde_json::to_string_pretty`] directly elsewhere would produce output
    /// in struct field order with unsorted arrays — valid JSON that silently
    /// breaks invariant 2.
    ///
    /// # Errors
    ///
    /// [`Error::Serialize`] if the value cannot be rendered, which for a
    /// well-formed `Manifest` cannot happen — no map keys are non-strings and no
    /// floats exist in the type graph at all.
    pub fn to_canonical_json(&self) -> Result<String, Error> {
        let mut canonical = self.clone();
        canonical.canonicalize();
        let value = serde_json::to_value(&canonical).map_err(Error::Serialize)?;
        let mut out = serde_json::to_string_pretty(&value).map_err(Error::Serialize)?;
        out.push('\n');
        Ok(out)
    }

    /// Parse a manifest from JSON.
    ///
    /// Unknown fields are rejected, matching the schema's
    /// `additionalProperties: false`, and an `advisory` object that violates the
    /// schema's pinning rule fails here rather than becoming an unrepresentable
    /// value.
    ///
    /// # Errors
    ///
    /// [`Error::Parse`] if the input is not a manifest this crate can represent.
    pub fn from_json(json: &str) -> Result<Self, Error> {
        serde_json::from_str(json).map_err(Error::Parse)
    }

    /// Put every array into its declared order and drop empty optionals.
    ///
    /// Idempotent: canonicalizing an already-canonical manifest is a no-op, which
    /// is what makes [`Manifest::to_canonical_json`] a fixed point.
    pub fn canonicalize(&mut self) {
        sort_canonically(&mut self.inventory, |entry| entry.path.clone());

        self.disclosure.declared_capabilities.sort();
        self.disclosure.declared_capabilities.dedup();
        self.disclosure.trigger_terms.sort();
        self.disclosure.trigger_terms.dedup();

        // Evidence first: the parent orders are defined in terms of their *first*
        // evidence entry, so sorting parents before children would key them off
        // whichever evidence happened to be listed first.
        for capability in &mut self.capabilities {
            sort_strict_evidence(&mut capability.evidence);
            normalize_detail(&mut capability.detail);
        }
        sort_canonically(&mut self.capabilities, |c: &Capability| {
            (c.capability.as_str(), strict_head(&c.evidence))
        });

        for instruction in &mut self.instructions {
            sort_strict_evidence(&mut instruction.evidence);
        }
        sort_canonically(&mut self.instructions, |i: &Instruction| {
            (i.signal.as_str(), strict_head(&i.evidence))
        });

        sort_canonically(&mut self.unresolved, |u: &Unresolved| {
            (u.file.clone(), u.reason.as_str(), u.start_byte)
        });

        if let Advisory::Enabled(run) = &mut self.advisory {
            for finding in &mut run.findings {
                sort_canonically(&mut finding.evidence, |e: &EvidenceAdvisory| {
                    (e.file.clone(), e.start_line)
                });
            }
            sort_canonically(&mut run.findings, |f: &AdvisoryFinding| {
                (f.kind.as_str(), advisory_head(&f.evidence), f.claim.clone())
            });
        }

        sort_canonically(&mut self.diagnostics, |d: &Diagnostic| {
            (d.code.as_str(), d.file.clone())
        });
    }
}

/// Sort `items` by `key`, breaking ties with each element's JSON rendering.
///
/// The tiebreak is what makes the order **total**. Two elements that agree on
/// every declared sort key would otherwise keep whatever relative order they were
/// built in, so a caller that assembled findings in a different sequence — a
/// different filesystem walk order, a different rule evaluation order — would get
/// different bytes out. Elements whose renderings are also equal are byte-for-byte
/// identical, so their relative order is unobservable in the output.
///
/// Deliberately not `#[derive(Ord)]` on the element types: that would make the
/// artifact's byte content depend on the *declaration order of struct fields and
/// enum variants*, so a cosmetic reordering in a future PR would silently change
/// every manifest in every repo.
fn sort_canonically<T, K, F>(items: &mut Vec<T>, key: F)
where
    T: Serialize,
    K: Ord,
    F: Fn(&T) -> K,
{
    let mut decorated: Vec<(K, String, T)> = items
        .drain(..)
        .map(|item| {
            let sort_key = key(&item);
            // This `to_string` is a *sort key*, never output — it is discarded
            // below and nothing outside this function sees it. The rule that
            // serde_json output goes through `to_canonical_json` alone is intact.
            //
            // Serializing a value that is about to be serialized anyway cannot
            // realistically fail. If it somehow did, falling back to an empty
            // tiebreak leaves the declared key order intact — invariant 10 says
            // a library crate does not get to panic over it.
            let rendered = serde_json::to_string(&item).unwrap_or_default();
            (sort_key, rendered, item)
        })
        .collect();
    decorated.sort_by(|a, b| (&a.0, &a.1).cmp(&(&b.0, &b.1)));
    items.extend(decorated.into_iter().map(|(_, _, item)| item));
}

/// Evidence for the deterministic tiers sorts by `(file, start_byte)`.
fn sort_strict_evidence(evidence: &mut Vec<EvidenceStrict>) {
    sort_canonically(evidence, |e: &EvidenceStrict| {
        (e.file.clone(), e.start_byte)
    });
}

/// The `(file, start_byte)` of the first strict evidence entry, if any.
fn strict_head(evidence: &[EvidenceStrict]) -> Head {
    evidence.first().map(|e| (e.file.clone(), e.start_byte))
}

/// The `(file, start_line)` of the first advisory evidence entry, if any.
fn advisory_head(evidence: &[EvidenceAdvisory]) -> Head {
    evidence.first().map(|e| (e.file.clone(), e.start_line))
}

/// Sort and deduplicate a detail list, dropping it entirely once empty.
fn normalize_list(list: &mut Option<Vec<String>>) {
    let Some(values) = list.as_mut() else { return };
    values.sort();
    values.dedup();
    if values.is_empty() {
        *list = None;
    }
}

/// Canonicalize a `detail`, dropping it entirely when nothing survives, so a bare
/// `{}` never reaches an artifact required to be byte-identical.
fn normalize_detail(detail: &mut Option<Detail>) {
    let Some(inner) = detail.as_mut() else { return };
    normalize_list(&mut inner.paths);
    normalize_list(&mut inner.hosts);
    if inner.is_empty() {
        *detail = None;
    }
}
