//! `gitea-adapter` ploxion — pure-WASM adapter for the LIVE gitea instance
//! (git.j0bot.ch), PLC v1.1 capability `net.fetch`. 3rd real deployed service
//! on the bus (sibling of ideas-map/repoverse-adapter). Brokered `plc_fetch`
//! GET of `/api/v1/repos/search` -> emit `git.list` `{url,code,count,up}`
//! (count tolerant = `"full_name"` per repo). Re-probe on `git.refresh`.
#![allow(clippy::missing_safety_doc)]

use ploxion_sdk::{emit, export_manifest, fetch_get, log, read_args};
use ploxion_sdk::bions::{json_str, json_uint as json_u64};

export_manifest!(
    r#"{"id":"gitea-adapter","version":"1.0.0","capabilities":["net.fetch"],"provides":["git.list"],"requires":["git.refresh"],"children_types":[],"parent_types":[]}"#
);

const DEFAULT_URL: &str = "https://git.j0bot.ch/api/v1/repos/search?limit=50";

fn count_substr(hay: &str, needle: &str) -> u64 {
    if needle.is_empty() { return 0; }
    let (mut n, mut i) = (0u64, 0usize);
    while let Some(p) = hay[i..].find(needle) { n += 1; i += p + needle.len(); }
    n
}

fn probe_and_emit(url: &str) {
    log(&format!("gitea-adapter: GET {url} (via plc_fetch, capability net.fetch)"));
    let resp = fetch_get(url);
    let code = json_u64(&resp, "status").unwrap_or(0);
    let up = (200..400).contains(&code);
    let count = if up { count_substr(&resp, "full_name") } else { 0 };
    log(&format!("gitea-adapter: code={code:03} up={up} repos~{count}"));
    let payload = format!("{{\"id\":\"gitea\",\"url\":\"{url}\",\"code\":{code},\"count\":{count},\"up\":{up}}}");
    emit("git.list", payload.as_bytes());
}

#[no_mangle]
pub extern "C" fn plc_init() {
    log("gitea-adapter: init (pure-WASM adapter for the LIVE gitea, capability net.fetch)");
    probe_and_emit(DEFAULT_URL);
}
#[no_mangle]
pub extern "C" fn plc_health() -> i32 { 0 }
#[no_mangle]
pub extern "C" fn plc_on_event(topic_ptr: i32, topic_len: i32, payload_ptr: i32, payload_len: i32) {
    let topic = unsafe { read_args(topic_ptr, topic_len) };
    if topic != b"git.refresh" { return; }
    let payload = unsafe { read_args(payload_ptr, payload_len) };
    let payload = String::from_utf8_lossy(payload);
    let url = json_str(&payload, "url").unwrap_or_else(|| DEFAULT_URL.to_string());
    probe_and_emit(&url);
}
#[no_mangle]
pub extern "C" fn plc_goodbye() { log("gitea-adapter: goodbye"); }
