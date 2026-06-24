//! # `recorder` — the core records its OWN life as a TSOIN.
//!
//! The project thesis is *« tout est un tsoin »* — the machine à tsoins records
//! everything. This module turns that lens on the runtime **itself**: the core's
//! life is its ordered bus trace (every [`Trace`] hop the [`Host`] journals —
//! `Emit`/`Deliver`/`Log`/`Lifecycle`/`Fetch`/`Recipe`), and that trace, recorded
//! frame-by-frame into a real [`tsoin::Timeline`], becomes a **tsoin timeline of
//! the runtime**: replayable bit-exact, forkable, with measured surprise/collapse.
//!
//! Nothing here is faked. We do not compute a collapse ratio or a Merkle root by
//! hand — we hand each event to the REAL tsoin engine ([`tsoin::Recorder`] /
//! [`tsoin::Timeline`], the git-of-states Merkle DAG) and read the figures back
//! out of it. The only thing this module owns is the **canonical encoding** of one
//! bus event into the bytes of one timeline frame, and its exact inverse.
//!
//! ## What is a frame?
//!
//! One [`Trace`] hop = one frame. The frame is a deterministic, self-describing
//! byte string over the same six-field event model the [`crate::map`] snapshot
//! already exposes — `{seq, kind, from, topic, payload, note}` — so the recording
//! is the snapshot's trace, made temporal:
//!
//! ```text
//! frame = "XBE1" || seq_u64_le
//!              || tag_u8                       (kind discriminant, see TAG_*)
//!              || lp(from) || lp(topic) || lp(payload) || lp(note)
//! lp(s)  = len_u32_le || s_utf8_bytes          (length-prefixed, unambiguous)
//! ```
//!
//! The encoding is **canonical** (fixed field order, fixed-width little-endian
//! lengths, no separators that could collide with content) and **total** (every
//! `Trace` variant maps onto exactly one tag + the six fields, fields that don't
//! apply to a kind are empty). [`decode_event`] is its exact inverse, so a frame
//! round-trips to a [`RecordedEvent`] that re-renders the original hop verbatim.
//!
//! ## How the timeline is built
//!
//! Each encoded frame is recorded as the next instant in one [`tsoin::Timeline`]
//! via [`tsoin::Timeline::record`]. The engine stores only the **delta** from the
//! previous frame (temporal residue) — consecutive bus hops share a lot of
//! structure (same `kind` tag, same `from`/`topic`), so the deltas are sparse and
//! the engine's honest `zstd` footprint collapses well below the naive cost. The
//! `t` axis of the frame's [`tsoin::Coords`] is the event's seq, so the timeline's
//! coordinates ARE the bus's ordering.
//!
//! ## Replay
//!
//! [`replay`] asks the engine to reconstruct every frame ([`tsoin::Timeline::frames`])
//! and returns the raw bytes oldest-first. Because the engine's replay is bit-exact
//! (XOR-residue fold over the Merkle chain), the reconstructed frames are identical
//! to the ones recorded — which we re-decode to assert the EXACT ordered event
//! stream comes back. If it ever did not, [`record::cmd`] would print
//! `replay bit-exact: NO` and exit non-zero; we never paper over a mismatch.

use crate::Trace;
use tsoin::{Coords, Recorder, Timeline};

/// Magic prefix tagging a frame as a v1 xerboxion bus-event encoding. Lets a
/// decoder reject foreign / corrupted bytes loudly instead of mis-parsing them.
pub const FRAME_MAGIC: &[u8; 4] = b"XBE1";

// Kind discriminants. Stable, fixed values — part of the on-the-wire encoding, so
// they must never be reordered (only appended to).
const TAG_EMIT: u8 = 1;
const TAG_DELIVER: u8 = 2;
const TAG_LOG: u8 = 3;
const TAG_LIFECYCLE: u8 = 4;
const TAG_FETCH: u8 = 5;
const TAG_RECIPE: u8 = 6;

