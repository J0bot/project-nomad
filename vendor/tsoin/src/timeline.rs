//! Temporal tsoin recorder — a branchable Merkle timeline of states where each
//! frame stores only the **delta from the previous frame** (the surprise between
//! two moments). *Le B0XION = le git des états.*
//!
//! ## The idea
//!
//! A *tsoin* is an instant of reality. A timeline is a sequence of such instants.
//! Storing the timeline naively costs `sum(len(frame))`. But consecutive instants
//! are usually *almost the same* — the only thing worth storing is the **residue
//! between moments**: what changed.
//!
//! This module reuses the engine's residue primitive (byte-wise XOR against a
//! prediction, [`crate::codec`]) but makes the *predictor be the previous frame
//! itself*:
//!
//! ```text
//! frame[0]  -> predictor = Identity (all-zero)         => residue[0] = frame[0]
//! frame[t]  -> predictor = reconstruct(frame[t-1])     => residue[t] = frame[t] XOR pred
//! ```
//!
//! So the store holds `frame[0]` plus the deltas. For a slowly-evolving session
//! (a 64-byte buffer changing a few bytes per frame) the deltas are mostly zero
//! and the footprint is `~= sum of small deltas << sum of full frames`. For a
//! fast-changing session the deltas are large and the footprint is `~= naive` —
//! honest, no magic.
//!
//! ### Differing frame lengths
//!
//! Frames may differ in length. The predictor for `frame[t]` is the previous
//! reconstructed frame **resized to `len(frame[t])`**: truncated if the previous
//! frame was longer, zero-padded if it was shorter (see [`resize_predictor`]).
//! XOR is then length-preserving against `len(frame[t])`, and decode resizes the
//! same way, so reconstruction stays bit-exact for any length sequence.
//!
//! ## The Merkle timeline
//!
//! Each recorded frame becomes a [`TimelineNode`] linked to its parent:
//!
//! ```text
//! TimelineNode { parent: Option<NodeHash>, residue_hash, len, coords }
//! node_hash = BLAKE3( parent_hash(or 32 zeros) || residue_hash || len_le || coords )
//! ```
//!
//! A [`Timeline`] is just a **head `NodeHash`** plus a shared [`Recorder`] (which
//! owns the content-addressed blob [`Store`] and the node DAG). `record` appends a
//! node and returns the new head. Because nodes and residues are content-addressed
//! and parents are referenced by hash, **forking is free**: a fork is a second
//! head pointing at an existing node; shared past is never copied.
//!
//! ## xerboxion detector
//!
//! José's popup: if two instants are *exactly identical*, that is a **xerboxion**.
//! The recorder keeps `content_hash(full reconstructed frame) -> first node` and
//! flags any later frame whose full content hash equals an earlier one — and *only*
//! exact-identical content, never a near-miss (a single differing byte changes the
//! BLAKE3 content hash).

use crate::codec::zstd_len;
use crate::coords::Coords;
use crate::hash::Hash;
use crate::store::Store;
use std::collections::HashMap;

/// The 32-byte sentinel used in a node hash where a node has no parent (the
/// timeline root). Distinct from any real `Hash` only by intent; a real BLAKE3
/// digest colliding with all-zero is cryptographically impossible.
const ROOT_PARENT_SENTINEL: [u8; 32] = [0u8; 32];

/// One frame in the timeline: a Merkle DAG node storing the **delta** (residue)
/// from its parent frame, not the frame itself.
///
/// The frame's bytes are reconstructed by walking to the root and XOR-ing deltas
/// forward (see [`Recorder::replay`]). The node is content-addressed by
/// [`TimelineNode::node_hash`], so identical nodes (same parent, same delta, same
/// length, same coords) collapse to one — the Merkle/Git-of-states property.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimelineNode {
    /// Parent frame's node hash, or `None` for the root frame (`frame[0]`).
    pub parent: Option<Hash>,
    /// Hash of this frame's residue blob in the [`Store`] (the delta = surprise).
    pub residue_hash: Hash,
    /// Byte length of *this* frame (== residue length).
    pub len: u64,
    /// Coordinates / timestamp of this instant (the `t` axis is the natural
    /// timestamp; the whole `Coords` is hashed into the node id).
    pub coords: Coords,
}

