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
//! - **Arrays** are sorted by the orders declared in
//!   `docs/02-manifest-schema.md`, with ties broken by each element's own
//!   *canonical* (sorted-key) rendering so the order is genuinely total. Those
//!   declared keys are not total on their own, and a merely partial order is a
//!   nondeterminism bug that surfaces only on the one input that has a tie.
//! - **Comparison is byte-wise over UTF-8**, never locale collation. Rust's
//!   `str`/`String` `Ord` is already byte-wise; the point is that nothing here
//!   reaches for anything else. Locale-sensitive comparison would make
//!   "byte-identical on any machine" false on the first non-ASCII path, and it
//!   would fail on the machine with the unusual `LANG` rather than in CI.
//! - **Framing** is two-space indent, LF, UTF-8, no BOM, trailing newline.

use crate::{
    Advisory, AdvisoryFinding, Capability, Detail, Diagnostic, Error, EvidenceAdvisory,
    EvidenceStrict, Instruction, Manifest, NonEmpty, Unresolved,
};
use serde::Serialize;
use std::num::NonZeroU64;

/// `(file, start_byte)` of a parent's first strict evidence entry, owned so it
/// can outlive the element it was read from during decorate-sort-undecorate.
///
/// `Option` only because [`NonEmpty::first`] returns one — see the note there;
/// an evidence list is never actually empty.
type StrictHead = Option<(String, u64)>;

/// `(file, start_line)` of a finding's first advisory evidence entry. Distinct
/// from [`StrictHead`] because the strict tier keys on a byte offset (minimum 0)
/// and the advisory tier on a line number (minimum 1).
type AdvisoryHead = Option<(String, NonZeroU64)>;

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
        canonical.canonicalize()?;
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
    ///
    /// # Errors
    ///
    /// [`Error::Serialize`] if an element cannot be rendered for the tiebreak.
    /// This propagates rather than being swallowed on purpose: falling back to a
    /// constant tiebreak would leave tied elements in insertion order, which is
    /// silent nondeterminism inside the function whose entire job is to prevent
    /// it. A loud failure is strictly better than a manifest that is subtly
    /// machine-dependent.
    pub fn canonicalize(&mut self) -> Result<(), Error> {
        sort_canonically(&mut self.inventory, |entry| entry.path.clone())?;

        self.disclosure.declared_capabilities.sort();
        self.disclosure.declared_capabilities.dedup();
        self.disclosure.trigger_terms.sort();
        self.disclosure.trigger_terms.dedup();

        // Evidence first: the parent orders are defined in terms of their *first*
        // evidence entry, so sorting parents before children would key them off
        // whichever evidence happened to be listed first.
        for capability in &mut self.capabilities {
            sort_strict_evidence(&mut capability.evidence)?;
            normalize_detail(&mut capability.detail);
        }
        sort_canonically(&mut self.capabilities, |c: &Capability| {
            (c.capability.as_str(), strict_head(&c.evidence))
        })?;

        for instruction in &mut self.instructions {
            sort_strict_evidence(&mut instruction.evidence)?;
        }
        sort_canonically(&mut self.instructions, |i: &Instruction| {
            (i.signal.as_str(), strict_head(&i.evidence))
        })?;

        sort_canonically(&mut self.unresolved, |u: &Unresolved| {
            (u.file.clone(), u.reason.as_str(), u.start_byte)
        })?;

        if let Advisory::Enabled(run) = &mut self.advisory {
            for finding in &mut run.findings {
                sort_canonically(finding.evidence.as_mut_vec(), |e: &EvidenceAdvisory| {
                    (e.file.clone(), e.start_line)
                })?;
            }
            sort_canonically(&mut run.findings, |f: &AdvisoryFinding| {
                (f.kind.as_str(), advisory_head(&f.evidence), f.claim.clone())
            })?;
        }

        sort_canonically(&mut self.diagnostics, |d: &Diagnostic| {
            (d.code.as_str(), d.file.clone())
        })?;

        Ok(())
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
fn sort_canonically<T, K, F>(items: &mut Vec<T>, key: F) -> Result<(), Error>
where
    T: Serialize,
    K: Ord,
    F: Fn(&T) -> K,
{
    // Render every element before taking ownership of any, so a failure leaves
    // `items` exactly as it was rather than half-drained.
    //
    // `to_value` first, then `to_string` on the Value, is load-bearing and not a
    // detour: `serde_json::to_string` applied straight to a struct emits fields in
    // DECLARATION order, which would make this tiebreak — and therefore the bytes
    // of every manifest containing a tie — depend on the order fields happen to
    // appear in `manifest.rs`. Going through `Value` sorts keys (its map is a
    // `BTreeMap`), so the tiebreak compares the same canonical form the artifact
    // itself is written in, and moving a struct field is invisible to the output.
    //
    // These renderings are sort keys, never output; they are discarded below and
    // nothing outside this function sees them.
    let mut rendered: Vec<String> = Vec::with_capacity(items.len());
    for item in items.iter() {
        let value = serde_json::to_value(item).map_err(Error::Serialize)?;
        rendered.push(value.to_string());
    }

    let mut decorated: Vec<(K, String, T)> = items
        .drain(..)
        .zip(rendered)
        .map(|(item, rendered)| (key(&item), rendered, item))
        .collect();
    decorated.sort_by(|a, b| (&a.0, &a.1).cmp(&(&b.0, &b.1)));
    items.extend(decorated.into_iter().map(|(_, _, item)| item));
    Ok(())
}

/// Evidence for the deterministic tiers sorts by `(file, start_byte)`.
fn sort_strict_evidence(evidence: &mut NonEmpty<EvidenceStrict>) -> Result<(), Error> {
    sort_canonically(evidence.as_mut_vec(), |e: &EvidenceStrict| {
        (e.file.clone(), e.start_byte)
    })
}

/// The `(file, start_byte)` of the first strict evidence entry.
fn strict_head(evidence: &NonEmpty<EvidenceStrict>) -> StrictHead {
    evidence.first().map(|e| (e.file.clone(), e.start_byte))
}

/// The `(file, start_line)` of the first advisory evidence entry.
fn advisory_head(evidence: &NonEmpty<EvidenceAdvisory>) -> AdvisoryHead {
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
