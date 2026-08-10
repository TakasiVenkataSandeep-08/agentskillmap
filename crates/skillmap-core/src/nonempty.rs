//! A list that cannot be empty.

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// A list guaranteed to hold at least one element.
///
/// Exists for one reason: invariant 4 says a finding without provenance is not a
/// finding, and the schema spells that as `"minItems": 1` on every `evidence`
/// array. A plain `Vec` would let `skillmap-core` render a capability with an
/// empty evidence list — valid Rust, valid serde, and a manifest that fails its
/// own schema. Emptiness is unrepresentable here instead, the same way
/// [`crate::Advisory`] makes an unpinned-but-enabled semantic pass
/// unrepresentable.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NonEmpty<T>(Vec<T>);

impl<T> NonEmpty<T> {
    /// Wrap `items`, or return `None` if it is empty.
    pub fn new(items: Vec<T>) -> Option<Self> {
        if items.is_empty() {
            None
        } else {
            Some(Self(items))
        }
    }

    /// Build from a first element and any number of further ones. Total, so it
    /// needs no error path at the call site.
    pub fn of(first: T, rest: impl IntoIterator<Item = T>) -> Self {
        let mut items = vec![first];
        items.extend(rest);
        Self(items)
    }

    /// The first element.
    ///
    /// Returns `Option` only because this crate denies `unwrap`, `expect`, and
    /// indexing in library code (invariant 10) — not because emptiness is
    /// representable. It never is.
    pub fn first(&self) -> Option<&T> {
        self.0.first()
    }

    /// The elements, in order.
    pub fn as_slice(&self) -> &[T] {
        &self.0
    }

    /// The elements, mutably.
    ///
    /// Safe to expose: a `&mut [T]` can reorder or replace elements but cannot
    /// change the length, so the non-empty invariant holds no matter what a
    /// caller does with it.
    pub fn as_mut_slice(&mut self) -> &mut [T] {
        &mut self.0
    }

    /// Iterate over the elements.
    pub fn iter(&self) -> std::slice::Iter<'_, T> {
        self.0.iter()
    }

    /// How many elements. Always at least 1.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Always `false`. Present because clippy asks for it beside `len`.
    pub fn is_empty(&self) -> bool {
        false
    }

    /// Consume into a plain `Vec`.
    pub fn into_vec(self) -> Vec<T> {
        self.0
    }

    /// Mutable access to the backing `Vec`, for canonicalization only.
    ///
    /// Crate-private on purpose: reordering elements preserves the non-empty
    /// invariant, but truncating would break it, and no caller outside
    /// [`crate::canonical`] has any reason to do either.
    pub(crate) fn as_mut_vec(&mut self) -> &mut Vec<T> {
        &mut self.0
    }
}

impl<'a, T> IntoIterator for &'a NonEmpty<T> {
    type Item = &'a T;
    type IntoIter = std::slice::Iter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

impl<T: Serialize> Serialize for NonEmpty<T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

impl<'de, T: Deserialize<'de>> Deserialize<'de> for NonEmpty<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let items = Vec::<T>::deserialize(deserializer)?;
        Self::new(items).ok_or_else(|| {
            serde::de::Error::custom(
                "expected at least one element: a finding with no evidence cannot be \
                 pointed at, and cannot be regression-tested (invariant 4)",
            )
        })
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    reason = "a failed unwrap in a test is the test failing"
)]
mod tests {
    use super::*;

    #[test]
    fn rejects_empty_at_construction() {
        assert!(NonEmpty::<u8>::new(vec![]).is_none());
        assert!(NonEmpty::new(vec![1u8]).is_some());
    }

    #[test]
    fn rejects_empty_at_deserialization() {
        assert!(serde_json::from_str::<NonEmpty<u8>>("[]").is_err());
        assert_eq!(
            serde_json::from_str::<NonEmpty<u8>>("[1,2]")
                .unwrap()
                .as_slice(),
            &[1, 2]
        );
    }

    #[test]
    fn serializes_as_a_plain_array() {
        let list = NonEmpty::of(1u8, [2, 3]);
        assert_eq!(serde_json::to_string(&list).unwrap(), "[1,2,3]");
    }
}
