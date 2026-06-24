//! Generators — pure deterministic predictors (the "software CFC" contract).
//!
//! A generator is a pure function `(coords, len) -> Vec<u8>` of exactly `len`
//! predicted bytes. It MUST be deterministic across machines and time: same
//! `id`, same `coords`, same `len` => identical bytes, forever. That property is
//! what lets us store only the residue (real XOR prediction) and reconstruct the
//! data anywhere by re-running the generator.
//!
//! A generator's *stable identity* is `Hash::of(id_string)`. The id string is
//! the only thing that needs to round-trip for a decoder to rebuild the
//! generator (see [`crate::registry`]). WASM modules are the long-term ideal for
//! this contract (synthesis §4 layer 0); v0 uses deterministic Rust generators
//! behind this trait.
//!
//! The two limit cases of the whole system are generators:
//! - [`Identity`] predicts all-zero => residue == data == classic storage.
//! - a perfect procedural generator predicts the data exactly => residue all-zero.

use crate::coords::Coords;
use crate::hash::Hash;

/// A pure, deterministic byte predictor.
pub trait Generator {
    /// Stable string identity of this generator instance (includes parameters,
    /// e.g. `"constant:0x40"`). Hashing this gives [`Self::generator_hash`].
    fn id(&self) -> String;

    /// Predict exactly `len` bytes at `coords`. MUST be pure and deterministic.
    fn generate(&self, coords: &Coords, len: usize) -> Vec<u8>;

    /// The generator's content-addressed identity: `Hash::of(id())`.
    fn generator_hash(&self) -> Hash {
        Hash::of(self.id().as_bytes())
    }
}

/// Predicts all zeros. Residue then equals the data verbatim — the
/// classic-storage limit case (the store degenerates to a plain blob store).
#[derive(Debug, Clone, Copy, Default)]
pub struct Identity;

impl Generator for Identity {
    fn id(&self) -> String {
        "identity".to_string()
    }
    fn generate(&self, _coords: &Coords, len: usize) -> Vec<u8> {
        vec![0u8; len]
    }
}

/// Predicts a single repeated byte. Good for constant fills / padding; the
/// procedural limit case for constant data (residue collapses to all-zero).
#[derive(Debug, Clone, Copy)]
pub struct Constant(pub u8);

impl Generator for Constant {
    fn id(&self) -> String {
        format!("constant:{:#04x}", self.0)
    }
    fn generate(&self, _coords: &Coords, len: usize) -> Vec<u8> {
        vec![self.0; len]
    }
}

/// First-order linear predictor for smooth / sequential data.
///
/// Models `byte[i] ≈ byte[i-1] + step` (wrapping), with `byte[-1]` taken from
/// `coords.seed`'s low byte and `step` from `coords.x`'s low byte. For a ramp or
/// slowly varying signal whose true step matches, the residue collapses toward
/// zero. Note: because the predictor cannot see the *real* previous byte (that
/// would make it data-dependent and break purity from coords alone), it predicts
/// a pure arithmetic progression seeded by the coords — exact for true ramps,
/// approximate otherwise. This is honest: it is a *generator*, not a filter.
#[derive(Debug, Clone, Copy, Default)]
pub struct LinearPredictor;

impl Generator for LinearPredictor {
    fn id(&self) -> String {
        "linear".to_string()
    }
    fn generate(&self, coords: &Coords, len: usize) -> Vec<u8> {
        let start = (coords.seed & 0xff) as u8;
        let step = (coords.x & 0xff) as u8;
        let mut out = Vec::with_capacity(len);
        let mut cur = start;
        for _ in 0..len {
            out.push(cur);
            cur = cur.wrapping_add(step);
        }
        out
    }
}

/// A procedural 2D ramp — a stand-in "image generator".
///
/// Reads `width` from `coords.x` and `height` from `coords.y`. Pixel `(px, py)`
/// (row-major, one byte per pixel) is predicted as
/// `((px + py) * 256 / (width + height)) mod 256` — a diagonal gradient. For an
/// image that *is* this gradient, the residue is all-zero (pure procedural).
/// If `width`/`height` are zero, falls back to a 1D index ramp so it is always
/// well-defined for `len` bytes.
#[derive(Debug, Clone, Copy, Default)]
pub struct Gradient2D;

impl Generator for Gradient2D {
    fn id(&self) -> String {
        "gradient2d".to_string()
    }
    fn generate(&self, coords: &Coords, len: usize) -> Vec<u8> {
        let width = coords.x.max(1);
        let height = coords.y.max(1);
        let span = (width + height).max(1);
        let mut out = Vec::with_capacity(len);
        for i in 0..len as u64 {
            let px = i % width;
            let py = i / width;
            // Diagonal ramp normalized to a byte.
            let v = ((px + py).wrapping_mul(256) / span) % 256;
            out.push(v as u8);
            if py >= height && width != 0 {
                // Beyond declared height we keep extending the ramp rather than
                // wrapping abruptly, so len > width*height stays well-defined.
            }
        }
        out
    }
}

/// Produce the canonical set of v0 generators as trait objects.
///
/// Order is stable; `encode_best` iterates this slice. Generators that take a
/// parameter (e.g. `Constant`) are included with a couple of useful values.
#[must_use]
pub fn default_generators() -> Vec<Box<dyn Generator>> {
    vec![
        Box::new(Identity),
        Box::new(Constant(0x00)),
        Box::new(Constant(0xff)),
        Box::new(LinearPredictor),
        Box::new(Gradient2D),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_is_all_zero() {
        let g = Identity;
        assert_eq!(g.generate(&Coords::origin(), 5), vec![0, 0, 0, 0, 0]);
    }

    #[test]
    fn constant_repeats_byte() {
        let g = Constant(0x42);
        assert_eq!(g.generate(&Coords::origin(), 3), vec![0x42; 3]);
        assert_eq!(g.id(), "constant:0x42");
    }

    #[test]
    fn linear_is_arithmetic_progression() {
        let g = LinearPredictor;
        let c = Coords {
            seed: 10,
            x: 2,
            ..Coords::default()
        };
        assert_eq!(g.generate(&c, 4), vec![10, 12, 14, 16]);
    }

    #[test]
    fn generators_are_deterministic_across_calls() {
        for g in default_generators() {
            let c = Coords::rect(8, 8);
            assert_eq!(g.generate(&c, 64), g.generate(&c, 64));
        }
    }

    #[test]
    fn generator_hash_tracks_id() {
        assert_eq!(Constant(1).generator_hash(), Hash::of(b"constant:0x01"));
        assert_ne!(Constant(1).generator_hash(), Constant(2).generator_hash());
    }
}