/// One bus event reduced to the flat, canonical six-field model — the unit the
/// timeline records and replay reconstructs. This is exactly the shape the
/// [`crate::map`] snapshot's `TraceRow` exposes, so a recorded frame and a
/// snapshot row describe the same hop with the same fields.
///
/// `kind` is the lowercase verb (`emit`/`route`/`log`/`life`/`fetch`/`recipe`);
/// fields that do not apply to a given kind are empty strings. Two `RecordedEvent`s
/// are equal iff their canonical bytes are equal, which is what makes the bit-exact
/// replay assertion meaningful.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordedEvent {
    /// 0-based position of this hop in the host's trace journal.
    pub seq: u64,
    /// Verb: `emit`, `route`, `log`, `life`, `fetch`, or `recipe`.
    pub kind: String,
    /// Acting/source participant id.
    pub from: String,
    /// Bus topic (empty for log/life/recipe).
    pub topic: String,
    /// Payload / line / detail.
    pub payload: String,
    /// Extra note (route target, http status, lifecycle phase, …).
    pub note: String,
}

impl RecordedEvent {
    /// Flatten one host [`Trace`] hop (at journal index `seq`) into the canonical
    /// event. The mapping mirrors [`crate::map`]'s `trace_row` field-for-field, so
    /// the recorded stream and the live map agree on every event.
    #[must_use]
    pub fn from_trace(seq: usize, t: &Trace) -> Self {
        let seq = seq as u64;
        match t {
            Trace::Emit { from, topic, payload } => RecordedEvent {
                seq,
                kind: "emit".into(),
                from: from.clone(),
                topic: topic.clone(),
                payload: payload.clone(),
                note: String::new(),
            },
            Trace::Deliver { from, to, topic } => RecordedEvent {
                seq,
                kind: "route".into(),
                from: from.clone(),
                topic: topic.clone(),
                payload: String::new(),
                note: format!("=> {to}"),
            },
            Trace::Log { from, line } => RecordedEvent {
                seq,
                kind: "log".into(),
                from: from.clone(),
                topic: String::new(),
                payload: line.clone(),
                note: String::new(),
            },
            Trace::Lifecycle { who, what } => RecordedEvent {
                seq,
                kind: "life".into(),
                from: who.clone(),
                topic: String::new(),
                payload: what.clone(),
                note: String::new(),
            },
            Trace::Fetch { from, method, url, status } => RecordedEvent {
                seq,
                kind: "fetch".into(),
                from: from.clone(),
                topic: String::new(),
                payload: format!("{method} {url}"),
                note: format!("status {status}"),
            },
            Trace::Recipe { who, step } => RecordedEvent {
                seq,
                kind: "recipe".into(),
                from: who.clone(),
                topic: String::new(),
                payload: step.clone(),
                note: String::new(),
            },
        }
    }

    /// The kind's stable tag byte. Internal to the encoding.
    fn tag(&self) -> u8 {
        match self.kind.as_str() {
            "emit" => TAG_EMIT,
            "route" => TAG_DELIVER,
            "log" => TAG_LOG,
            "life" => TAG_LIFECYCLE,
            "fetch" => TAG_FETCH,
            "recipe" => TAG_RECIPE,
            // Unknown kinds never occur (the set is closed by `from_trace`); encode
            // as 0 so a decoder can still detect and reject them rather than guess.
            _ => 0,
        }
    }

    /// One-line human rendering — used by the `record` subcommand to print the
    /// reconstructed stream. Mirrors the spirit of [`Trace::render`].
    #[must_use]
    pub fn render(&self) -> String {
        match self.kind.as_str() {
            "emit" => format!("emit   {} -> [{}] {}", self.from, self.topic, self.payload),
            "route" => format!("route  [{}] {} {}", self.topic, self.from, self.note),
            "log" => format!("log    {}: {}", self.from, self.payload),
            "life" => format!("life   {}: {}", self.from, self.payload),
            "fetch" => format!("fetch  {} -> {} ({})", self.from, self.payload, self.note),
            "recipe" => format!("recipe {}: {}", self.from, self.payload),
            other => format!("{other:<6} {} {}", self.from, self.payload),
        }
    }
}

