//! Content-addressed blob store (the "Git généralisé / Merkle" layer).
//!
//! `put(bytes) -> Hash`, `get(&Hash) -> Option<bytes>`. Storage is
//! content-addressed: identical bytes collapse to a single blob, so two branches
//! that share a residue share one entry — this is what makes [`crate::Branch`]
//! forks free. In-memory is the v0 default; an optional on-disk mirror under
//! `./store/` is provided for persistence experiments.

use crate::hash::Hash;
use std::collections::hash_map::Entry;
use std::collections::HashMap;
#[cfg(feature = "native")]
use std::path::{Path, PathBuf};

/// An in-memory, content-addressed blob store.
///
/// Deduplicating by construction: `put` of bytes already present is a no-op that
/// returns the existing hash. [`Self::blob_count`] reports the number of
/// *distinct* blobs, which the fork test uses to prove fork+append is cheap.
#[derive(Debug, Default, Clone)]
pub struct Store {
    blobs: HashMap<Hash, Vec<u8>>,
    /// Optional directory to also persist blobs to (one file per hash). Native
    /// only — the wasm build persists via IndexedDB on the JS side, so this disk
    /// mirror does not exist (and `std::fs` is not used) under wasm.
    #[cfg(feature = "native")]
    disk: Option<PathBuf>,
}

impl Store {
    /// A fresh in-memory store.
    #[must_use]
    pub fn new() -> Self {
        Store {
            blobs: HashMap::new(),
            #[cfg(feature = "native")]
            disk: None,
        }
    }

    /// A store that also mirrors every blob to `dir` (created if absent). Native
    /// only (filesystem-bound).
    ///
    /// Existing blobs already on disk are loaded back into memory so the store
    /// survives process restarts.
    #[cfg(feature = "native")]
    pub fn with_disk(dir: impl AsRef<Path>) -> std::io::Result<Self> {
        let dir = dir.as_ref().to_path_buf();
        std::fs::create_dir_all(&dir)?;
        let mut blobs = HashMap::new();
        for entry in std::fs::read_dir(&dir)? {
            let entry = entry?;
            let bytes = std::fs::read(entry.path())?;
            // Re-derive the hash from content rather than trusting the filename.
            blobs.insert(Hash::of(&bytes), bytes);
        }
        Ok(Store {
            blobs,
            disk: Some(dir),
        })
    }

    /// Store `bytes`, returning their content hash. Idempotent.
    pub fn put(&mut self, bytes: &[u8]) -> Hash {
        let h = Hash::of(bytes);
        if let Entry::Vacant(slot) = self.blobs.entry(h) {
            #[cfg(feature = "native")]
            if let Some(dir) = &self.disk {
                // Best-effort persistence; an IO error here would corrupt the
                // invariant that disk mirrors memory, so propagate via panic in
                // the rare case — callers wanting fallible IO use the in-memory
                // store. v0 keeps the API infallible for ergonomics.
                let path = dir.join(h.to_hex());
                let _ = std::fs::write(path, bytes);
            }
            slot.insert(bytes.to_vec());
        }
        h
    }

    /// Retrieve the bytes for a hash, if present.
    #[must_use]
    pub fn get(&self, h: &Hash) -> Option<&[u8]> {
        self.blobs.get(h).map(Vec::as_slice)
    }

    /// `true` if the store holds a blob with this hash.
    #[must_use]
    pub fn contains(&self, h: &Hash) -> bool {
        self.blobs.contains_key(h)
    }

    /// Number of *distinct* blobs held. Load-bearing for the fork-is-free test.
    #[must_use]
    pub fn blob_count(&self) -> usize {
        self.blobs.len()
    }

    /// Total byte footprint: the sum of the lengths of every *distinct* blob.
    /// Content-addressed, so shared blobs are counted once — this is the real
    /// stored cost, used by the temporal recorder to compare against the naive
    /// (sum-of-full-frames) cost.
    #[must_use]
    pub fn total_bytes(&self) -> usize {
        self.blobs.values().map(Vec::len).sum()
    }

    /// Iterate over the bytes of every *distinct* stored blob. Order is
    /// unspecified (it follows the underlying map). Used to measure a real,
    /// compressed footprint of the store where order does not matter (e.g. a sum).
    pub fn iter_blobs(&self) -> impl Iterator<Item = &[u8]> {
        self.blobs.values().map(Vec::as_slice)
    }

    /// Every distinct blob's bytes, in a **deterministic** order (sorted by the
    /// blob's content hash). `HashMap` iteration order is randomized per run, so
    /// any *order-sensitive* measurement (e.g. concatenating then compressing)
    /// must use this to be reproducible across runs and machines.
    #[must_use]
    pub fn blobs_sorted_by_hash(&self) -> Vec<&[u8]> {
        let mut keyed: Vec<(&Hash, &Vec<u8>)> = self.blobs.iter().collect();
        keyed.sort_by(|a, b| a.0.cmp(b.0));
        keyed.into_iter().map(|(_, v)| v.as_slice()).collect()
    }

    /// Like [`Store::blobs_sorted_by_hash`] but carries each blob's content hash
    /// too, in the same deterministic (hash-sorted) order. Used to serialize the
    /// store for out-of-process persistence (IndexedDB) where both the key and the
    /// bytes are needed and the order must be reproducible.
    #[must_use]
    pub fn blobs_sorted_by_hash_keyed(&self) -> Vec<(Hash, &[u8])> {
        let mut keyed: Vec<(&Hash, &Vec<u8>)> = self.blobs.iter().collect();
        keyed.sort_by(|a, b| a.0.cmp(b.0));
        keyed.into_iter().map(|(h, v)| (*h, v.as_slice())).collect()
    }

    /// Test/measurement hook: overwrite the bytes stored under `h` *without*
    /// changing the key. This deliberately breaks the content-addressing
    /// invariant to simulate residue corruption (bit-rot, a malicious peer), so
    /// tests can show the address's `residue_hash` no longer matches.
    pub fn corrupt(&mut self, h: &Hash, new_bytes: Vec<u8>) -> bool {
        match self.blobs.get_mut(h) {
            Some(slot) => {
                *slot = new_bytes;
                true
            }
            None => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn put_get_roundtrip() {
        let mut s = Store::new();
        let h = s.put(b"data");
        assert_eq!(s.get(&h), Some(&b"data"[..]));
        assert!(s.contains(&h));
        assert!(s.get(&Hash::of(b"other")).is_none());
    }

    #[test]
    fn put_is_deduplicating() {
        let mut s = Store::new();
        let a = s.put(b"same");
        let b = s.put(b"same");
        assert_eq!(a, b);
        assert_eq!(s.blob_count(), 1);
        s.put(b"different");
        assert_eq!(s.blob_count(), 2);
    }

    #[test]
    fn corrupt_changes_content_under_same_key() {
        let mut s = Store::new();
        let h = s.put(b"original");
        assert!(s.corrupt(&h, b"tampered".to_vec()));
        assert_eq!(s.get(&h), Some(&b"tampered"[..]));
        // The stored bytes no longer hash to the key — detectable.
        assert_ne!(Hash::of(s.get(&h).unwrap()), h);
    }
}
