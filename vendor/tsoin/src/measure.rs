//! Honest measurements of the residue idea.
//!
//! For each corpus we report: `raw_len`, the `best` generator and its raw
//! `residue_len`, `zstd(best_residue)`, and `zstd(raw)` as the honest baseline.
//!
//! The point this demonstrates (and the point we must NOT fake):
//! - data that MATCHES a generator => residue collapses toward 0 bytes after
//!   zstd (the procedural limit case);
//! - RANDOM data => the residue stays ~= raw and zstd cannot shrink it (no free
//!   lunch — the engine is honest, it does not invent compression).

use crate::codec::{encode_best, zstd_len};
use crate::coords::Coords;
use crate::generator::Generator;
use crate::store::Store;

/// One row of the measurement table.
#[derive(Debug, Clone)]
pub struct Row {
    /// Human label for the corpus.
    pub corpus: String,
    /// Original byte length.
    pub raw_len: usize,
    /// Id of the generator `encode_best` chose.
    pub best_generator: String,
    /// Raw residue length for the winning generator (== raw_len for a XOR
    /// generator; the coded-stream length for the model codec).
    pub residue_len: usize,
    /// The winning residue's real stored cost — `zstd(residue)` for a pure
    /// generator, the coded length for the already-entropy-coded model codec.
    pub stored_residue_len: usize,
    /// zstd of the raw data — the honest general-purpose baseline.
    pub zstd_raw_len: usize,
}

impl Row {
    /// stored residue cost as a fraction of raw length (lower = better).
    #[must_use]
    pub fn residue_ratio(&self) -> f64 {
        if self.raw_len == 0 {
            0.0
        } else {
            self.stored_residue_len as f64 / self.raw_len as f64
        }
    }
}

/// Measure one corpus with the given generators and coords.
pub fn measure(
    corpus: &str,
    data: &[u8],
    coords: Coords,
    generators: &[Box<dyn Generator>],
) -> Row {
    // Use a throwaway store; we only care about sizes here.
    let mut store = Store::new();
    let (_addr, report) = encode_best(data, coords, generators, &mut store);
    let best = report
        .iter()
        .min_by(|a, b| {
            a.stored_cost
                .cmp(&b.stored_cost)
                .then(a.residue.len().cmp(&b.residue.len()))
                .then(a.id.cmp(&b.id))
        })
        .expect("non-empty generator set");
    Row {
        corpus: corpus.to_string(),
        raw_len: data.len(),
        best_generator: best.id.clone(),
        residue_len: best.residue.len(),
        stored_residue_len: best.stored_cost,
        zstd_raw_len: zstd_len(data),
    }
}

/// Render rows as a fixed-width text table (what the demo prints / we capture).
#[must_use]
pub fn render_table(rows: &[Row]) -> String {
    let mut out = String::new();
    let header = format!(
        "{:<16} {:>9} {:>16} {:>11} {:>14} {:>13} {:>9}\n",
        "corpus", "raw_len", "best_generator", "residue", "resid_cost", "zstd(raw)", "resid/raw"
    );
    out.push_str(&header);
    out.push_str(&"-".repeat(header.len() - 1));
    out.push('\n');
    for r in rows {
        out.push_str(&format!(
            "{:<16} {:>9} {:>16} {:>11} {:>14} {:>13} {:>8.2}%\n",
            r.corpus,
            r.raw_len,
            r.best_generator,
            r.residue_len,
            r.stored_residue_len,
            r.zstd_raw_len,
            r.residue_ratio() * 100.0
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generator::default_generators;

    #[test]
    fn matching_data_collapses_random_does_not() {
        let gens = default_generators();
        let constant = vec![0x33u8; 4096];
        let row_c = measure("const", &constant, Coords::origin(), &gens);
        // Constant data: residue collapses to near nothing.
        assert!(
            row_c.stored_residue_len < row_c.raw_len / 10,
            "constant residue should collapse, got {} of {}",
            row_c.stored_residue_len,
            row_c.raw_len
        );

        // Pseudo-random data (deterministic LCG so the test is reproducible).
        let mut x: u64 = 0x1234_5678_9abc_def0;
        let random: Vec<u8> = (0..4096)
            .map(|_| {
                x = x
                    .wrapping_mul(6364136223846793005)
                    .wrapping_add(1442695040888963407);
                (x >> 33) as u8
            })
            .collect();
        let row_r = measure("random", &random, Coords::origin(), &gens);
        // No free lunch: the best residue cost stays close to raw (within ~10%).
        // The model competes here too and must NOT magically shrink random data.
        assert!(
            row_r.stored_residue_len as f64 > row_r.raw_len as f64 * 0.90,
            "random residue must not magically compress, got {} of {}",
            row_r.stored_residue_len,
            row_r.raw_len
        );
    }
}
