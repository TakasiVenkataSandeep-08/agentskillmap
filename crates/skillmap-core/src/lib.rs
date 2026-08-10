//! The manifest spine: types, canonical serialization, and content digests.
//!
//! This crate is the bottom of the dependency graph — it depends on no other
//! crate in the workspace — because everything downstream produces or consumes
//! the manifest it defines. See `ARCHITECTURE.md`.
//!
//! Three things live here and nowhere else:
//!
//! 1. [`Manifest`] and the types it is built from, mirroring
//!    `schema/manifest-v1.schema.json`. The three assurance tiers are separate
//!    types in separate fields, so invariant 5 is enforced by shape rather than
//!    by every consumer filtering correctly.
//! 2. [`Manifest::to_canonical_json`], the only supported serialization path.
//! 3. [`content_digest`], the merkle root that gives a bundle its identity.
//!
//! ```
//! use skillmap_core::{Digest, content_digest};
//!
//! let files = vec![
//!     ("SKILL.md".to_owned(), Digest::of(b"---\nname: example\n---\n")),
//!     ("scripts/collect.py".to_owned(), Digest::of(b"print(1)\n")),
//! ];
//! // Discovery order cannot change a bundle's identity.
//! let mut shuffled = files.clone();
//! shuffled.reverse();
//! assert_eq!(content_digest(&files), content_digest(&shuffled));
//! ```

mod canonical;
mod digest;
mod error;
mod manifest;

pub use digest::{content_digest, Digest};
pub use error::Error;
pub use manifest::{
    Advisory, AdvisoryFinding, AdvisoryKind, AdvisoryRun, BundleKind, Capability, CapabilityTerm,
    Detail, Diagnostic, DiagnosticCode, Disclosure, EvidenceAdvisory, EvidenceStrict, Instruction,
    InstructionSignal, InventoryEntry, LoadPhase, Manifest, ParseStatus, Reachability, Target,
    Tool, Unresolved, UnresolvedReason, SCHEMA_VERSION,
};
