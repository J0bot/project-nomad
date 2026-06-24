//! `tsoin-demo` — round-trips a few corpora, proves a fork is free, and prints
//! the honest measurement table.

use tsoin::codec::encode_best;
use tsoin::coords::Coords;
use tsoin::generator::{default_generators, Generator, Gradient2D, Identity, LinearPredictor};
use tsoin::measure::{measure, render_table, Row};
use tsoin::model_codec::decode; // transparent: routes model OR pure addresses
use tsoin::{codec, Branch, Registry, Store};

/// Deterministic pseudo-random bytes (LCG) — reproducible across runs/machines.
fn pseudo_random(n: usize, seed: u64) -> Vec<u8> {
    let mut x = seed;
    (0..n)
        .map(|_| {
            x = x
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            (x >> 33) as u8
        })
        .collect()
}

/// A diagonal gradient "image" that exactly matches `Gradient2D` at `rect(w,h)`.
fn gradient_image(w: u64, h: u64) -> (Vec<u8>, Coords) {
    let coords = Coords::rect(w, h);
    let data = Gradient2D.generate(&coords, (w * h) as usize);
    (data, coords)
}

/// A smooth/repetitive sequence: a true arithmetic ramp matching LinearPredictor.
fn smooth_ramp(n: usize, start: u8, step: u8) -> (Vec<u8>, Coords) {
    let coords = Coords {
        seed: start as u64,
        x: step as u64,
        ..Coords::default()
    };
    let data = LinearPredictor.generate(&coords, n);
    (data, coords)
}

fn main() {
    println!("== tsoin — Adressage Génératif v0 — demo ==\n");

    let registry = Registry::with_defaults();
    let generators = default_generators();

    // ---- 1. Round-trip demonstration (bit-exact) -------------------------
    println!("[1] round-trip (decode(encode(x)) == x), bit-for-bit:");
    let sample = b"the quick brown fox jumps over the lazy dog";
    let mut store = Store::new();
    let (addr, _report) = encode_best(sample, Coords::origin(), &generators, &mut store);
    let back = decode(&addr, &registry, &store).expect("decodes");
    println!("    data={:?}", std::str::from_utf8(sample).unwrap());
    // The winner may be a pure generator (in the registry) or the model codec
    // (resolved by the transparent router, not the registry).
    let best_label = if addr.generator_hash == tsoin::model_codec::model_generator_hash() {
        tsoin::model::MODEL_ID.to_string()
    } else {
        registry
            .get(&addr.generator_hash)
            .map(tsoin::Generator::id)
            .unwrap_or_else(|| "?".into())
    };
    println!(
        "    best generator = {}  (residue_hash = {})",
        best_label,
        addr.residue_hash.short()
    );
    println!("    round-trip exact: {}\n", back == sample);
    assert_eq!(back, sample);

    // ---- 2. Fork is free -------------------------------------------------
    println!("[2] fork is free (store.blob_count proves it):");
    let mut store2 = Store::new();
    let mut main = Branch::new();
    for i in 0u8..5 {
        let a = codec::encode(&[i; 32], &Identity, Coords::origin(), &mut store2);
        main.append(a);
    }
    let before = store2.blob_count();
    let mut feature = main.fork();
    let after_fork = store2.blob_count();
    let new_state = codec::encode(
        b"a brand new branch state",
        &Identity,
        Coords::origin(),
        &mut store2,
    );
    feature.append(new_state);
    let after_append = store2.blob_count();
    println!(
        "    main has {} states; store blobs before fork  = {before}",
        main.len()
    );
    println!(
        "    after fork (5 shared states): store blobs      = {after_fork}  (delta {})",
        after_fork - before
    );
    println!(
        "    after fork + 1 append:        store blobs      = {after_append}  (delta {})",
        after_append - after_fork
    );
    println!(
        "    => fork added 0 blobs, append added exactly 1: {}\n",
        after_fork == before && after_append == after_fork + 1
    );

    // ---- 3. Honest measurement table ------------------------------------
    println!("[3] measurement table (raw vs best residue vs zstd(raw) baseline):");
    println!("    encode_best now also considers the adaptive MODEL codec (ctx-order2-v0):");
    println!("    real text/code => the model is the cheapest generator (residue collapses);");
    println!("    structured data => a cheap pure generator wins; random => no free lunch.\n");
    let mut rows: Vec<Row> = Vec::new();

    // (a) random bytes — no free lunch (the model must NOT shrink these).
    let rnd = pseudo_random(8192, 0xDEAD_BEEF_CAFE_F00D);
    rows.push(measure("random", &rnd, Coords::origin(), &generators));

    // (b) gradient image — matches Gradient2D => residue collapses to ~0.
    let (img, img_coords) = gradient_image(64, 64);
    rows.push(measure("gradient_img", &img, img_coords, &generators));

    // (c) smooth/repetitive ramp — matches LinearPredictor => collapses to ~0.
    let (ramp, ramp_coords) = smooth_ramp(8192, 0, 1);
    rows.push(measure("smooth_ramp", &ramp, ramp_coords, &generators));

    // (d) REAL text / code: this crate's own sources, Cargo.lock, and (if present)
    //     a real braindump. The model is the knowledge => residue << raw.
    let manifest = env!("CARGO_MANIFEST_DIR");
    let real_files: &[(&str, String)] = &[
        ("src_lib.rs", format!("{manifest}/src/lib.rs")),
        ("src_model.rs", format!("{manifest}/src/model.rs")),
        ("src_timeline.rs", format!("{manifest}/src/timeline.rs")),
        ("Cargo.lock", format!("{manifest}/Cargo.lock")),
        (
            "braindump.md",
            "/bi0ns/input/2026-06-17-braindump-machine-a-tsoins.md".to_string(),
        ),
    ];
    for (label, path) in real_files {
        match std::fs::read(path) {
            Ok(bytes) if !bytes.is_empty() => {
                rows.push(measure(label, &bytes, Coords::origin(), &generators));
            }
            _ => {} // skip unavailable corpora silently (e.g. /bi0ns not mounted)
        }
    }

    let table = render_table(&rows);
    print!("{table}");

    println!("\nReading the table:");
    println!("  - random:        residue ~>= raw, nothing shrinks it -> NO free lunch (honest).");
    println!("  - gradient_img/smooth_ramp: data matches a pure generator -> residue ~0.");
    println!("  - real text/code: ctx-order2-v0 (the model) wins; its entropy-coded residue");
    println!("                    is well below raw and competitive with zstd(raw).");

    // ---- 4. Prove the model path is bit-exact on a real file -------------
    println!("\n[4] model codec round-trip on a real file (bit-exact):");
    if let Ok(src) = std::fs::read(format!("{manifest}/src/model.rs")) {
        let mut s = Store::new();
        let addr = tsoin::model_codec::encode(&src, &mut s);
        let back = decode(&addr, &registry, &s).expect("model decode");
        let coded = s.get(&addr.residue_hash).unwrap().len();
        println!(
            "    src/model.rs: raw={} coded={} ({:.1}% of raw)  round-trip exact: {}",
            src.len(),
            coded,
            100.0 * coded as f64 / src.len() as f64,
            back == src
        );
        assert_eq!(back, src, "model codec must be bit-exact");
    }
}
