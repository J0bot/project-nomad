//! `tsoin-timeline-demo` — the temporal tsoin recorder in action.
//!
//! Records a slow-changing session and a fast-changing one (store-vs-naive for
//! both, honest), forks the slow session at frame 25 and diverges, replays BOTH
//! branches asserting bit-exactness, then injects a frame identical to an earlier
//! one to show the xerboxion detector fire. *Le B0XION = le git des états.*

use tsoin::coords::Coords;
use tsoin::timeline::{Recorder, Timeline};

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

fn pct(part: usize, whole: usize) -> f64 {
    if whole == 0 {
        0.0
    } else {
        100.0 * part as f64 / whole as f64
    }
}

fn main() {
    println!("== tsoin — temporal tsoin recorder — demo ==");
    println!("   a branchable Merkle timeline; each frame stores only the surprise");
    println!("   between moments. Le B0XION = le git des états.\n");

    // ---- 1. Slow-changing session: a 64-byte buffer, a few bytes/frame -------
    println!("[1] slow-changing session (64-byte buffer, ~2 bytes change / frame, 50 frames):");
    let mut rec = Recorder::new();
    let mut slow = Timeline::new();
    let mut buf = vec![0x7Eu8; 64];
    let mut slow_frames: Vec<Vec<u8>> = Vec::new();
    for t in 0..50u64 {
        buf[(t as usize) % 64] = buf[(t as usize) % 64].wrapping_add(1);
        buf[(2 * t as usize + 1) % 64] = (t as u8).wrapping_mul(3);
        slow.record(
            &mut rec,
            &buf,
            Coords {
                t,
                ..Default::default()
            },
        );
        slow_frames.push(buf.clone());
    }
    let slow_head = slow.head().unwrap();
    let naive_slow = rec.naive_cost(&slow_head);
    let store_raw_slow = rec.store_footprint_raw();
    let store_z_slow = rec.store_footprint_compressed();
    let store_j_slow = rec.store_footprint_joint();
    println!("    naive cost (sum of 50 full frames) = {naive_slow} bytes");
    println!(
        "    store, raw deltas                  = {store_raw_slow} bytes  ({:.1}% of naive)",
        pct(store_raw_slow, naive_slow)
    );
    println!(
        "    store, per-blob zstd               = {store_z_slow} bytes  ({:.1}% of naive)",
        pct(store_z_slow, naive_slow)
    );
    println!(
        "    store, joint zstd (true surprise)  = {store_j_slow} bytes  ({:.1}% of naive)  <-- the win",
        pct(store_j_slow, naive_slow)
    );
    println!("    (raw deltas are full-length but mostly ZERO; a real compressor crushes them.)\n");

    // ---- 2. Fast-changing session: every frame fresh entropy -----------------
    println!("[2] fast-changing session (64-byte buffer, all-new bytes / frame, 50 frames):");
    let mut rec_fast = Recorder::new();
    let mut fast = Timeline::new();
    for t in 0..50u64 {
        let frame = pseudo_random(64, 0xABCD_0000_0000_0000 ^ t.wrapping_mul(0x9E37_79B9));
        fast.record(
            &mut rec_fast,
            &frame,
            Coords {
                t,
                ..Default::default()
            },
        );
    }
    let fast_head = fast.head().unwrap();
    let naive_fast = rec_fast.naive_cost(&fast_head);
    let store_j_fast = rec_fast.store_footprint_joint();
    println!("    naive cost                         = {naive_fast} bytes");
    println!(
        "    store, joint zstd (true surprise)  = {store_j_fast} bytes  ({:.1}% of naive)",
        pct(store_j_fast, naive_fast)
    );
    println!("    => deltas are high-entropy, store ~= naive. NO free lunch (honest).\n");

    // ---- 3. Fork at frame 25, diverge, replay BOTH branches bit-exact --------
    println!("[3] free fork of the slow session at frame 25, then diverge:");
    // The node hash at frame index 25 (0-based) on the slow timeline.
    let slow_all_frames = slow.frames(&rec); // oldest first, full sequence
    assert_eq!(
        slow_all_frames, slow_frames,
        "slow timeline replay must match"
    );
    // Walk to the frame-25 node by replaying parents: easiest is to re-record on a
    // fork from a Timeline positioned at that node. We get that node by collecting
    // heads again from a throwaway re-walk.
    let fork_node = {
        // Re-derive the head hashes by recording into a fresh recorder would change
        // hashes? No — node hashes are deterministic from (parent,residue,len,coords)
        // and identical input yields identical nodes. But simplest: keep heads.
        // We recorded into `rec`; collect heads via replay_all-style walk:
        // climb from slow_head to root counting depth.
        let mut chain = Vec::new();
        let mut cur = Some(slow_head);
        while let Some(h) = cur {
            let n = rec.node(&h).expect("node");
            cur = n.parent;
            chain.push(h);
        }
        chain.reverse(); // root .. head
        chain[25]
    };

    let before_fork = rec.blob_count();
    let mut branch_a = Timeline::at(fork_node);
    let mut branch_b = Timeline::at(fork_node);
    println!("    blob_count before fork = {before_fork} (fork copies NOTHING)");
    assert_eq!(rec.blob_count(), before_fork);

    // Branch A: drift the buffer one way.
    let mut a_frames: Vec<Vec<u8>> = Vec::new();
    let mut a_buf = rec.replay(&fork_node);
    for k in 0..8u8 {
        a_buf[k as usize % 64] = 0xA0u8.wrapping_add(k);
        branch_a.record(&mut rec, &a_buf, Coords::default());
        a_frames.push(a_buf.clone());
    }
    // Branch B: drift it the other way (different bytes => different deltas).
    let mut b_frames: Vec<Vec<u8>> = Vec::new();
    let mut b_buf = rec.replay(&fork_node);
    for k in 0..8u8 {
        b_buf[(63 - k as usize) % 64] = 0x0Bu8.wrapping_mul(k.wrapping_add(1));
        branch_b.record(&mut rec, &b_buf, Coords::default());
        b_frames.push(b_buf.clone());
    }
    let after = rec.blob_count();
    println!(
        "    blob_count after 2 branches x 8 frames = {after} (+{} distinct deltas)",
        after - before_fork
    );

    // Replay BOTH branches and assert bit-exact (full sequence = shared prefix + suffix).
    let shared_prefix: Vec<Vec<u8>> = slow_frames[..=25].to_vec();
    let expect_a: Vec<Vec<u8>> = shared_prefix
        .iter()
        .cloned()
        .chain(a_frames.clone())
        .collect();
    let expect_b: Vec<Vec<u8>> = shared_prefix
        .iter()
        .cloned()
        .chain(b_frames.clone())
        .collect();
    let got_a = branch_a.frames(&rec);
    let got_b = branch_b.frames(&rec);
    assert_eq!(got_a, expect_a, "branch A replay must be bit-exact");
    assert_eq!(got_b, expect_b, "branch B replay must be bit-exact");
    println!(
        "    branch A: {} frames, replay bit-exact: {}",
        got_a.len(),
        got_a == expect_a
    );
    println!(
        "    branch B: {} frames, replay bit-exact: {}",
        got_b.len(),
        got_b == expect_b
    );
    println!("    the shared past (frames 0..=25) is stored ONCE for both branches.\n");

    // ---- 4. xerboxion: inject a frame identical to an earlier one ------------
    println!("[4] xerboxion detector (José's popup) — exact-identical instants:");
    let target = slow_frames[10].clone(); // an instant that really happened
    let xerb = branch_a.record(&mut rec, &target, Coords::default());
    match xerb {
        Some(x) => {
            println!("    re-recorded frame[10]'s exact bytes on branch A.");
            println!(
                "    XERBOXION fired! content_hash = {}",
                x.content_hash.short()
            );
            println!("      first  node = {}", x.first.short());
            println!("      repeat node = {}", x.repeat.short());
            println!("    running xerboxion count = {}", rec.xerboxion_count());
            assert_eq!(rec.replay(&x.first), target);
            assert_eq!(rec.replay(&x.repeat), target);
        }
        None => panic!("expected a xerboxion on an exact repeat"),
    }
    // And a near-miss does NOT fire.
    let mut near = slow_frames[12].clone();
    near[0] ^= 0x01;
    let before_count = rec.xerboxion_count();
    let none = branch_b.record(&mut rec, &near, Coords::default());
    println!(
        "    a 1-byte-different frame fired a xerboxion? {}  (count still {})",
        none.is_some(),
        rec.xerboxion_count()
    );
    assert!(none.is_none(), "near-miss must NOT fire");
    assert_eq!(rec.xerboxion_count(), before_count);

    println!("\nSummary:");
    println!(
        "  slow session: store {store_j_slow}B vs naive {naive_slow}B  ({:.1}% — the surprise is small).",
        pct(store_j_slow, naive_slow)
    );
    println!(
        "  fast session: store {store_j_fast}B vs naive {naive_fast}B  ({:.1}% — honest, no magic).",
        pct(store_j_fast, naive_fast)
    );
    println!("  forks are free (shared past stored once); replay is bit-exact on every branch.");
    println!("  xerboxion fires only on byte-for-byte identical instants.");
}
