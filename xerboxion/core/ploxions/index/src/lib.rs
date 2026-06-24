//! `index` ploxion — the FIRST pure CONSUMER that REACTS to the fed data.
//!
//! It has NO capabilities (no `net.fetch` — it never fetches; it only listens to
//! the bus). It `requires` every DATA topic that currently has no consumer, so
//! the host auto-subscribes it to all of them. On each `plc_on_event` it bumps a
//! per-topic counter, remembers the last topic seen, and emits `index.summary` —
//! a compact JSON of per-topic counts + total + last topic.
//!
//! `index.summary` has no consumer (and `index` does NOT require it), so it goes
//! to trace only: observable in /snapshot + /events, no self-loop. Closes gap d
//! in the qu-est-ce-qui-va-ou map (nothing reacted to the fed data before).

#![allow(clippy::missing_safety_doc)]

use std::cell::RefCell;
use std::collections::BTreeMap;

use ploxion_sdk::{emit, export_manifest, log, read_args};

// Pure consumer: capabilities=[], provides=[index.summary], requires=the data
// topics that today have NO consumer. MUST NOT require index.summary (no self-loop).
export_manifest!(
    r#"{"id":"index","version":"1.0.0","capabilities":[],"provides":["index.summary"],"requires":["repo.item","repo.list","git.list","pin.list","osiris.land.quake","osiris.air.track","osiris.land.fire","osiris.conflict.zone","osiris.cyber.cve","osiris.alert"],"children_types":[],"parent_types":[]}"#
);

/// The index's whole state, in its own wasm linear memory (isolated per ploxion):
/// per-topic event counts, the running total, and the last topic seen. Kept
/// across events via a `thread_local!` static (WASM is single-threaded, so this
/// is effectively a process-wide static — the same idiom the watcher uses).
#[derive(Default)]
struct State {
    counts: BTreeMap<String, u32>,
    total: u32,
    last_topic: String,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

/// Build the compact `index.summary` payload by hand (no serde), mirroring how
/// the adapters assemble payload strings, e.g.
/// `{"total":4,"last":"alert","counts":{"alert":1,"repo.item":3}}`.
/// Keys are connector-authored topic names (no embedded quotes), so a flat
/// hand-rolled encoder is enough.
fn summary_json(s: &State) -> String {
    let mut out = String::from("{\"total\":");
    out.push_str(&s.total.to_string());
    out.push_str(",\"last\":\"");
    out.push_str(&s.last_topic);
    out.push_str("\",\"counts\":{");
    let mut first = true;
    for (topic, n) in &s.counts {
        if !first {
            out.push(',');
        }
        first = false;
        out.push('"');
        out.push_str(topic);
        out.push_str("\":");
        out.push_str(&n.to_string());
    }
    out.push_str("}}");
    out
}

// --- PLC lifecycle ----------------------------------------------------------

#[no_mangle]
pub extern "C" fn plc_init() {
    STATE.with(|s| *s.borrow_mut() = State::default());
    log("index: init (live + listening to all unconsumed data topics; provides index.summary)");
}

/// Always OK (0): the index is a passive tallier; it has no failure mode tied to
/// the data it observes.
#[no_mangle]
pub extern "C" fn plc_health() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn plc_on_event(
    topic_ptr: i32,
    topic_len: i32,
    payload_ptr: i32,
    payload_len: i32,
) {
    let topic = unsafe { read_args(topic_ptr, topic_len) };
    let topic = String::from_utf8_lossy(topic).into_owned();

    // Safety: index must never react to its own output (manifest doesn't require
    // it, but guard anyway so a future wiring change can't create a self-loop).
    if topic == "index.summary" {
        return;
    }

    // The payload is read (the index "sees" the fed data) but its content is not
    // surfaced in the summary, which is intentionally just counts + total + last.
    let _payload = unsafe { read_args(payload_ptr, payload_len) };

    let summary = STATE.with(|s| {
        let mut s = s.borrow_mut();
        *s.counts.entry(topic.clone()).or_insert(0) += 1;
        s.total += 1;
        s.last_topic = topic.clone();
        summary_json(&s)
    });

    log(&format!("index: tallied {topic} -> {summary}"));
    emit("index.summary", summary.as_bytes());
}

#[no_mangle]
pub extern "C" fn plc_goodbye() {
    let total = STATE.with(|s| s.borrow().total);
    log(&format!("index: goodbye (tallied {total} data event(s))"));
    STATE.with(|s| *s.borrow_mut() = State::default());
}
