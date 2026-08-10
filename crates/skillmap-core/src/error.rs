//! Error type for this crate.

use thiserror::Error;

/// Anything that can go wrong producing or consuming a manifest.
///
/// Deliberately small and non-`panic`king: invariant 10 makes hostile, malformed
/// input the normal case, so every failure here is a value a caller can handle.
#[derive(Debug, Error)]
pub enum Error {
    /// The manifest could not be rendered to canonical JSON.
    #[error("canonical serialization failed: {0}")]
    Serialize(#[source] serde_json::Error),

    /// The input was not a manifest this crate can represent.
    #[error("manifest parse failed: {0}")]
    Parse(#[source] serde_json::Error),

    /// A digest was not `sha256:` followed by exactly 64 lowercase hex digits.
    #[error("invalid digest {0:?}: expected `sha256:` followed by 64 lowercase hex digits")]
    InvalidDigest(String),

    /// The `advisory` object violated the pinning rule that the schema's
    /// `if`/`then` pair encodes: a pass that ran must name its model and pin its
    /// prompt hash, and a pass that did not run must carry neither, nor findings.
    ///
    /// Unrepresentable once parsed — [`crate::Advisory`] is an enum — so this can
    /// only surface at the deserialization boundary.
    #[error("advisory object is internally inconsistent: {0}")]
    InconsistentAdvisory(&'static str),
}