/// Append a length-prefixed (`u32` LE) string to `buf`. Length-prefixing makes the
/// encoding unambiguous: no separator can be confused with content.
fn put_lp(buf: &mut Vec<u8>, s: &str) {
    let bytes = s.as_bytes();
    buf.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
    buf.extend_from_slice(bytes);
}

/// Read a length-prefixed string from `bytes` at `*off`, advancing `*off`. Returns
/// `None` on any truncation or invalid UTF-8 (a corrupted / tampered frame).
fn get_lp(bytes: &[u8], off: &mut usize) -> Option<String> {
    let start = *off;
    let len_end = start.checked_add(4)?;
    if len_end > bytes.len() {
        return None;
    }
    let len = u32::from_le_bytes(bytes[start..len_end].try_into().ok()?) as usize;
    let str_end = len_end.checked_add(len)?;
    if str_end > bytes.len() {
        return None;
    }
    let s = std::str::from_utf8(&bytes[len_end..str_end]).ok()?.to_string();
    *off = str_end;
    Some(s)
}

/// Encode one event into a single, canonical, self-describing timeline frame. The
/// layout is fixed (magic, seq, tag, then four length-prefixed fields) so the same
/// event always produces the same bytes — a prerequisite for deterministic,
/// reproducible recording.
#[must_use]
pub fn encode_event(ev: &RecordedEvent) -> Vec<u8> {
    let mut buf = Vec::with_capacity(
        4 + 8 + 1 + 16 + ev.from.len() + ev.topic.len() + ev.payload.len() + ev.note.len(),
    );
    buf.extend_from_slice(FRAME_MAGIC);
    buf.extend_from_slice(&ev.seq.to_le_bytes());
    buf.push(ev.tag());
    put_lp(&mut buf, &ev.from);
    put_lp(&mut buf, &ev.topic);
    put_lp(&mut buf, &ev.payload);
    put_lp(&mut buf, &ev.note);
    buf
}

/// Decode a frame back into a [`RecordedEvent`] — the exact inverse of
/// [`encode_event`]. Returns `None` if the bytes are not a valid v1 frame (bad
/// magic, unknown tag, truncation, non-UTF-8) — the signal a replayed/loaded frame
/// was corrupted or tampered with, which the sabotage test relies on.
#[must_use]
pub fn decode_event(bytes: &[u8]) -> Option<RecordedEvent> {
    if bytes.len() < 4 + 8 + 1 || &bytes[0..4] != FRAME_MAGIC {
        return None;
    }
    let seq = u64::from_le_bytes(bytes[4..12].try_into().ok()?);
    let tag = bytes[12];
    let kind = match tag {
        TAG_EMIT => "emit",
        TAG_DELIVER => "route",
        TAG_LOG => "log",
        TAG_LIFECYCLE => "life",
        TAG_FETCH => "fetch",
        TAG_RECIPE => "recipe",
        _ => return None, // unknown / corrupted kind tag
    };
    let mut off = 13usize;
    let from = get_lp(bytes, &mut off)?;
    let topic = get_lp(bytes, &mut off)?;
    let payload = get_lp(bytes, &mut off)?;
    let note = get_lp(bytes, &mut off)?;
    // Reject trailing garbage so a tampered, over-long frame does not silently
    // pass as valid.
    if off != bytes.len() {
        return None;
    }
    Some(RecordedEvent {
        seq,
        kind: kind.to_string(),
        from,
        topic,
        payload,
        note,
    })
}

/// The result of recording a trace as a tsoin timeline: the engine's [`Recorder`]
/// (owning the Merkle DAG + content-addressed delta store) and the [`Timeline`]
/// head into it. Kept together because the timeline is only meaningful against the
/// recorder that holds its nodes — and because the [`stats`] / [`fork`] helpers
/// need both.
pub struct BusTsoin {
    /// The engine recorder: the git-of-states Merkle DAG + delta store. THIS is
    /// what holds all the truth; we only ever read figures back out of it.
    pub recorder: Recorder,
    /// The head of the timeline that recorded the bus trace, into `recorder`.
    pub timeline: Timeline,
    /// The canonical frames we handed to the engine, kept so replay can be
    /// asserted byte-equal against exactly what was recorded (the bit-exact check).
    pub frames: Vec<Vec<u8>>,
}

