//! `ideas-map-adapter` ploxion — a **pure-WASM adapter for a real DEPLOYED
//! service** (PLC v1.1, capability `net.fetch`).
//!
//! Complements the generic `health-adapter`: instead of just up/down, this
//! bridges the **data** of José's live `ideas_map` (Rust/Axum, the deployed
//! ploxion at z5qc9….j0bot.ch) onto the XERB0XI0N bus. It declares
//! `capabilities:["net.fetch"]`, so the host links the gated `plc_fetch` import
//! into THIS ploxion's Store only; the adapter does a brokered HTTP GET of
//! `/api/pins` (it never opens a socket — it asks the host) and emits a
//! `pin.list` `{url,code,count,up}` event. `count` is derived tolerantly from
//! the raw response (one `"lng"` per pin), so no fragile nested-JSON unescaping.
//!
//! Driven two ways: on `plc_init` (probe the default live endpoint) and on a
//! `pin.refresh` `{"url":..}` trigger on the bus (re-probe / point elsewhere).
//! A ploxion that referenced `plc_fetch` without declaring `net.fetch` is
//! rejected by the host at load — the gate is the host's.

#![allow(clippy::missing_safety_doc)]

use ploxion_sdk::{emit, export_manifest, fetch_get, log, read_args};
use ploxion_sdk::bions::{json_str, json_uint as json_u64};

export_manifest!(
    r#"{"id":"ideas-map-adapter","version":"1.0.0","capabilities":["net.fetch"],"provides":["pin.list"],"requires":["pin.refresh"],"children_types":[],"parent_types":[]}"#
);

/// Default live endpoint: the deployed ideas_map's public pins API. The host
/// (on the VPS) can reach it; `pin.refresh {"url":..}` overrides at runtime.
const DEFAULT_URL: &str = "https://z5qc9wvdtdtha703yj41k9t6.j0bot.ch/api/pins";

/// Count non-overlapping occurrences of `needle` in `hay`. We count `lng` in the
/// raw fetch result: one geo pin = one `"lng"` field, and the substring survives
/// JSON escaping (`\"lng\"`), so no unescaping is needed.
fn count_substr(hay: &str, needle: &str) -> u64 {
    if needle.is_empty() { return 0; }
    let mut n = 0u64;
    let mut i = 0usize;
    while let Some(p) = hay[i..].find(needle) {
        n += 1;
        i += p + needle.len();
    }
    n
}

/// Probe one ideas-map endpoint THROUGH the host and emit `pin.list`.
fn probe_and_emit(url: &str) {
    log(&format!("ideas-map-adapter: GET {url} (via plc_fetch, capability net.fetch)"));
    let resp = fetch_get(url);
    let code = json_u64(&resp, "status").unwrap_or(0);
    let up = (200..400).contains(&code);
    // tolerant pin count from the raw (possibly escaped) response body
    let count = if up { count_substr(&resp, "lng") } else { 0 };
    log(&format!("ideas-map-adapter: code={code:03} up={up} pins~{count}"));
    let payload = format!("{{\"id\":\"ideas-map\",\"url\":\"{url}\",\"code\":{code},\"count\":{count},\"up\":{up}}}");
    emit("pin.list", payload.as_bytes());
}

#[no_mangle]
pub extern "C" fn plc_init() {
    log("ideas-map-adapter: init (pure-WASM adapter for the LIVE ideas_map, capability net.fetch)");
    probe_and_emit(DEFAULT_URL);
}

#[no_mangle]
pub extern "C" fn plc_health() -> i32 { 0 }

/// On `pin.refresh` `{"url":..}` (url optional -> default), re-probe and emit.
#[no_mangle]
pub extern "C" fn plc_on_event(topic_ptr: i32, topic_len: i32, payload_ptr: i32, payload_len: i32) {
    let topic = unsafe { read_args(topic_ptr, topic_len) };
    if topic != b"pin.refresh" {
        return;
    }
    let payload = unsafe { read_args(payload_ptr, payload_len) };
    let payload = String::from_utf8_lossy(payload);
    let url = json_str(&payload, "url").unwrap_or_else(|| DEFAULT_URL.to_string());
    probe_and_emit(&url);
}

#[no_mangle]
pub extern "C" fn plc_goodbye() {
    log("ideas-map-adapter: goodbye");
}
