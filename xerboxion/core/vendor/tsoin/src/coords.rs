//! Coordinates — the second field of the generative address.
//!
//! Coordinates *locate* the data so a generator can predict it: a seed plus a
//! 4D point `(x, y, z, t)`. For procedural generators they parametrize the
//! prediction (e.g. `Gradient2D` reads `x` as width); for seeded generators the
//! seed selects the stream. A canonical Hilbert-4D index for sensor data is
//! future work (synthesis §4); this flat struct is the v0 stand-in.

/// A point in the generative coordinate space.
///
/// All fields are part of the address and therefore part of what is hashed when
/// an address is serialized — two encodings with different coords are distinct
/// even if they share a generator and residue.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Coords {
    /// Generator-defined seed / stream selector.
    pub seed: u64,
    /// Spatial / index axes. `Gradient2D` interprets `x`=width, `y`=height.
    pub x: u64,
    pub y: u64,
    pub z: u64,
    /// Time axis.
    pub t: u64,
}

impl Coords {
    /// Coords with everything zero.
    #[must_use]
    pub fn origin() -> Self {
        Coords::default()
    }

    /// Convenience: just a seed, rest zero.
    #[must_use]
    pub fn seeded(seed: u64) -> Self {
        Coords {
            seed,
            ..Coords::default()
        }
    }

    /// Convenience for 2D generators: width/height in `x`/`y`.
    #[must_use]
    pub fn rect(width: u64, height: u64) -> Self {
        Coords {
            x: width,
            y: height,
            ..Coords::default()
        }
    }

    /// Canonical byte encoding of the coordinates (little-endian, fixed order).
    /// Used when hashing an address so the encoding is stable across machines.
    #[must_use]
    pub fn to_bytes(&self) -> [u8; 40] {
        let mut out = [0u8; 40];
        out[0..8].copy_from_slice(&self.seed.to_le_bytes());
        out[8..16].copy_from_slice(&self.x.to_le_bytes());
        out[16..24].copy_from_slice(&self.y.to_le_bytes());
        out[24..32].copy_from_slice(&self.z.to_le_bytes());
        out[32..40].copy_from_slice(&self.t.to_le_bytes());
        out
    }
}