impl TimelineNode {
    /// Canonical Merkle node id:
    /// `BLAKE3(parent || residue_hash || len_le || coords)`, where `parent` is the
    /// 32-byte parent hash or 32 zero bytes for the root. Fixed layout so the id is
    /// stable across machines and over time.
    #[must_use]
    pub fn node_hash(&self) -> Hash {
        let mut buf = Vec::with_capacity(32 + 32 + 8 + 40);
        match &self.parent {
            Some(p) => buf.extend_from_slice(p.as_bytes()),
            None => buf.extend_from_slice(&ROOT_PARENT_SENTINEL),
        }
        buf.extend_from_slice(self.residue_hash.as_bytes());
        buf.extend_from_slice(&self.len.to_le_bytes());
        buf.extend_from_slice(&self.coords.to_bytes());
        Hash::of(&buf)
    }
}

/// Resize a predictor (the previous reconstructed frame) to exactly `len` bytes:
/// truncate if longer, zero-pad if shorter. This is how frames of *different*
/// lengths are handled — documented in the module header. Zero-padding means a
/// frame that grows is predicted as "the old frame, then zeros", so the appended
/// region's residue equals the appended bytes verbatim (no free lunch on growth).
#[must_use]
pub fn resize_predictor(prev: &[u8], len: usize) -> Vec<u8> {
    let mut out = vec![0u8; len];
    let take = prev.len().min(len);
    out[..take].copy_from_slice(&prev[..take]);
    out
}

/// XOR two equal-length slices into a fresh vec.
fn xor(a: &[u8], b: &[u8]) -> Vec<u8> {
    debug_assert_eq!(a.len(), b.len());
    a.iter().zip(b).map(|(x, y)| x ^ y).collect()
}

/// A detected **xerboxion**: two instants with byte-for-byte identical content.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Xerboxion {
    /// Content hash of the (identical) reconstructed frame bytes.
    pub content_hash: Hash,
    /// The earlier node that first produced this exact content.
    pub first: Hash,
    /// The later node whose content repeats it (the one just recorded/scanned).
    pub repeat: Hash,
}

/// The shared recorder: owns the content-addressed blob [`Store`], the node DAG,
/// and the xerboxion content index. Multiple [`Timeline`] heads point into one
/// recorder, which is exactly what makes forks free (shared nodes/residues are
/// stored once).
#[derive(Debug, Default)]
pub struct Recorder {
    store: Store,
    /// node_hash -> node. Content-addressed: identical nodes collapse to one.
    nodes: HashMap<Hash, TimelineNode>,
    /// content_hash(full reconstructed frame) -> first node that produced it.
    seen_content: HashMap<Hash, Hash>,
    /// All xerboxions detected so far, in detection order.
    xerboxions: Vec<Xerboxion>,
}

impl Recorder {
    /// A fresh, empty recorder.
    #[must_use]
    pub fn new() -> Self {
        Recorder::default()
    }

    /// Read-only access to the underlying blob store (for `blob_count`, etc.).
    #[must_use]
    pub fn store(&self) -> &Store {
        &self.store
    }

    /// Number of distinct residue/delta blobs stored. Load-bearing for the
    /// fork-is-free test: fork + append exactly one new frame grows this by 1.
    #[must_use]
    pub fn blob_count(&self) -> usize {
        self.store.blob_count()
    }

    /// Raw stored footprint: the sum of the byte lengths of every distinct delta
    /// blob held in the store (content-addressed, so shared deltas counted once).
    ///
    /// Because XOR is length-preserving, a raw delta blob is the same length as
    /// its frame; the *win* of temporal residue is that the deltas are mostly
    /// **zeros** (sparse surprise), which a real compressor crushes — see
    /// [`Recorder::store_footprint_compressed`], the honest "real cost". This raw
    /// figure is reported too, for transparency (no hidden magic).
    #[must_use]
    pub fn store_footprint_raw(&self) -> usize {
        self.store.total_bytes()
    }

    /// Honest stored footprint: the sum, over every distinct delta blob, of its
    /// size **after zstd**. Temporal residue makes the deltas mostly zero for a
    /// slowly-evolving session, so this collapses far below the naive cost; for a
    /// fast-changing session the deltas are high-entropy and this stays `~= naive`
    /// (no free lunch). This is the number to compare against [`naive_cost`].
    ///
    /// We compress each delta blob independently (the store keys are independent
    /// blobs), matching how the engine elsewhere uses `zstd_len` as its stand-in
    /// for real cost / entropy. Note this charges a per-blob compressor framing
    /// overhead to every tiny delta; [`store_footprint_joint`] removes that to
    /// show the true *joint* surprise.
    ///
    /// [`naive_cost`]: Recorder::naive_cost
    /// [`store_footprint_joint`]: Recorder::store_footprint_joint
    #[must_use]
    pub fn store_footprint_compressed(&self) -> usize {
        self.store.iter_blobs().map(zstd_len).sum()
    }

