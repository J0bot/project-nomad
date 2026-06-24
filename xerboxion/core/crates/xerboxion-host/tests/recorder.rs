//! Integration tests for the **bus-as-tsoin recorder** — the project thesis
//! applied reflexively to the runtime: the core records its OWN ordered bus trace
//! as a real tsoin timeline (replayable bit-exact, forkable, with engine-measured
//! collapse).
//!
//! These are OFFLINE and fully deterministic — they build a known [`Trace`] event
//! list directly (no wasm, no network, no clock), so they run anywhere and never
//! flake. The engine doing the recording is the REAL `tsoin` crate depended on by
//! the host; every figure asserted here is read back out of it, never faked.
//!
//! Coverage (the requirements):
//! - record a known event list -> replay is BIT-EXACT (byte-equality on frames,
//!   and the decoded stream equals the original events in order);
//! - SABOTAGE: alter one recorded frame's bytes, or drop one, before replay ->
//!   the tamper is DETECTED (decode fails or the stream no longer matches);
//! - FORK produces an independent branch (original intact, branch diverged,
//!   fork is free — only one new delta blob);
//! - stats are consistent (joint stored <= raw for a compressible stream; the
//!   Merkle/timeline ROOT changes when the content changes, and is stable when it
//!   does not).

use xerboxion_host::recorder::{
    self, decode_event, encode_event, RecordedEvent,
};
use xerboxion_host::Trace;

/// A known, representative trace exercising EVERY `Trace` variant. Fixed content,
/// so every derived figure (root hash, byte counts) is deterministic.
fn known_trace() -> Vec<Trace> {
    vec![
        Trace::Lifecycle {
            who: "ping".into(),
            what: "init".into(),
        },
        Trace::Emit {
            from: "ping".into(),
            topic: "ping".into(),
            payload: "ping#1".into(),
        },
        Trace::Deliver {
            from: "ping".into(),
            to: "pong".into(),
            topic: "ping".into(),
        },
        Trace::Log {
            from: "pong".into(),
            line: "pong: got 'ping#1' -> count=1".into(),
        },
        Trace::Emit {
            from: "pong".into(),
            topic: "pong".into(),
            payload: "pong#1".into(),
        },
        Trace::Fetch {
            from: "health-adapter".into(),
            method: "GET".into(),
            url: "https://fs.j0bot.ch/health".into(),
            status: 200,
        },
        Trace::Recipe {
            who: "recipe:alerter".into(),
            step: "FIRE -> emit [alert.notify] ideas-map is DOWN (code 503)".into(),
        },
        Trace::Lifecycle {
            who: "ping".into(),
            what: "goodbye".into(),
        },
    ]
}

/// The expected canonical event stream for [`known_trace`] (the bit-exact target).
fn expected_events(trace: &[Trace]) -> Vec<RecordedEvent> {
    trace
        .iter()
        .enumerate()
        .map(|(i, t)| RecordedEvent::from_trace(i, t))
        .collect()
}

#[test]
fn record_then_replay_is_bit_exact() {
    let trace = known_trace();
    let bt = recorder::record_trace(&trace);

    // The engine's reconstruction is byte-identical to what was recorded.
    let replayed = recorder::replay(&bt);
    assert_eq!(
        replayed, bt.frames,
        "engine replay must be byte-for-byte identical to the recorded frames"
    );

    // And the reconstructed-then-decoded stream equals the original events, in
    // order — the EXACT ordered bus event stream comes back.
    let events = recorder::replay_events(&bt).expect("every replayed frame decodes");
    assert_eq!(events, expected_events(&trace));
    assert_eq!(events.len(), trace.len());

    // Every frame carries its journal index as its seq.
    for (i, ev) in events.iter().enumerate() {
        assert_eq!(ev.seq, i as u64);
    }
}

#[test]
fn sabotage_altering_a_frame_is_detected() {
    let trace = known_trace();
    let bt = recorder::record_trace(&trace);
    let original = recorder::replay_events(&bt).expect("decodes");

    // Take the recorded frames and TAMPER with one byte deep inside a frame's
    // payload region (past the 13-byte header), simulating a corrupted/altered
    // recording before replay.
    let mut frames = recorder::replay(&bt);
    let victim = 3usize; // the pong log frame
    let pos = frames[victim].len() - 1; // last byte of the field region
    frames[victim][pos] ^= 0xFF;

    // Decoding the tampered set either fails outright or yields a stream that no
    // longer matches the original — the tamper is DETECTED, never silently passed.
    let decoded: Option<Vec<RecordedEvent>> =
        frames.iter().map(|f| decode_event(f)).collect();
    match decoded {
        None => { /* corruption caught at decode time — detected */ }
        Some(stream) => assert_ne!(
            stream, original,
            "a tampered frame must NOT reconstruct the original stream"
        ),
    }
}

#[test]
fn sabotage_corrupting_the_magic_fails_to_decode() {
    let ev = RecordedEvent::from_trace(
        0,
        &Trace::Emit {
            from: "a".into(),
            topic: "t".into(),
            payload: "p".into(),
        },
    );
    let mut frame = encode_event(&ev);
    // Clobber the magic prefix — a foreign / corrupted frame.
    frame[0] ^= 0xFF;
    assert!(
        decode_event(&frame).is_none(),
        "a frame with a broken magic must be rejected, not mis-parsed"
    );

    // A truncated frame is likewise rejected (no silent partial parse).
    let good = encode_event(&ev);
    assert!(decode_event(&good[..good.len() - 1]).is_none());
    // Trailing garbage is rejected too.
    let mut over = encode_event(&ev);
    over.push(0x00);
    assert!(decode_event(&over).is_none());
}

