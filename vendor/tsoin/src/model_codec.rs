//! The **model codec** — the adaptive predictive model ([`crate::model`]) wired
//! into the engine as a first-class codec path producing an [`Address`].
//!
//! Where the pure codec ([`crate::codec`]) computes `residue = data XOR
//! generator(coords)` and stores the residue verbatim, the model codec computes
//! `residue = entropy_code(data under the adaptive model)` and stores **that**.
//! The address layout is unchanged — it is still `(generator_hash, coordinates,
//! residue_hash, len)`:
//!
//! * `generator_hash = Hash::of(`[`crate::model::MODEL_ID`]`)` — the model *is*
//!   the generator (the shared knowledge);
//! * `coordinates` — unused by the model (it conditions on the data's own past,
//!   not on coords); kept for a uniform address shape, conventionally
//!   [`Coords::origin`];
//! * `residue_hash` — points at the entropy-coded blob in the [`Store`] (the
//!   surprise, the only thing stored);
//! * `len` — original byte length, which the decoder needs to know how many
//!   bytes to pull back out of the coded stream.
//!
//! Decoding resolves the blob, re-runs the model over it, and reproduces the
//! input **bit-for-bit**. Because the model identity is a hash like any other
//! generator's, the same [`Registry`]/decode dispatch can route to it — see
//! [`decode`] which recognises the model hash and otherwise defers to the pure
//! codec.

use crate::address::Address;
use crate::codec::{self, DecodeError};
use crate::coords::Coords;
use crate::hash::Hash;
use crate::model::{self, MODEL_ID};
use crate::registry::Registry;
use crate::store::Store;

/// The model generator's content-addressed identity (`Hash::of(MODEL_ID)`).
#[must_use]
pub fn model_generator_hash() -> Hash {
    Hash::of(MODEL_ID.as_bytes())
}

/// Encode `data` with the adaptive model, storing the coded residue and
/// returning its [`Address`]. The address's `generator_hash` is the model id.
pub fn encode(data: &[u8], store: &mut Store) -> Address {
    let coded = model::encode(data);
    let residue_hash = store.put(&coded);
    Address {
        generator_hash: model_generator_hash(),
        coordinates: Coords::origin(),
        residue_hash,
        len: data.len() as u64,
    }
}

/// Decode an [`Address`] whose `generator_hash` is the model id, reproducing the
/// original bytes bit-for-bit. Errors mirror the pure codec's: a missing blob is
/// [`DecodeError::MissingResidue`]; a `generator_hash` that is *not* the model id
/// is [`DecodeError::UnknownGenerator`] (use [`decode`] for transparent routing).
pub fn decode_model(address: &Address, store: &Store) -> Result<Vec<u8>, DecodeError> {
    if address.generator_hash != model_generator_hash() {
        return Err(DecodeError::UnknownGenerator);
    }
    let coded = store
        .get(&address.residue_hash)
        .ok_or(DecodeError::MissingResidue)?;
    Ok(model::decode(coded, address.len as usize))
}

/// Transparent decode: if `address` is a model-coded address, run the model;
/// otherwise defer to the pure [`crate::codec::decode`] (which resolves the
/// generator from `registry`). One entry point that handles both codec paths.
pub fn decode(
    address: &Address,
    registry: &Registry,
    store: &Store,
) -> Result<Vec<u8>, DecodeError> {
    if address.generator_hash == model_generator_hash() {
        decode_model(address, store)
    } else {
        codec::decode(address, registry, store)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_via_address() {
        let mut store = Store::new();
        let reg = Registry::with_defaults();
        let data = b"model codec round-trip: the model is the generator, residue is the surprise.";
        let addr = encode(data, &mut store);
        assert_eq!(addr.generator_hash, model_generator_hash());
        assert_eq!(addr.len as usize, data.len());
        let back = decode(&addr, &reg, &store).unwrap();
        assert_eq!(&back, data);
    }

    #[test]
    fn decode_model_rejects_foreign_generator() {
        let store = Store::new();
        let addr = Address {
            generator_hash: Hash::of(b"not-the-model"),
            coordinates: Coords::origin(),
            residue_hash: Hash::of(b"x"),
            len: 0,
        };
        assert_eq!(decode_model(&addr, &store), Err(DecodeError::UnknownGenerator));
    }

    #[test]
    fn transparent_decode_routes_pure_codec() {
        // A pure (Identity) address must still decode through the model_codec's
        // transparent entry point.
        use crate::generator::Identity;
        let mut store = Store::new();
        let reg = Registry::with_defaults();
        let data = b"pure path";
        let addr = codec::encode(data, &Identity, Coords::origin(), &mut store);
        let back = decode(&addr, &reg, &store).unwrap();
        assert_eq!(&back, data);
    }
}
