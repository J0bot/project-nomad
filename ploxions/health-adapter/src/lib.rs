//! `health-adapter` ploxion — a **pure-WASM service adapter** (PLC v1.1).
//!
//! This is the payoff of consented capabilities: a service is bridged to the
//! XERB0XI0N bus by a **sandboxed WASM ploxion**, not by native host code. The
//! adapter declares `capabilities:["net.fetch"]` in its manifest, so the host —
//! and ONLY for this ploxion — links the gated `plc_fetch` import into its
//! `Store`. The adapter then performs a REAL HTTP GET *through the host* (it
//! still cannot open a socket itself; it asks the host, which brokers the I/O)
//! and emits one `service.health` `{id,url,code,up}` event per probed URL —
//! exactly the same topic and payload shape the NATIVE `service-connector`
//! emits. So the existing `watcher` ploxion reacts to it unchanged: it cannot
//! tell whether the health came from native host code or from this sandboxed
//! adapter. That is option (A) from the services oracle, realised.
//!
//! It probes targets two ways:
//!   - on `plc_init`, a built-in default set of health URLs (the demo points
//!     these at the live VPS endpoints), and
//!   - on a `health.check` `{"id":..,"url":..}` trigger event on the bus, so the
//!     host/demo can drive an arbitrary URL at runtime.
//!
//! A compliant ploxion that did NOT declare `net.fetch` would simply never call
//! `plc_fetch`; one that *references* the symbol without declaring it is
//! rejected by the host at load. The gate is the host's, not ours.

#![allow(clippy::missing_safety_doc)]

use ploxion_sdk::{emit, export_manifest, fetch_get, log, read_args};
use ploxion_sdk::bions::{json_str, json_uint as json_u64};

// Capabilities: net.fetch (gated host import). Provides service.health (same as
// the native connector). Requires health.check (a runtime trigger to probe a
// given url). The host links plc_fetch ONLY because "net.fetch" is declared.
export_manifest!(
    r#"{"id":"health-adapter","version":"1.0.0","capabilities":["net.fetch"],"provides":["service.health"],"requires":["health.check"],"children_types":[],"parent_types":[]}"#
);

/// The built-in default targets probed on init. The IDs match the live VPS
/// ploxions these endpoints belong to; the demo relies on these being reachable.
/// (id, url)
const DEFAULTS: &[(&str, &str)] = &[
    ("filesystem", "http://127.0.0.1:3010/health"),
    ("tsoin-svc", "http://127.0.0.1:3011/health"),
];

// --- minimal JSON field extraction (same style as the watcher) --------------

/// Probe one target THROUGH the host (`plc_fetch`), then emit a `service.health`
/// event in the connector's shape `{"id","url","code","up"}`. This is the whole
/// adapter: fetch (brokered) -> normalise -> emit onto the bus.
fn probe_and_emit(id: &str, url: &str) {
    log(&format!("health-adapter: GET {url} (via plc_fetch, capability net.fetch)"));

    // The brokered fetch. The host returns JSON {status, body, error?}. We never
    // see a socket; the host did the I/O because we declared the capability.
    let resp = fetch_get(url);
    let code = json_u64(&resp, "status").unwrap_or(0);
    let up = (200..400).contains(&code);

    if up {
        log(&format!("health-adapter: {id} UP ({code:03})"));
    } else {
        let why = json_str(&resp, "error").unwrap_or_default();
        if why.is_empty() {
            log(&format!("health-adapter: {id} DOWN ({code:03})"));
        } else {
            log(&format!("health-adapter: {id} DOWN ({code:03}) [{why}]"));
        }
    }

    // Emit in EXACTLY the native connector's shape so the watcher reacts as-is.
    let payload = format!("{{\"id\":\"{id}\",\"url\":\"{url}\",\"code\":{code},\"up\":{up}}}");
    emit("service.health", payload.as_bytes());
}

// --- PLC lifecycle ----------------------------------------------------------

/// On init, probe the built-in default targets (real HTTP via the host) and emit
/// a service.health for each. The host dispatches those emits to the watcher.
#[no_mangle]
pub extern "C" fn plc_init() {
    log("health-adapter: init (pure-WASM service adapter, capability net.fetch)");
    for (id, url) in DEFAULTS {
        probe_and_emit(id, url);
    }
}

/// Health: always OK — the adapter is stateless; its job is to bridge, not to
/// judge the fleet (that's the watcher's role).
#[no_mangle]
pub extern "C" fn plc_health() -> i32 {
    0
}

/// On a `health.check` `{"id":..,"url":..}` trigger, probe that one URL and emit
/// its service.health. Lets the host/demo drive an arbitrary URL at runtime
/// (e.g. the live https://j0bot.ch).
#[no_mangle]
pub extern "C" fn plc_on_event(
    topic_ptr: i32,
    topic_len: i32,
    payload_ptr: i32,
    payload_len: i32,
) {
    let topic = unsafe { read_args(topic_ptr, topic_len) };
    if topic != b"health.check" {
        return;
    }
    let payload = unsafe { read_args(payload_ptr, payload_len) };
    let payload = String::from_utf8_lossy(payload);
    let url = match json_str(&payload, "url") {
        Some(u) => u,
        None => {
            log("health-adapter: health.check without a url — ignoring");
            return;
        }
    };
    let id = json_str(&payload, "id").unwrap_or_else(|| "adhoc".to_string());
    probe_and_emit(&id, &url);
}

#[no_mangle]
pub extern "C" fn plc_goodbye() {
    log("health-adapter: goodbye");
}