    /// Joint honest footprint: every distinct delta blob concatenated and zstd'd
    /// **as one stream**. This removes the per-blob compressor framing overhead
    /// that [`store_footprint_compressed`] charges to each tiny delta, so it
    /// reflects the true joint surprise of the whole timeline — the cleanest
    /// "sum of small deltas" figure. Still honest: high-entropy deltas do not
    /// compress, so a fast session stays `~= naive` here too.
    ///
    /// Blobs are concatenated in a **deterministic** order (sorted by content
    /// hash) so the figure is reproducible across runs and machines — `HashMap`
    /// iteration order is otherwise randomized per run.
    ///
    /// [`store_footprint_compressed`]: Recorder::store_footprint_compressed
    #[must_use]
    pub fn store_footprint_joint(&self) -> usize {
        let mut all = Vec::with_capacity(self.store.total_bytes());
        for b in self.store.blobs_sorted_by_hash() {
            all.extend_from_slice(b);
        }
        zstd_len(&all)
    }

    /// Number of distinct nodes (frames) in the DAG.
    #[must_use]
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Look up a node by its hash.
    #[must_use]
    pub fn node(&self, h: &Hash) -> Option<TimelineNode> {
        self.nodes.get(h).copied()
    }

    /// Every distinct node in the DAG (hash, node). Order is unspecified; used to
    /// serialize the recorder for out-of-process persistence (e.g. the browser's
    /// IndexedDB). Pair with [`Recorder::blobs_with_hashes`] to capture the full
    /// state, and [`Recorder::import_node`] to rehydrate it.
    #[must_use]
    pub fn nodes_snapshot(&self) -> Vec<(Hash, TimelineNode)> {
        self.nodes.iter().map(|(h, n)| (*h, *n)).collect()
    }

    /// Every distinct residue/delta blob with its content hash, for serialization.
    #[must_use]
    pub fn blobs_with_hashes(&self) -> Vec<(Hash, Vec<u8>)> {
        self.store
            .blobs_sorted_by_hash_keyed()
            .into_iter()
            .map(|(h, b)| (h, b.to_vec()))
            .collect()
    }

    /// Rehydrate one residue blob into the store (idempotent, content-addressed).
    /// Used when loading a persisted recorder; the hash is re-derived from the
    /// bytes, so a tampered blob simply lands under its true hash.
    pub fn import_blob(&mut self, bytes: &[u8]) -> Hash {
        self.store.put(bytes)
    }

    /// Rehydrate one Merkle node into the DAG and update the xerboxion index as if
    /// it had been freshly recorded. Nodes MUST be imported in topological order
    /// (every parent before its children) so that [`Recorder::replay`] can
    /// reconstruct the node's full content to run the exact-repeat detector — the
    /// same detection the live [`Recorder::record`] path performs. Idempotent on
    /// the node id; returns any [`Xerboxion`] this node triggers.
    ///
    /// The companion to [`Recorder::nodes_snapshot`] / [`Recorder::import_blob`]:
    /// import all blobs, then import all nodes parent-first, and the recorder is
    /// byte-for-byte equivalent to the one that produced the snapshot (same DAG,
    /// same store, same xerboxions).
    pub fn import_node(&mut self, node: TimelineNode) -> Option<Xerboxion> {
        let node_hash = node.node_hash();
        self.nodes.entry(node_hash).or_insert(node);
        // Reconstruct the full frame to run exact-repeat detection, mirroring the
        // live record() path. Requires parents + residue already present.
        let frame = self.try_replay(&node_hash)?;
        let content_hash = Hash::of(&frame);
        match self.seen_content.get(&content_hash) {
            Some(&first) if first != node_hash => {
                let x = Xerboxion {
                    content_hash,
                    first,
                    repeat: node_hash,
                };
                self.xerboxions.push(x);
                Some(x)
            }
            Some(_) => None,
            None => {
                self.seen_content.insert(content_hash, node_hash);
                None
            }
        }
    }

