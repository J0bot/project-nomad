//! Content hashing — the addressing primitive of the store.
//!
//! Everything in `tsoin` is addressed by the BLAKE3 hash of its bytes. We wrap
//! the raw 32-byte digest in a small newtype so that hashes are `Copy`, cheap to
//! pass around, hashable (usable as a `HashMap` key), and have a stable
//! hex display for the spec / journal.

use std::fmt;

/// A 256-bit BLAKE3 content hash.
///
/// This is the stable identity of a blob in the store, of a generator (hash of
/// its id string), and the `residue_hash` field of an [`crate::Address`].
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Hash([u8; 32]);

impl Hash {
    /// Hash an arbitrary byte slice with BLAKE3.
    #[must_use]
    pub fn of(bytes: &[u8]) -> Self {
        Hash(*blake3::hash(bytes).as_bytes())
    }

    /// The raw 32 bytes of the digest.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// Construct from raw bytes (e.g. when deserializing an address).
    #[must_use]
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Hash(bytes)
    }

    /// Lowercase hex of the full digest.
    #[must_use]
    pub fn to_hex(&self) -> String {
        let mut s = String::with_capacity(64);
        for b in self.0 {
            s.push_str(&format!("{b:02x}"));
        }
        s
    }

    /// A short prefix for human-readable tables / logs (8 hex chars).
    #[must_use]
    pub fn short(&self) -> String {
        self.to_hex()[..8].to_string()
    }
}

impl fmt::Display for Hash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_hex())
    }
}

impl fmt::Debug for Hash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Hash({})", self.short())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_and_distinct() {
        assert_eq!(Hash::of(b"abc"), Hash::of(b"abc"));
        assert_ne!(Hash::of(b"abc"), Hash::of(b"abd"));
    }

    #[test]
    fn hex_roundtrip_shape() {
        let h = Hash::of(b"hello");
        assert_eq!(h.to_hex().len(), 64);
        assert_eq!(h.short().len(), 8);
        assert_eq!(Hash::from_bytes(*h.as_bytes()), h);
    }
}