/// Honest, engine-derived statistics about a recorded bus tsoin. Every field is
/// READ from the real [`tsoin::Recorder`] / [`tsoin::Timeline`] — never computed by
/// hand here. The headline `collapse_ratio` is `raw_bytes / stored_joint` straight
/// off the engine's own `naive_cost` and `store_footprint_joint`.
#[derive(Debug, Clone)]
pub struct TsoinStats {
    /// Number of frames (= bus events) recorded, from the engine's node count.
    pub frames: usize,
    /// Naive cost: sum of full frame lengths (what storing each instant in full
    /// would cost), from [`tsoin::Timeline::naive_cost`].
    pub raw_bytes: usize,
    /// Raw stored footprint: the sum of the distinct delta blob lengths before any
    /// compression, from [`tsoin::Recorder::store_footprint_raw`]. Shown for
    /// transparency (deltas are length-preserving; the win is they're sparse).
    pub stored_raw: usize,
    /// Per-blob honest footprint: each distinct delta blob zstd-19'd independently,
    /// from [`tsoin::Recorder::store_footprint_compressed`]. Conservative — it
    /// charges a per-blob compressor framing overhead to every tiny delta.
    pub stored_compressed: usize,
    /// Joint honest footprint: every distinct delta blob concatenated and zstd-19'd
    /// as ONE stream, from [`tsoin::Recorder::store_footprint_joint`]. Removes the
    /// per-blob framing overhead, so it reflects the true joint surprise of the
    /// whole timeline — the cleanest "sum of small deltas" figure, and the one the
    /// headline ratio uses. The REAL cost.
    pub stored_joint: usize,
    /// `raw_bytes / stored_joint`. > 1 means the timeline collapsed (sparse
    /// surprise); `~= 1` means an incompressible stream (no magic). Derived from
    /// the two engine figures above, nothing else.
    pub collapse_ratio: f64,
    /// Distinct delta blobs in the store (content-addressed), from
    /// [`tsoin::Recorder::blob_count`].
    pub blob_count: usize,
    /// Exactly-identical instants the engine detected (José's xerboxion popup),
    /// from [`tsoin::Recorder::xerboxion_count`].
    pub xerboxions: usize,
    /// The timeline's head node hash = its Merkle/timeline ROOT identity (the hash
    /// over the whole chain of deltas + coords). `"(empty)"` if no frame recorded.
    pub root_hex: String,
}

/// Record an ordered slice of host [`Trace`] events as a tsoin timeline through
/// the REAL engine. Each event is canonicalized ([`encode_event`]) and recorded as
/// one frame via [`tsoin::Timeline::record`]; the engine stores only the delta from
/// the previous frame. Returns the recorder + timeline head + the exact frames fed
/// in (for the bit-exact replay assertion).
///
/// The frame's [`tsoin::Coords`] `t` axis is the event's journal index, so the
/// timeline's coordinates encode the bus ordering itself.
#[must_use]
pub fn record_trace(trace: &[Trace]) -> BusTsoin {
    let mut recorder = Recorder::new();
    let mut timeline = Timeline::new();
    let mut frames = Vec::with_capacity(trace.len());

    for (seq, t) in trace.iter().enumerate() {
        let ev = RecordedEvent::from_trace(seq, t);
        let frame = encode_event(&ev);
        let coords = Coords {
            t: seq as u64,
            ..Coords::default()
        };
        timeline.record(&mut recorder, &frame, coords);
        frames.push(frame);
    }

    BusTsoin {
        recorder,
        timeline,
        frames,
    }
}

