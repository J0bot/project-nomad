//! The generative address — the triplet. THE new primitive (synthesis §4 layer 2).
//!
//! `Address = (generator_hash, coordinates, residue_hash)`.
//!
//! It says nothing about *content*; it says how to *recompute* content:
//! "run the generator with this hash at these coordinates, then apply the
//! residue blob with this hash". The residue blob itself lives in the
//! [`crate::Store`]; the address only carries its hash. This is the field layout
//! proposed FROZEN in `docs/SPEC-v0.md`.

use crate::coords::Coords;
use crate::hash::Hash;

/// A content-free address of some data: how to regenerate it, plus the exact
/// residual surprise needed to make the regeneration bit-perfect.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Address {
    /// Identity of the predictor: `Hash::of(generator.id())`.
    pub generator_hash: Hash,
    /// Where in generative space the data lives.
    pub coordinates: Coords,
    /// Hash of the residue blob in the store (the tsoin = the Shannon surprise).
    pub residue_hash: Hash,
    /// Length in bytes of the original data (== residue length). Needed so the
    /// generator knows how many bytes to predict on decode. Part of the address.
    pub len: u64,
}

impl Address {
    /// The address's own content hash — a stable id for journaling (a Merkle DAG
    /// node id). Canonical, fixed-layout encoding so it matches across machines.
    #[must_use]
    pub fn node_hash(&self) -> Hash {
        let mut buf = Vec::with_capacity(32 + 40 + 32 + 8);
        buf.extend_from_slice(self.generator_hash.as_bytes());
        buf.extend_from_slice(&self.coordinates.to_bytes());
        buf.extend_from_slice(self.residue_hash.as_bytes());
        buf.extend_from_slice(&self.len.to_le_bytes());
        Hash::of(&buf)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn node_hash_is_deterministic_and_field_sensitive() {
        let a = Address {
            generator_hash: Hash::of(b"g"),
            coordinates: Coords::seeded(7),
            residue_hash: Hash::of(b"r"),
            len: 10,
        };
        assert_eq!(a.node_hash(), a.node_hash());
        let mut b = a;
        b.len = 11;
        assert_ne!(a.node_hash(), b.node_hash());
    }
}
