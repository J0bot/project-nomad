//! `watcher` ploxion — a sandboxed WASM observer of the LIVE deployed services.
//!
//! It `requires` the single topic `"service.health"`, so the host auto-subscribes
//! it. The host's **native** `service-connector` (which is allowed to touch the
//! network — this ploxion is NOT) probes the real deployed services from
//! `runtime.json` and emits one `service.health` `{id,url,code,up}` event per
//! service onto the bus. The watcher reacts to each:
//!
//! - logs `watcher: service <id> is UP (200)` / `DOWN (000)`,
//! - keeps running counts of UP vs DOWN services (in its OWN sandbox memory),
//! - emits an `"alert"` event when a service is DOWN.
//!
//! This is the whole point of the split: the watcher is pure, isolated compute
//! that nonetheless *reacts to the real world* — the real world reaches it only
//! as opaque bus payloads bridged in by the trusted native side. The watcher
//! cannot itself open a socket; it doesn't need to.

#![allow(clippy::missing_safety_doc)]

use std::cell::RefCell;

use ploxion_sdk::{emit, export_manifest, log, read_args};
use ploxion_sdk::bions::{json_str, json_uint as json_u64};

// Requires the connector's topic; provides "alert" (emitted on a DOWN service).
export_manifest!(
    r#"{"id":"watcher","version":"1.0.0","provides":["alert"],"requires":["service.health"],"children_types":[],"parent_types":[]}"#
);

/// The watcher's whole state: how many services it has seen UP vs DOWN. Lives in
/// this ploxion's own wasm linear memory; isolated from every other ploxion.
#[derive(Default)]
struct Counts {
    up: u32,
    down: u32,
}

thread_local! {
    static COUNTS: RefCell<Counts> = RefCell::new(Counts::default());
}

// --- minimal JSON field extraction (payload is flat + connector-authored) ----

/// Extract a boolean value for `"key"`, e.g. `{"up":true}` -> `Some(true)`.
fn json_bool(json: &str, key: &str) -> Option<bool> {
    let needle = format!("\"{key}\"");
    let start = json.find(&needle)? + needle.len();
    let rest = &json[start..];
    let colon = rest.find(':')?;
    let after = rest[colon + 1..].trim_start();
    if after.starts_with("true") {
        Some(true)
    } else if after.starts_with("false") {
        Some(false)
    } else {
        None
    }
}

// --- PLC lifecycle ----------------------------------------------------------

#[no_mangle]
pub extern "C" fn plc_init() {
    COUNTS.with(|c| *c.borrow_mut() = Counts::default());
    log("watcher: init (watching service.health from the live deployed services)");
}

/// Health: degraded (2) iff at least one watched service is currently DOWN, else
/// OK (0). So the watcher's own health mirrors the fleet it observes.
#[no_mangle]
pub extern "C" fn plc_health() -> i32 {
    COUNTS.with(|c| if c.borrow().down > 0 { 2 } else { 0 })
}

#[no_mangle]
pub extern "C" fn plc_on_event(
    topic_ptr: i32,
    topic_len: i32,
    payload_ptr: i32,
    payload_len: i32,
) {
    let topic = unsafe { read_args(topic_ptr, topic_len) };
    if topic != b"service.health" {
        return; // the watcher only reacts to service.health
    }
    let payload = unsafe { read_args(payload_ptr, payload_len) };
    let payload = String::from_utf8_lossy(payload);

    let id = json_str(&payload, "id").unwrap_or_else(|| "?".to_string());
    let code = json_u64(&payload, "code").unwrap_or(0);
    // Trust the connector's `up` flag if present; else derive from the code.
    let up = json_bool(&payload, "up").unwrap_or((200..400).contains(&code));

    let (up_n, down_n) = COUNTS.with(|c| {
        let mut c = c.borrow_mut();
        if up {
            c.up += 1;
        } else {
            c.down += 1;
        }
        (c.up, c.down)
    });

    if up {
        log(&format!(
            "watcher: service {id} is UP ({code:03}) [up={up_n} down={down_n}]"
        ));
    } else {
        log(&format!(
            "watcher: service {id} is DOWN ({code:03}) [up={up_n} down={down_n}]"
        ));
        // React: raise an alert on the bus for the down service.
        let alert = format!("{{\"service\":\"{id}\",\"code\":{code},\"down\":true}}");
        emit("alert", alert.as_bytes());
    }
}

#[no_mangle]
pub extern "C" fn plc_goodbye() {
    let (up, down) = COUNTS.with(|c| {
        let c = c.borrow();
        (c.up, c.down)
    });
    log(&format!(
        "watcher: goodbye (saw {up} UP, {down} DOWN service health event(s))"
    ));
    COUNTS.with(|c| *c.borrow_mut() = Counts::default());
}
