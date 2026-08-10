//! SHA-256 digests and the bundle content digest.

use crate::Error;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use sha2::{Digest as _, Sha256};

/// The `sha256:` prefix every digest carries in the manifest.
const PREFIX: &str = "sha256:";

/// A SHA-256 digest.
///
/// Stored as the raw 32 bytes and rendered as `sha256:<64 lowercase hex>` on the
/// wire, matching the schema's `digest` pattern. Keeping the raw bytes rather
/// than the string means [`content_digest`] never has to re-parse hex it just
/// printed, and makes the "64 lowercase hex digits" guarantee structural instead
/// of something every construction site has to remember.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Digest([u8; 32]);

impl Digest {
    /// Wrap 32 raw digest bytes.
    #[must_use]
    pub const fn from_raw(raw: [u8; 32]) -> Self {
        Self(raw)
    }

    /// Hash `bytes` with SHA-256.
    #[must_use]
    pub fn of(bytes: &[u8]) -> Self {
        Self(Sha256::digest(bytes).into())
    }

    /// The raw 32 digest bytes.
    #[must_use]
    pub const fn as_raw(&self) -> &[u8; 32] {
        &self.0
    }

    /// Render as `sha256:<64 lowercase hex>` — the manifest wire form.
    #[must_use]
    pub fn to_wire(&self) -> String {
        let mut out = String::with_capacity(PREFIX.len() + 64);
        out.push_str(PREFIX);
        for byte in &self.0 {
            // No `write!`: formatting into a String cannot fail, and swallowing a
            // Result to say so reads worse than two pushes.
            out.push(hex_digit(byte >> 4));
            out.push(hex_digit(byte & 0x0f));
        }
        out
    }

    /// Parse the `sha256:<64 lowercase hex>` wire form.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidDigest`] unless `s` is exactly the prefix followed
    /// by 64 lowercase hex digits. Uppercase is rejected on purpose: two spellings
    /// of one digest would be two different bytes in an artifact required to be
    /// byte-identical.
    pub fn parse(s: &str) -> Result<Self, Error> {
        let invalid = || Error::InvalidDigest(s.to_owned());
        let hex = s.strip_prefix(PREFIX).ok_or_else(invalid)?;
        if hex.len() != 64 {
            return Err(invalid());
        }
        let mut raw = [0u8; 32];
        let mut pairs = hex.as_bytes().chunks_exact(2);
        for slot in &mut raw {
            let pair = pairs.next().ok_or_else(invalid)?;
            let hi = pair
                .first()
                .copied()
                .and_then(hex_value)
                .ok_or_else(invalid)?;
            let lo = pair
                .get(1)
                .copied()
                .and_then(hex_value)
                .ok_or_else(invalid)?;
            *slot = (hi << 4) | lo;
        }
        Ok(Self(raw))
    }
}

/// Map a nibble to its lowercase hex character.
const fn hex_digit(nibble: u8) -> char {
    match nibble {
        0..=9 => (b'0' + nibble) as char,
        _ => (b'a' + nibble - 10) as char,
    }
}

/// Map a lowercase hex byte to its value, rejecting uppercase and non-hex.
const fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
}

impl Serialize for Digest {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_wire())
    }
}

impl<'de> Deserialize<'de> for Digest {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        Self::parse(&s).map_err(serde::de::Error::custom)
    }
}

/// Compute a bundle's `content_digest`: a merkle root over the inventory,
/// covering **file bytes only**.
///
/// ```text
/// leaf_i = sha256( path_i_utf8 || 0x00 || raw_sha256_bytes_i )
/// root   = sha256( leaf_0 || leaf_1 || … || leaf_n )
/// ```
///
/// `files` is sorted here rather than trusted to arrive sorted — leaves must be
/// in byte-wise path order or the root is caller-dependent, which is exactly the
/// class of bug invariant 2 exists to prevent.
///
/// `load_phase` and `parse_status` are deliberately **not** inputs. The digest
/// means "these bytes", nothing more; folding classification in would invalidate
/// every `skillmap.lock` in every repo each time the load-phase classifier
/// improved, with no matching change in what the skill can actually do.
#[must_use]
pub fn content_digest(files: &[(String, Digest)]) -> Digest {
    let mut sorted: Vec<&(String, Digest)> = files.iter().collect();
    sorted.sort_by(|a, b| a.0.as_bytes().cmp(b.0.as_bytes()));

    let mut root = Sha256::new();
    for (path, file_digest) in sorted {
        let mut leaf = Sha256::new();
        leaf.update(path.as_bytes());
        leaf.update([0x00]);
        leaf.update(file_digest.as_raw());
        root.update(leaf.finalize());
    }
    Digest::from_raw(root.finalize().into())
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    reason = "a failed unwrap in a test is the test failing"
)]
mod tests {
    use super::*;

    #[test]
    fn wire_form_round_trips() {
        let digest = Digest::of(b"hello");
        let wire = digest.to_wire();
        assert!(wire.starts_with("sha256:"));
        assert_eq!(wire.len(), 71);
        assert_eq!(Digest::parse(&wire).unwrap(), digest);
    }

    #[test]
    fn known_vector() {
        // SHA-256 of the empty string — the standard test vector.
        assert_eq!(
            Digest::of(b"").to_wire(),
            "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn rejects_malformed() {
        for bad in [
            "",
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
            "sha256:tooshort",
            "sha256:E3B0C44298FC1C149AFBF4C8996FB92427AE41E4649B934CA495991B7852B855",
            "sha1:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
            "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b85z",
        ] {
            assert!(Digest::parse(bad).is_err(), "should have rejected {bad:?}");
        }
    }

    #[test]
    fn content_digest_is_order_independent() {
        let a = ("a.py".to_owned(), Digest::of(b"aaa"));
        let b = ("b.py".to_owned(), Digest::of(b"bbb"));
        assert_eq!(
            content_digest(&[a.clone(), b.clone()]),
            content_digest(&[b, a]),
            "the merkle root must not depend on the order files were discovered in"
        );
    }

    #[test]
    fn content_digest_separates_path_from_contents() {
        // Without the 0x00 separator, ("ab", X) and ("a", "b"||X) would collide.
        let left = [("ab".to_owned(), Digest::of(b""))];
        let right = [("a".to_owned(), Digest::of(b"b"))];
        assert_ne!(content_digest(&left), content_digest(&right));
    }
}