#[test]
fn sabotage_dropping_a_frame_diverges() {
    let trace = known_trace();
    let bt = recorder::record_trace(&trace);
    let original = recorder::replay_events(&bt).expect("decodes");

    // Drop one frame from the replayed sequence (a lost instant).
    let mut frames = recorder::replay(&bt);
    frames.remove(4);
    let after: Vec<RecordedEvent> = frames
        .iter()
        .map(|f| decode_event(f).expect("remaining frames still decode"))
        .collect();

    // The stream is shorter and no longer matches the recorded history.
    assert_eq!(after.len(), original.len() - 1);
    assert_ne!(after, original, "a dropped frame must diverge from the original");
}

#[test]
fn fork_produces_an_independent_branch() {
    let trace = known_trace();
    let mut bt = recorder::record_trace(&trace);

    let original_head = bt.timeline.head().expect("recorded a head");
    let original_frames = recorder::replay(&bt);
    let blobs_before = bt.recorder.blob_count();

    // FREE FORK: shares the whole past, copies nothing.
    let mut branch = recorder::fork(&bt);
    assert_eq!(branch.head(), Some(original_head), "fork starts at the same head");
    assert_eq!(
        bt.recorder.blob_count(),
        blobs_before,
        "forking must copy no data"
    );

    // Record one genuinely-new event on the branch only.
    let extra = RecordedEvent {
        seq: trace.len() as u64,
        kind: "emit".into(),
        from: "branch".into(),
        topic: "branch.divergence".into(),
        payload: "only on the fork".into(),
        note: String::new(),
    };
    recorder::record_onto(&mut bt, &mut branch, &extra);

    // Branch diverged; original head AND replay are untouched.
    assert_ne!(branch.head(), Some(original_head), "branch must diverge");
    assert_eq!(bt.timeline.head(), Some(original_head), "original head intact");
    assert_eq!(recorder::replay(&bt), original_frames, "original replay intact");

    // The branch has exactly one more frame, and it decodes to the extra event.
    let branch_frames = branch.frames(&bt.recorder);
    assert_eq!(branch_frames.len(), original_frames.len() + 1);
    let last = decode_event(branch_frames.last().unwrap()).expect("branch frame decodes");
    assert_eq!(last, extra);

    // Fork is FREE: exactly one new delta blob entered the store.
    assert_eq!(
        bt.recorder.blob_count(),
        blobs_before + 1,
        "fork + one append adds exactly one delta blob"
    );
}

#[test]
fn stats_are_consistent_and_engine_derived() {
    let trace = known_trace();
    let bt = recorder::record_trace(&trace);
    let s = recorder::stats(&bt);

    // Frame count == event count.
    assert_eq!(s.frames, trace.len());
    // Raw bytes == sum of the actual frame lengths we recorded.
    let raw: usize = bt.frames.iter().map(|f| f.len()).sum();
    assert_eq!(s.raw_bytes, raw);

    // The honest JOINT stored footprint (no per-blob framing overhead) does not
    // exceed the raw cost for this structured, compressible stream — collapse >= 1.
    assert!(
        s.stored_joint <= s.raw_bytes,
        "joint stored {} must be <= raw {}",
        s.stored_joint,
        s.raw_bytes
    );
    assert!(s.collapse_ratio >= 1.0, "collapse must be >= 1x");

    // The root is a real 64-hex-char BLAKE3 digest, not the empty sentinel.
    assert_ne!(s.root_hex, "(empty)");
    assert_eq!(s.root_hex.len(), 64);
    assert!(s.root_hex.chars().all(|c| c.is_ascii_hexdigit()));
}

#[test]
fn root_changes_when_content_changes_and_is_stable_otherwise() {
    let trace = known_trace();

    // Recording the SAME trace twice yields the SAME root (deterministic identity).
    let a = recorder::stats(&recorder::record_trace(&trace)).root_hex;
    let b = recorder::stats(&recorder::record_trace(&trace)).root_hex;
    assert_eq!(a, b, "same trace must produce the same Merkle/timeline root");

    // Changing ANY content (one payload byte) changes the root.
    let mut altered = trace.clone();
    altered[1] = Trace::Emit {
        from: "ping".into(),
        topic: "ping".into(),
        payload: "ping#2".into(), // was ping#1
    };
    let c = recorder::stats(&recorder::record_trace(&altered)).root_hex;
    assert_ne!(a, c, "changed content must change the root");

    // Reordering events also changes the root (the timeline is ORDERED — the seq
    // and the temporal chain both feed the node hash).
    let mut reordered = trace.clone();
    reordered.swap(1, 4);
    let d = recorder::stats(&recorder::record_trace(&reordered)).root_hex;
    assert_ne!(a, d, "reordering events must change the root");
}

#[test]
fn empty_trace_records_an_empty_timeline() {
    let bt = recorder::record_trace(&[]);
    assert!(bt.timeline.head().is_none());
    assert!(recorder::replay(&bt).is_empty());
    let s = recorder::stats(&bt);
    assert_eq!(s.frames, 0);
    assert_eq!(s.raw_bytes, 0);
    assert_eq!(s.collapse_ratio, 1.0);
    assert_eq!(s.root_hex, "(empty)");
}
