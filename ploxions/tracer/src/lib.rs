//! `tracer` ploxion — a passive observer.
//!
//! It `requires` the common topics ("tick", "ping", "pong") so the host
//! auto-subscribes it, and on every delivery it just logs. It emits nothing.
//! This demonstrates that the bus delivers the SAME event to multiple
//! subscribers, and that an observer ploxion can watch the fabric without
//! affecting it (it never emits, so it can't perturb the flow).

use ploxion_sdk::{export_manifest, log, ploxion_lifecycle, read_args};

export_manifest!(
    r#"{"id":"tracer","version":"1.0.0","provides":[],"requires":["tick","ping","pong"],"children_types":[],"parent_types":[]}"#
);

ploxion_lifecycle!(init: "tracer: init (observing tick/ping/pong)", goodbye: "tracer: goodbye");

#[no_mangle]
pub extern "C" fn plc_on_event(
    topic_ptr: i32,
    topic_len: i32,
    payload_ptr: i32,
    payload_len: i32,
) {
    let topic = unsafe { read_args(topic_ptr, topic_len) };
    let payload = unsafe { read_args(payload_ptr, payload_len) };
    let topic = String::from_utf8_lossy(topic);
    let payload = String::from_utf8_lossy(payload);
    log(&format!("tracer: observed [{topic}] {payload}"));
}