/// CHEAP root-only derivation: record `trace` as a tsoin timeline and return ONLY
/// the timeline head hash (hex), the empty string for an empty trace.
///
/// This is the hot-path counterpart to `record_trace` + [`stats`]: it does the same
/// BLAKE3/delta recording but DELIBERATELY skips every footprint figure in
/// [`stats`] (`store_footprint_compressed`/`store_footprint_joint`/`naive_cost`),
/// which run zstd-19 over the WHOLE ever-growing delta store and are O(trace) per
/// call. `snapshot_of` only consumes the root, so the footprints were pure waste on
/// the snapshot hot path. The tsoin RECORDER (the bus journal) is untouched — this
/// only derives the response field.
#[must_use]
pub fn trace_root(trace: &[Trace]) -> String {
    let bt = record_trace(trace);
    bt.timeline
        .head()
        .map(|h| h.to_hex())
        .unwrap_or_default()
}

/// Reconstruct every recorded frame from the timeline, oldest first, via the
/// engine's bit-exact replay ([`tsoin::Timeline::frames`]). The returned bytes are
/// the engine's reconstruction — compare them to [`BusTsoin::frames`] to prove the
/// round trip is bit-exact.
#[must_use]
pub fn replay(bt: &BusTsoin) -> Vec<Vec<u8>> {
    bt.timeline.frames(&bt.recorder)
}

/// Reconstruct + decode every frame back into the ordered [`RecordedEvent`] stream.
/// `None` if any frame fails to decode (corruption). This is the reflexive payoff:
/// the exact ordered bus events the core lived, reconstructed from its tsoin.
#[must_use]
pub fn replay_events(bt: &BusTsoin) -> Option<Vec<RecordedEvent>> {
    replay(bt).iter().map(|f| decode_event(f)).collect()
}

/// Read honest statistics straight out of the engine. Nothing is hand-computed
/// except the ratio, which is the quotient of two engine figures.
#[must_use]
pub fn stats(bt: &BusTsoin) -> TsoinStats {
    let frames = bt.recorder.node_count();
    let raw_bytes = bt.timeline.naive_cost(&bt.recorder);
    let stored_raw = bt.recorder.store_footprint_raw();
    let stored_compressed = bt.recorder.store_footprint_compressed();
    let stored_joint = bt.recorder.store_footprint_joint();
    let collapse_ratio = if raw_bytes == 0 || stored_joint == 0 {
        // An empty timeline (no events) has nothing to collapse; the ratio is
        // undefined, so report 1.0 (no collapse) rather than 0/x or x/0. Note an
        // empty store still zstd-frames to a few bytes, so guard on raw==0 too.
        1.0
    } else {
        raw_bytes as f64 / stored_joint as f64
    };
    let root_hex = bt
        .timeline
        .head()
        .map(|h| h.to_hex())
        .unwrap_or_else(|| "(empty)".to_string());
    TsoinStats {
        frames,
        raw_bytes,
        stored_raw,
        stored_compressed,
        stored_joint,
        collapse_ratio,
        blob_count: bt.recorder.blob_count(),
        xerboxions: bt.recorder.xerboxion_count(),
        root_hex,
    }
}

/// FREE FORK of a recorded bus tsoin: a second [`Timeline`] head sharing the whole
/// recorded past, into the SAME recorder. The fork diverges only when something is
/// recorded onto it — the original head is untouched. Returns the forked timeline;
/// record onto it with [`record_onto`].
#[must_use]
pub fn fork(bt: &BusTsoin) -> Timeline {
    bt.timeline.fork()
}

