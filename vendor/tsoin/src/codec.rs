//! The codec — encode data to an address, decode an address back to data.
//!
//! Residue scheme: **byte-wise XOR**. `residue = data XOR prediction` (both
//! truncated/extended to `data.len()` — the generator always yields exactly
//! `len` bytes). XOR is exact, reversible, and symmetric: `data = prediction XOR
//! residue`. For data that matches the generator the residue is all-zero (which
//! a general compressor crushes to ~nothing); for random data the residue is
//! statistically random (no free lunch).
//!
//! `encode_best` chooses the generator whose residue is *smallest after zstd* —
//! i.e. the lowest-entropy residue = the least surprise = the cheapest generator.

use crate::address::Address;
use crate::coords::Coords;
use crate::generator::Generator;
use crate::registry::Registry;
use crate::store::Store;

/// Errors from decoding.
#[derive(Debug, PartialEq, Eq)]
pub enum DecodeError {
    /// The address's `generator_hash` is not in the registry.
    UnknownGenerator,
    /// The residue blob named by `residue_hash` is not in the store.
    MissingResidue,
    /// The stored residue blob's length disagrees with `address.len` — the
    /// residue was corrupted (or truncated). Detectable, not silently decoded.
    ResidueLengthMismatch { expected: u64, found: usize },
}

impl std::fmt::Display for DecodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DecodeError::UnknownGenerator => write!(f, "generator hash not in registry"),
            DecodeError::MissingResidue => write!(f, "residue blob not in store"),
            DecodeError::ResidueLengthMismatch { expected, found } => {
                write!(
                    f,
                    "residue length mismatch: expected {expected}, found {found}"
                )
            }
        }
    }
}

impl std::error::Error for DecodeError {}

/// XOR two equal-length byte slices into a fresh vec.
fn xor(a: &[u8], b: &[u8]) -> Vec<u8> {
    debug_assert_eq!(a.len(), b.len());
    a.iter().zip(b).map(|(x, y)| x ^ y).collect()
}

/// Encode `data` against `generator` at `coords`, storing the residue in `store`
/// and returning the [`Address`]. The generator must be resolvable on decode —
/// register it in the [`Registry`] used by [`decode`].
pub fn encode(
    data: &[u8],
    generator: &dyn Generator,
    coords: Coords,
    store: &mut Store,
) -> Address {
    let prediction = generator.generate(&coords, data.len());
    debug_assert_eq!(
        prediction.len(),
        data.len(),
        "generator must yield exactly len bytes"
    );
    let residue = xor(data, &prediction);
    let residue_hash = store.put(&residue);
    Address {
        generator_hash: generator.generator_hash(),
        coordinates: coords,
        residue_hash,
        len: data.len() as u64,
    }
}

/// Decode an [`Address`] back to the original bytes. Bit-for-bit exact.
pub fn decode(
    address: &Address,
    registry: &Registry,
    store: &Store,
) -> Result<Vec<u8>, DecodeError> {
    let generator = registry
        .get(&address.generator_hash)
        .ok_or(DecodeError::UnknownGenerator)?;
    let residue = store
        .get(&address.residue_hash)
        .ok_or(DecodeError::MissingResidue)?;
    if residue.len() as u64 != address.len {
        return Err(DecodeError::ResidueLengthMismatch {
            expected: address.len,
            found: residue.len(),
        });
    }
    let prediction = generator.generate(&address.coordinates, address.len as usize);
    Ok(xor(&prediction, residue))
}

/// Size of `bytes` after the entropy/real-cost stand-in. Level 19 zstd on native;
/// the engine's own pure-Rust model codec on wasm (see [`zstd_len_level`]).
#[must_use]
pub fn zstd_len(bytes: &[u8]) -> usize {
    zstd_len_level(bytes, 19)
}

/// Size of `bytes` after the entropy/real-cost stand-in at `level`.
///
/// On native builds this is real zstd at `level` (a strong, deterministic
/// default). zstd does **not** build on `wasm32`, so when the `zstd` feature is
/// off we substitute the engine's own pure-Rust adaptive **model codec**
/// ([`crate::model::encode`]) as the honest stand-in: it is a real entropy coder,
/// deterministic, and collapses low-entropy data while staying `~= raw` on random
/// data — the exact "no free lunch" property zstd gives us here. `level` is
/// ignored in that path (the model has no level knob). The winner-selection
/// ordering in [`encode_best`] stays an honest, monotone cost either way.
#[cfg(feature = "zstd")]
#[must_use]
pub fn zstd_len_level(bytes: &[u8], level: i32) -> usize {
    zstd::encode_all(bytes, level)
        .map(|v| v.len())
        .unwrap_or(bytes.len())
}

/// wasm / no-zstd fallback: the engine's pure-Rust model codec as the honest
/// "compressed size" stand-in. See the native variant for rationale.
#[cfg(not(feature = "zstd"))]
#[must_use]
pub fn zstd_len_level(bytes: &[u8], _level: i32) -> usize {
    if bytes.is_empty() {
        return 0;
    }
    // The model coder is a real entropy coder; clamp to raw so this is a true
    // upper bound on a store's cost (never reports a win that isn't genuinely
    // there — random data stays at raw).
    crate::model::encode(bytes).len().min(bytes.len())
}

