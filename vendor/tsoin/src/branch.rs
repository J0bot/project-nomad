//! Branch / journal — an ordered history of addresses, with FREE forks.
//!
//! A [`Branch`] is an ordered list of [`Address`]es (states). It is the
//! "journal + branches" layer (synthesis §4 layer 3). The key property:
//!
//! **Fork is free.** `fork()` clones only the list of addresses (each address is
//! `Copy`, a few hashes — *not* the data, which lives once in the store). Two
//! branches that share a prefix share those store blobs by reference: the store
//! is content-addressed, so the bytes exist once. Appending a new state to one
//! branch only `put`s the *new* residue blob; it never touches the shared past.
//!
//! Because all *unvisited* generative space is implicit (any coords you have not
//! written a residue for are simply regenerated on demand), forks also share all
//! the future they have not yet explored — exactly the synthesis claim.
//!
//! [`Store::blob_count`] makes the freeness *measurable*: fork a branch with N
//! states, append 1, and the store grows by exactly the new state's residue
//! blob (1, or 0 if that residue already existed by content-addressing).

use crate::address::Address;
use crate::hash::Hash;

/// An ordered history of states, each a generative [`Address`].
#[derive(Debug, Clone, Default)]
pub struct Branch {
    states: Vec<Address>,
}

impl Branch {
    /// An empty branch.
    #[must_use]
    pub fn new() -> Self {
        Branch { states: Vec::new() }
    }

    /// Append a state (a freshly encoded address). The caller has already
    /// `put` the residue into the store via the codec, so this only records the
    /// pointer — O(1), no data copied.
    pub fn append(&mut self, address: Address) {
        self.states.push(address);
    }

    /// Number of states in this branch's history.
    #[must_use]
    pub fn len(&self) -> usize {
        self.states.len()
    }

    /// Whether the branch has no states.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.states.is_empty()
    }

    /// The states, oldest first.
    #[must_use]
    pub fn states(&self) -> &[Address] {
        &self.states
    }

    /// The most recent state, if any.
    #[must_use]
    pub fn head(&self) -> Option<&Address> {
        self.states.last()
    }

    /// FORK: a new branch sharing all of this branch's history.
    ///
    /// Only the vector of (Copy) addresses is cloned — no residue bytes are
    /// duplicated; both branches resolve the same blobs from the shared store.
    /// This is the free fork.
    #[must_use]
    pub fn fork(&self) -> Branch {
        Branch {
            states: self.states.clone(),
        }
    }

    /// A Merkle-style digest of the whole branch: fold the node hashes of every
    /// state in order. Two branches with identical histories share this id; a
    /// fork+append changes only the forked branch's id. (Simple linear fold for
    /// v0; a full Merkle DAG with parent links is future work, synthesis §4.)
    #[must_use]
    pub fn merkle_root(&self) -> Hash {
        let mut buf = Vec::with_capacity(self.states.len() * 32);
        for s in &self.states {
            buf.extend_from_slice(s.node_hash().as_bytes());
        }
        Hash::of(&buf)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codec::encode;
    use crate::coords::Coords;
    use crate::generator::Identity;
    use crate::store::Store;

    fn state(data: &[u8], store: &mut Store) -> Address {
        encode(data, &Identity, Coords::origin(), store)
    }

    #[test]
    fn fork_then_append_adds_only_one_residue() {
        let mut store = Store::new();
        let mut main = Branch::new();
        for i in 0u8..5 {
            main.append(state(&[i; 16], &mut store));
        }
        let before = store.blob_count();
        assert_eq!(main.len(), 5);

        // Fork shares the past; the store does not grow.
        let mut feature = main.fork();
        assert_eq!(store.blob_count(), before, "fork copied no data");
        assert_eq!(feature.len(), 5);

        // Append one brand-new state: store grows by exactly 1 residue blob.
        feature.append(state(b"a genuinely new state!!", &mut store));
        assert_eq!(store.blob_count(), before + 1);
        // Main is untouched.
        assert_eq!(main.len(), 5);
        assert_eq!(feature.len(), 6);
    }

    #[test]
    fn merkle_root_distinguishes_diverged_branches() {
        let mut store = Store::new();
        let mut main = Branch::new();
        main.append(state(b"shared", &mut store));
        let mut a = main.fork();
        let mut b = main.fork();
        assert_eq!(a.merkle_root(), b.merkle_root());
        a.append(state(b"left", &mut store));
        b.append(state(b"right", &mut store));
        assert_ne!(a.merkle_root(), b.merkle_root());
    }
}
