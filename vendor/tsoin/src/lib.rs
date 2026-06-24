//! # tsoin — Adressage Génératif engine v0
//!
//! Store **addresses, not content**. A *tsoin* is the **residue**: the exact
//! bit-level difference between what a pure deterministic generator predicts and
//! the real data — the Shannon surprise, the only thing worth storing.
//!
//! ```text
//! address = (generator_hash, coordinates, residue_hash)
//! read    = run generator(coordinates) then apply residue
//! write   = note the address
//! fork    = free (branches share all past + all unvisited space)
//! ```
//!
//! Two limit cases fall out for free:
//! - generator = [`Identity`] (predicts zeros) => residue == data == classic storage;
//! - a perfect procedural generator => residue empty == pure generation.
//!
//! ## Layers (synthesis §4)
//! - [`store`] — content-addressed blob store (BLAKE3). *Git généralisé / Merkle.*
//! - [`generator`] — pure deterministic predictors (the "software CFC"). WASM is
//!   the long-term ideal; v0 uses Rust generators behind the [`Generator`] trait.
//! - [`address`] — the triplet, the NEW primitive, proposed FROZEN in `docs/SPEC-v0.md`.
//! - [`codec`] — encode/decode (bit-exact) and `encode_best` (cheapest generator).
//! - [`model`] — the **text/byte domain generator**: a deterministic adaptive
//!   context-mixing model over a from-scratch binary range coder. The model *is*
//!   the knowledge; the entropy-coded residue is the surprise. Collapses real
//!   text/code below `zstd -19`; honest (`~= raw`) on random.
//! - [`model_codec`] — the model wired in as a codec path producing an [`Address`]
//!   (generator_hash = model id, residue = coded blob), a candidate in `encode_best`.
//! - [`branch`] — journal + free forks.
//! - [`timeline`] — the temporal tsoin recorder: a branchable Merkle timeline of
//!   states where each frame stores only the delta from the previous frame (the
//!   surprise between moments). *Le B0XION = le git des états.*
//! - [`measure`] — honest residue measurements.
//!
//! ## Quick start
//! ```
//! use tsoin::{Store, Registry, Coords, generator::Identity, codec};
//!
//! let mut store = Store::new();
//! let registry = Registry::with_defaults();
//! let data = b"hello tsoin";
//!
//! let addr = codec::encode(data, &Identity, Coords::origin(), &mut store);
//! let back = codec::decode(&addr, &registry, &store).unwrap();
//! assert_eq!(&back, data); // bit-for-bit
//! ```

pub mod address;
pub mod branch;
pub mod codec;
pub mod coords;
pub mod generator;
pub mod hash;
pub mod measure;
pub mod model;
pub mod model_codec;
/// On-disk `.tsoin/` persistence for the native CLI. Filesystem-bound, so it is
/// only built for the native target (`native` feature); the wasm app persists to
/// IndexedDB from the JS side instead.
#[cfg(feature = "native")]
pub mod persist;
pub mod registry;
pub mod store;
pub mod timeline;

pub use address::Address;
pub use branch::Branch;
pub use coords::Coords;
pub use generator::{
    default_generators, Constant, Generator, Gradient2D, Identity, LinearPredictor,
};
pub use hash::Hash;
pub use registry::Registry;
pub use store::Store;
pub use timeline::{Recorder, Timeline, TimelineNode, Xerboxion};