/// Record one more canonical event onto a (typically forked) timeline, into `bt`'s
/// recorder, advancing that timeline's head. Used to show a branch diverging from
/// the original while the original stays intact. Returns the encoded frame.
pub fn record_onto(bt: &mut BusTsoin, branch: &mut Timeline, ev: &RecordedEvent) -> Vec<u8> {
    let frame = encode_event(ev);
    // Continue the t axis past the original's last frame.
    let coords = Coords {
        t: ev.seq,
        ..Coords::default()
    };
    branch.record(&mut bt.recorder, &frame, coords);
    frame
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A small, representative trace exercising every Trace variant — used by the
    /// unit tests here and mirrored by the integration tests.
    fn sample_trace() -> Vec<Trace> {
        vec![
            Trace::Lifecycle { who: "ping".into(), what: "init".into() },
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
            Trace::Log { from: "pong".into(), line: "got ping".into() },
            Trace::Fetch {
                from: "health-adapter".into(),
                method: "GET".into(),
                url: "https://fs.j0bot.ch".into(),
                status: 200,
            },
            Trace::Recipe {
                who: "recipe:alerter".into(),
                step: "FIRE -> emit [alert.notify] down".into(),
            },
        ]
    }

    #[test]
    fn encode_decode_roundtrips_every_variant() {
        let trace = sample_trace();
        for (seq, t) in trace.iter().enumerate() {
            let ev = RecordedEvent::from_trace(seq, t);
            let bytes = encode_event(&ev);
            let back = decode_event(&bytes).expect("valid frame decodes");
            assert_eq!(ev, back, "event must round-trip through its frame bytes");
        }
    }

    #[test]
    fn encoding_is_canonical_and_stable() {
        let ev = RecordedEvent::from_trace(
            7,
            &Trace::Emit {
                from: "a".into(),
                topic: "t".into(),
                payload: "p".into(),
            },
        );
        // Same event => identical bytes, every time.
        assert_eq!(encode_event(&ev), encode_event(&ev));
        // The frame starts with the magic + the seq.
        let b = encode_event(&ev);
        assert_eq!(&b[0..4], FRAME_MAGIC);
        assert_eq!(u64::from_le_bytes(b[4..12].try_into().unwrap()), 7);
    }

    #[test]
    fn record_then_replay_is_bit_exact() {
        let trace = sample_trace();
        let bt = record_trace(&trace);
        let replayed = replay(&bt);
        assert_eq!(replayed, bt.frames, "engine replay must be byte-identical");
        // And the decoded stream equals the original events in order.
        let events = replay_events(&bt).expect("frames decode");
        let expected: Vec<RecordedEvent> = trace
            .iter()
            .enumerate()
            .map(|(i, t)| RecordedEvent::from_trace(i, t))
            .collect();
        assert_eq!(events, expected);
    }

    #[test]
    fn stats_come_from_the_engine_and_are_consistent() {
        let trace = sample_trace();
        let bt = record_trace(&trace);
        let s = stats(&bt);
        assert_eq!(s.frames, trace.len());
        assert_eq!(s.raw_bytes, bt.frames.iter().map(|f| f.len()).sum::<usize>());
        // The JOINT honest footprint (no per-blob framing overhead) must not exceed
        // the raw cost for this structured, compressible stream — so collapse >= 1.
        assert!(
            s.stored_joint <= s.raw_bytes,
            "joint {} > raw {}",
            s.stored_joint,
            s.raw_bytes
        );
        assert!(s.collapse_ratio >= 1.0);
        assert_ne!(s.root_hex, "(empty)");
    }

    #[test]
    fn fork_diverges_while_original_intact() {
        let trace = sample_trace();
        let mut bt = record_trace(&trace);
        let original_head = bt.timeline.head().expect("recorded");
        let original_frames = replay(&bt);

        let mut branch = fork(&bt);
        // Forking copies nothing and the branch starts at the same head.
        assert_eq!(branch.head(), Some(original_head));

        // Record one more event on the branch only.
        let extra = RecordedEvent {
            seq: trace.len() as u64,
            kind: "emit".into(),
            from: "branch".into(),
            topic: "branch.topic".into(),
            payload: "divergent".into(),
            note: String::new(),
        };
        record_onto(&mut bt, &mut branch, &extra);

        // Branch head moved; original head did not.
        assert_ne!(branch.head(), Some(original_head));
        assert_eq!(bt.timeline.head(), Some(original_head));
        // Original still replays to exactly its frames (unchanged).
        assert_eq!(replay(&bt), original_frames);
        // Branch has one extra frame, and it decodes to the extra event.
        let branch_frames = branch.frames(&bt.recorder);
        assert_eq!(branch_frames.len(), original_frames.len() + 1);
        let last = decode_event(branch_frames.last().unwrap()).expect("branch frame decodes");
        assert_eq!(last, extra);
    }
}