    /// All xerboxions detected so far (pairs of exactly-identical instants).
    #[must_use]
    pub fn xerboxions(&self) -> &[Xerboxion] {
        &self.xerboxions
    }

    /// Running count of xerboxions detected (José's popup counter).
    #[must_use]
    pub fn xerboxion_count(&self) -> usize {
        self.xerboxions.len()
    }

    /// Record a new frame as a child of `parent` (or as a root if `parent` is
    /// `None`), at `coords`. Stores **only the delta** from the parent's
    /// reconstructed frame, links the node, and runs the xerboxion detector.
    ///
    /// Returns the new node's hash (the new head) and, if this frame's full
    /// content exactly repeats an earlier frame's, the detected [`Xerboxion`].
    pub fn record(
        &mut self,
        parent: Option<Hash>,
        frame: &[u8],
        coords: Coords,
    ) -> (Hash, Option<Xerboxion>) {
        // 1. Build the prediction: previous reconstructed frame resized to len,
        //    or all-zero (Identity) for the root.
        let prediction = match parent {
            Some(p) => {
                let prev = self.replay(&p);
                resize_predictor(&prev, frame.len())
            }
            None => vec![0u8; frame.len()],
        };
        // 2. residue = frame XOR prediction; store the delta.
        let residue = xor(frame, &prediction);
        let residue_hash = self.store.put(&residue);

        // 3. Link the Merkle node.
        let node = TimelineNode {
            parent,
            residue_hash,
            len: frame.len() as u64,
            coords,
        };
        let node_hash = node.node_hash();
        self.nodes.entry(node_hash).or_insert(node);

        // 4. xerboxion detection on the FULL content (not the delta): exact repeat?
        let content_hash = Hash::of(frame);
        let xerb = match self.seen_content.get(&content_hash) {
            Some(&first) if first != node_hash => {
                let x = Xerboxion {
                    content_hash,
                    first,
                    repeat: node_hash,
                };
                self.xerboxions.push(x);
                Some(x)
            }
            Some(_) => None, // same node re-recorded; not a new instant
            None => {
                self.seen_content.insert(content_hash, node_hash);
                None
            }
        };

        (node_hash, xerb)
    }

    /// Reconstruct the exact bytes of the frame at `node_hash` by walking parents
    /// to the root and XOR-ing the deltas forward. **Bit-for-bit exact** for any
    /// node in any branch.
    ///
    /// # Panics
    /// Panics if `node_hash` (or any ancestor / residue) is missing — that would
    /// be a corrupted DAG, not a normal condition. Use [`Recorder::try_replay`]
    /// for a fallible version.
    #[must_use]
    pub fn replay(&self, node_hash: &Hash) -> Vec<u8> {
        self.try_replay(node_hash)
            .expect("replay: node, ancestor, or residue missing from recorder")
    }

    /// Fallible [`Recorder::replay`]: returns `None` if the node, an ancestor, or
    /// a residue blob is absent.
    #[must_use]
    pub fn try_replay(&self, node_hash: &Hash) -> Option<Vec<u8>> {
        // Collect the chain root..=node by following parents.
        let mut chain: Vec<TimelineNode> = Vec::new();
        let mut cur = Some(*node_hash);
        while let Some(h) = cur {
            let node = *self.nodes.get(&h)?;
            cur = node.parent;
            chain.push(node);
        }
        chain.reverse(); // now root first

        // Fold deltas forward: frame[t] = residue[t] XOR resize(frame[t-1], len_t).
        let mut frame: Vec<u8> = Vec::new();
        for node in chain {
            let residue = self.store.get(&node.residue_hash)?;
            if residue.len() as u64 != node.len {
                return None; // corrupted / truncated delta
            }
            let prediction = if frame.is_empty() && node.parent.is_none() {
                // Root: predictor is all-zero, so frame == residue.
                vec![0u8; node.len as usize]
            } else {
                resize_predictor(&frame, node.len as usize)
            };
            frame = xor(residue, &prediction);
        }
        Some(frame)
    }