/// Try every generator (and the adaptive **model codec**) and keep the one whose
/// residue is *cheapest in the store* — the lowest-surprise generator.
///
/// This is the engine "finding the cheapest generator / least surprise". Two
/// kinds of candidate compete on a single, fair cost axis:
///
/// * **Pure generators** ([`Generator`]): residue = `data XOR generator(coords)`,
///   stored verbatim. Its real cost is `zstd(residue)` (a store would compress
///   it), so that is its comparison key.
/// * **The model codec** ([`crate::model`]): residue = the entropy-coded stream
///   of the adaptive model's prediction errors, stored verbatim. It is *already*
///   entropy-coded, so its comparison key is simply its own length (re-zstd-ing
///   an entropy-coded stream cannot shrink it). Its `generator_hash` is the model
///   id; [`crate::model_codec::decode`] routes such an address back through the
///   model on decode.
///
/// Including the model makes real text/code pick the model (its residue collapses
/// far below the raw bytes), while gradient/constant data still picks a cheap
/// pure generator (whose residue is all-zero). Ties broken by raw residue length,
/// then by generator id, for determinism. The winning residue blob is left in the
/// store. Returns the winning address plus a per-candidate report.
pub fn encode_best(
    data: &[u8],
    coords: Coords,
    generators: &[Box<dyn Generator>],
    store: &mut Store,
) -> (Address, Vec<Candidate>) {
    assert!(!generators.is_empty(), "need at least one generator");
    let mut candidates: Vec<Candidate> = Vec::with_capacity(generators.len() + 1);
    for g in generators {
        let prediction = g.generate(&coords, data.len());
        let residue = xor(data, &prediction);
        let cost = zstd_len(&residue);
        candidates.push(Candidate {
            id: g.id(),
            generator_hash: g.generator_hash(),
            residue,
            stored_cost: cost,
            coded: false,
        });
    }
    // The adaptive model codec as a candidate generator. Its residue is the
    // entropy-coded stream; its stored cost is that stream's own length.
    let coded = crate::model::encode(data);
    let coded_cost = coded.len();
    candidates.push(Candidate {
        id: crate::model::MODEL_ID.to_string(),
        generator_hash: crate::model_codec::model_generator_hash(),
        residue: coded,
        stored_cost: coded_cost,
        coded: true,
    });

    // Pick min by (stored cost, raw residue len, id) — fully deterministic.
    let best_idx = (0..candidates.len())
        .min_by(|&i, &j| {
            let a = &candidates[i];
            let b = &candidates[j];
            a.stored_cost
                .cmp(&b.stored_cost)
                .then(a.residue.len().cmp(&b.residue.len()))
                .then(a.id.cmp(&b.id))
        })
        .expect("non-empty");

    let best = &candidates[best_idx];
    let residue_hash = store.put(&best.residue);
    let address = Address {
        generator_hash: best.generator_hash,
        coordinates: coords,
        residue_hash,
        len: data.len() as u64,
    };

    (address, candidates)
}

/// A per-generator candidate result, used for the measurement report.
#[derive(Debug, Clone)]
pub struct Candidate {
    /// Generator id string.
    pub id: String,
    /// Content-addressed identity of the generator.
    pub generator_hash: crate::hash::Hash,
    /// The residue bytes stored for this candidate: a XOR residue for a pure
    /// generator, or the entropy-coded stream for the model codec.
    pub residue: Vec<u8>,
    /// The candidate's real stored cost — the comparison key. `zstd(residue)` for
    /// a pure (XOR) generator; the coded length for the already-entropy-coded
    /// model codec.
    pub stored_cost: usize,
    /// `true` if `residue` is an entropy-coded model stream (decode routes through
    /// [`crate::model_codec`]); `false` for a pure XOR residue.
    pub coded: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generator::{default_generators, Constant, Identity};

    #[test]
    fn roundtrip_identity() {
        let mut store = Store::new();
        let reg = Registry::with_defaults();
        let data = b"the quick brown fox";
        let addr = encode(data, &Identity, Coords::origin(), &mut store);
        assert_eq!(decode(&addr, &reg, &store).unwrap(), data);
        // Identity => residue == data.
        assert_eq!(store.get(&addr.residue_hash).unwrap(), data);
    }

    #[test]
    fn constant_data_collapses_residue_to_zero() {
        let mut store = Store::new();
        let data = vec![0x55u8; 100];
        let addr = encode(&data, &Constant(0x55), Coords::origin(), &mut store);
        assert_eq!(store.get(&addr.residue_hash).unwrap(), &vec![0u8; 100][..]);
    }

    #[test]
    fn encode_best_prefers_low_entropy_residue() {
        let mut store = Store::new();
        let gens = default_generators();
        // Constant data: Constant(0xAA) gives an all-zero residue.
        let data = vec![0xAAu8; 256];
        let (_addr, report) = encode_best(&data, Coords::origin(), &gens, &mut store);
        let best = report.iter().min_by_key(|c| c.stored_cost).unwrap();
        // The winner's residue compresses far smaller than Identity's (raw data).
        let identity = report.iter().find(|c| c.id == "identity").unwrap();
        assert!(best.stored_cost <= identity.stored_cost);
    }

    #[test]
    fn decode_detects_missing_residue() {
        let store = Store::new();
        let reg = Registry::with_defaults();
        let addr = Address {
            generator_hash: Identity.generator_hash(),
            coordinates: Coords::origin(),
            residue_hash: crate::hash::Hash::of(b"absent"),
            len: 4,
        };
        assert_eq!(
            decode(&addr, &reg, &store),
            Err(DecodeError::MissingResidue)
        );
    }
}
