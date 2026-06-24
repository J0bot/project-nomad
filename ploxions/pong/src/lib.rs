//! `pong` ploxion — requires the "ping" topic.
//!
//! On each "ping" delivered via `plc_on_event` it increments a counter and
//! emits "pong" carrying the running count. This is the other half of the
//! end-to-end fabric proof: an event emitted by `ping`, routed by the host bus,
//! crossing into a *separate* WASM sandbox, and producing a reaction.

use std::sync::atomic::{AtomicU32, Ordering};

use ploxion_sdk::{emit, export_manifest, log, read_args};

// PLC manifest. requires "ping"; provides "pong".
export_manifest!(
    r#"{"id":"pong","version":"1.0.0","provides":["pong"],"requires":["ping"],"children_types":[],"parent_types":[]}"#
);

static COUNT: AtomicU32 = AtomicU32::new(0);

#[no_mangle]
pub extern "C" fn plc_init() {
    log("pong: init (waiting for pings)");
}

#[no_mangle]
pub extern "C" fn plc_health() -> i32 {
    0
}

/// On each "ping", bump the counter and emit "pong".
#[no_mangle]
pub extern "C" fn plc_on_event(
    topic_ptr: i32,
    topic_len: i32,
    payload_ptr: i32,
    payload_len: i32,
) {
    let topic = unsafe { read_args(topic_ptr, topic_len) };
    if topic != b"ping" {
        return;
    }
    let incoming = unsafe { read_args(payload_ptr, payload_len) };
    let incoming = String::from_utf8_lossy(incoming);
    let n = COUNT.fetch_add(1, Ordering::Relaxed) + 1;
    log(&format!("pong: got '{incoming}' -> count={n}"));
    let payload = format!("pong#{n}");
    emit("pong", payload.as_bytes());
}

#[no_mangle]
pub extern "C" fn plc_goodbye() {
    let n = COUNT.load(Ordering::Relaxed);
    log(&format!("pong: goodbye (served {n} pings)"));
}
