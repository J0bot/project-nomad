//! `ping` ploxion — provides the "ping" topic.
//!
//! Lifecycle: on `plc_init` it emits one "ping"; it also subscribes to the host
//! "tick" topic, and on each tick emits another "ping". This proves a ploxion
//! can both react to the bus and source events onto it.

use std::cell::Cell;
use std::sync::atomic::{AtomicU32, Ordering};

use ploxion_sdk::{emit, export_manifest, log, read_args};

// PLC manifest. provides "ping"; requires "tick" (host drives ticks).
export_manifest!(
    r#"{"id":"ping","version":"1.0.0","provides":["ping"],"requires":["tick"],"children_types":[],"parent_types":[]}"#
);

static SEQ: AtomicU32 = AtomicU32::new(0);

thread_local! {
    static INIT_DONE: Cell<bool> = const { Cell::new(false) };
}

fn send_ping() {
    let n = SEQ.fetch_add(1, Ordering::Relaxed) + 1;
    let payload = format!("ping#{n}");
    log(&format!("ping: emitting {payload}"));
    emit("ping", payload.as_bytes());
}

/// Called once by the host. Emit an initial ping.
#[no_mangle]
pub extern "C" fn plc_init() {
    INIT_DONE.with(|c| c.set(true));
    log("ping: init");
    send_ping();
}

/// Health: ok only after init ran.
#[no_mangle]
pub extern "C" fn plc_health() -> i32 {
    if INIT_DONE.with(|c| c.get()) {
        0
    } else {
        1
    }
}

/// On a host "tick", emit another ping.
#[no_mangle]
pub extern "C" fn plc_on_event(
    topic_ptr: i32,
    topic_len: i32,
    _payload_ptr: i32,
    _payload_len: i32,
) {
    let topic = unsafe { read_args(topic_ptr, topic_len) };
    if topic == b"tick" {
        send_ping();
    }
}

/// Clean shutdown.
#[no_mangle]
pub extern "C" fn plc_goodbye() {
    log("ping: goodbye");
}