    /// Reconstruct EVERY frame from the root up to `head`, oldest first. The full
    /// timeline as a sequence of instants.
    #[must_use]
    pub fn replay_all(&self, head: &Hash) -> Vec<Vec<u8>> {
        // Walk to root collecting node hashes, then replay incrementally.
        let mut chain: Vec<TimelineNode> = Vec::new();
        let mut cur = Some(*head);
        while let Some(h) = cur {
            let node = self.nodes.get(&h).copied().expect("node present");
            cur = node.parent;
            chain.push(node);
        }
        chain.reverse();

        let mut frames: Vec<Vec<u8>> = Vec::with_capacity(chain.len());
        let mut frame: Vec<u8> = Vec::new();
        for node in chain {
            let residue = self.store.get(&node.residue_hash).expect("residue present");
            let prediction = if node.parent.is_none() {
                vec![0u8; node.len as usize]
            } else {
                resize_predictor(&frame, node.len as usize)
            };
            frame = xor(residue, &prediction);
            frames.push(frame.clone());
        }
        frames
    }

    /// The naive cost of a timeline ending at `head`: the sum of the full byte
    /// lengths of every frame (what you'd pay storing each instant in full).
    #[must_use]
    pub fn naive_cost(&self, head: &Hash) -> usize {
        let mut total = 0usize;
        let mut cur = Some(*head);
        while let Some(h) = cur {
            let node = self.nodes.get(&h).copied().expect("node present");
            total += node.len as usize;
            cur = node.parent;
        }
        total
    }
}

/// A single timeline: a **head node hash** into a shared [`Recorder`].
///
/// The timeline owns no data — only the head. Forking is therefore trivially free
/// (clone a hash). `record` mutates the shared recorder and advances this head;
/// `fork_at` produces a second timeline pointing at any existing node.
#[derive(Debug, Clone, Copy)]
pub struct Timeline {
    head: Option<Hash>,
}

impl Default for Timeline {
    fn default() -> Self {
        Timeline::new()
    }
}

impl Timeline {
    /// A new, empty timeline (no frames yet).
    #[must_use]
    pub fn new() -> Self {
        Timeline { head: None }
    }

    /// A timeline whose head is an existing node — the fork constructor.
    #[must_use]
    pub fn at(node: Hash) -> Self {
        Timeline { head: Some(node) }
    }

    /// The current head node hash, if any frame has been recorded.
    #[must_use]
    pub fn head(&self) -> Option<Hash> {
        self.head
    }

    /// Record `frame` at `coords` into `rec`, advancing this timeline's head.
    /// Returns the detected [`Xerboxion`] if this frame exactly repeats an earlier
    /// instant. Stores only the delta from the current head's frame.
    pub fn record(
        &mut self,
        rec: &mut Recorder,
        frame: &[u8],
        coords: Coords,
    ) -> Option<Xerboxion> {
        let (new_head, xerb) = rec.record(self.head, frame, coords);
        self.head = Some(new_head);
        xerb
    }

    /// FREE FORK: a new timeline sharing this one's entire past, diverging on the
    /// next `record`. No nodes or residues are copied — the new timeline is just a
    /// second head into the same recorder. Fork at the current head; use
    /// [`Timeline::at`] to fork at an arbitrary past node.
    #[must_use]
    pub fn fork(&self) -> Timeline {
        *self
    }

    /// Reconstruct the current head frame (bit-exact). Panics if empty.
    #[must_use]
    pub fn current(&self, rec: &Recorder) -> Vec<u8> {
        rec.replay(&self.head.expect("empty timeline has no current frame"))
    }

    /// Reconstruct the full sequence of frames up to the head (oldest first).
    #[must_use]
    pub fn frames(&self, rec: &Recorder) -> Vec<Vec<u8>> {
        match self.head {
            Some(h) => rec.replay_all(&h),
            None => Vec::new(),
        }
    }

    /// Naive cost (sum of full frame lengths) for this timeline's history.
    #[must_use]
    pub fn naive_cost(&self, rec: &Recorder) -> usize {
        match self.head {
            Some(h) => rec.naive_cost(&h),
            None => 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn root_residue_is_the_full_first_frame() {
        let mut rec = Recorder::new();
        let mut tl = Timeline::new();
        let f0 = b"the very first instant of reality";
        tl.record(&mut rec, f0, Coords::default());
        // Root stored against Identity => residue == full first frame.
        let node = rec.node(&tl.head().unwrap()).unwrap();
        assert_eq!(rec.store().get(&node.residue_hash).unwrap(), f0);
    }

    #[test]
    fn replay_is_bit_exact_for_every_frame() {
        let mut rec = Recorder::new();
        let mut tl = Timeline::new();
        let frames: Vec<Vec<u8>> = (0u8..20)
            .map(|i| {
                let mut v = vec![0x10u8; 40];
                v[i as usize % 40] = i; // change one byte per frame
                v
            })
            .collect();
        let mut heads = Vec::new();
        for (t, f) in frames.iter().enumerate() {
            tl.record(
                &mut rec,
                f,
                Coords {
                    t: t as u64,
                    ..Default::default()
                },
            );
            heads.push(tl.head().unwrap());
        }
        // Every individual node replays to exactly its frame.
        for (f, h) in frames.iter().zip(&heads) {
            assert_eq!(&rec.replay(h), f);
        }
        // replay_all yields the whole sequence.
        assert_eq!(tl.frames(&rec), frames);
    }

    #[test]
    fn differing_lengths_roundtrip() {
        let mut rec = Recorder::new();
        let mut tl = Timeline::new();
        let frames: Vec<Vec<u8>> = vec![
            b"short".to_vec(),
            b"a much longer frame than before".to_vec(),
            b"tiny".to_vec(),
            b"grown again to a medium length frame".to_vec(),
        ];
        for f in &frames {
            tl.record(&mut rec, f, Coords::default());
        }
        assert_eq!(tl.frames(&rec), frames);
    }

    #[test]
    fn fork_then_append_adds_only_one_delta() {
        let mut rec = Recorder::new();
        let mut main = Timeline::new();
        for i in 0u8..10 {
            let mut f = vec![0x20u8; 64];
            f[0] = i;
            main.record(&mut rec, &f, Coords::default());
        }
        let before = rec.blob_count();
        // Fork at head: copies nothing.
        let mut branch = main.fork();
        assert_eq!(rec.blob_count(), before, "fork copied no data");
        // Append one genuinely new frame: store grows by exactly 1 delta.
        let mut nf = vec![0x20u8; 64];
        nf[63] = 0xAB; // a change not seen before
        branch.record(&mut rec, &nf, Coords::default());
        assert_eq!(rec.blob_count(), before + 1);
        // Main untouched; its head still replays.
        let _ = main.current(&rec);
    }

    #[test]
    fn xerboxion_fires_on_exact_repeat_only() {
        let mut rec = Recorder::new();
        let mut tl = Timeline::new();
        let a = b"instant A".to_vec();
        let mut b = a.clone();
        b[0] ^= 0x01; // one byte different

        assert!(tl.record(&mut rec, &a, Coords::default()).is_none());
        // A near-miss must NOT fire.
        assert!(tl.record(&mut rec, &b, Coords::default()).is_none());
        assert_eq!(rec.xerboxion_count(), 0);
        // An exact repeat of A MUST fire.
        let x = tl.record(&mut rec, &a, Coords::default());
        assert!(x.is_some(), "exact repeat must flag a xerboxion");
        assert_eq!(rec.xerboxion_count(), 1);
        assert_eq!(x.unwrap().content_hash, Hash::of(&a));
    }

    #[test]
    fn slow_session_store_far_below_naive() {
        let mut rec = Recorder::new();
        let mut tl = Timeline::new();
        let mut buf = vec![0x42u8; 64];
        for t in 0..50u64 {
            buf[(t as usize) % 64] = buf[(t as usize) % 64].wrapping_add(1);
            tl.record(
                &mut rec,
                &buf,
                Coords {
                    t,
                    ..Default::default()
                },
            );
        }
        let naive = tl.naive_cost(&rec); // 50 * 64 = 3200
        let store = rec.store_footprint_compressed();
        assert_eq!(naive, 50 * 64);
        // Store holds frame[0] (64) plus 49 sparse (mostly-zero) deltas; after a
        // real compressor the honest footprint is far below naive.
        assert!(
            store < naive / 2,
            "slow session: store {store} vs naive {naive}"
        );
    }

    #[test]
    fn node_hash_links_parent() {
        // Two roots with the same content but recorded as root vs as a child must
        // get different node hashes (parent is part of the id).
        let mut rec = Recorder::new();
        let h_root = rec.record(None, b"x", Coords::default()).0;
        let h_child = rec.record(Some(h_root), b"x", Coords::default()).0;
        assert_ne!(h_root, h_child);
        let child = rec.node(&h_child).unwrap();
        assert_eq!(child.parent, Some(h_root));
    }
}
