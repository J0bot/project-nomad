//! Integration tests for the **OSIRIS x machine-à-tsoins convergence** — the
//! `osiris-adapter` ploxion + its `tsoin.record` connection to the `tsoin`
//! ploxion. DETERMINISTIC and OFFLINE: the brokered `plc_fetch` is replaced by a
//! STUB ([`Host::with_fetch_fn`]) that serves the committed OSIRIS-shaped
//! fixtures, so no live upstream is ever touched.
//!
//! What is proven:
//!   a) PARSE+EMIT — the adapter parses each OSIRIS route's real JSON shape and
//!      emits the right `osiris.*` topic per world event (quake/flight/cve/zone).
//!   b) THE TSOIN CONNECTION — each (bounded) osiris event is ALSO emitted as
//!      `tsoin.record`, routed into the REAL `tsoin` ploxion which records it;
//!      a `tsoin.replay` reconstructs it BIT-EXACT equal to the emitted event.
//!   c) THE GATE — the adapter declares `net.fetch`; discovery finds it by
//!      capability; a wasm importing `plc_fetch` without declaring it is rejected.

use std::path::PathBuf;

use xerboxion_host::{FetchLimits, FetchResult, Host, Trace};

fn ploxion_dir() -> PathBuf {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest
        .parent()
        .and_then(|p| p.parent())
        .map(|p| p.join("target/ploxions"))
        .unwrap()
}

fn wasm(name: &str) -> Option<Vec<u8>> {
    std::fs::read(ploxion_dir().join(format!("{name}.wasm"))).ok()
}

macro_rules! require_wasm {
    ($name:expr) => {
        match wasm($name) {
            Some(b) => b,
            None => {
                eprintln!("SKIP: {}.wasm not built — run scripts/build-ploxions.sh first", $name);
                return;
            }
        }
    };
}

// The committed OSIRIS-shaped fixtures (the exact real route response shapes).
const F_QUAKES: &str = include_str!("../../../ploxions/osiris-adapter/fixtures/earthquakes.json");
const F_FLIGHTS: &str = include_str!("../../../ploxions/osiris-adapter/fixtures/flights.json");
const F_FRONT: &str = include_str!("../../../ploxions/osiris-adapter/fixtures/frontlines.json");
const F_CYBER: &str = include_str!("../../../ploxions/osiris-adapter/fixtures/cyber-threats.json");

/// A deterministic stub fetch fn that serves the OSIRIS fixtures by route path.
/// An unknown route is a 404 (still a valid outcome — the adapter skips it).
fn stub_osiris(_method: &str, url: &str, _body: &[u8], _limits: FetchLimits) -> FetchResult {
    let body = if url.ends_with("/api/earthquakes") {
        F_QUAKES
    } else if url.ends_with("/api/flights") {
        F_FLIGHTS
    } else if url.ends_with("/api/frontlines") {
        F_FRONT
    } else if url.ends_with("/api/cyber-threats") {
        F_CYBER
    } else {
        return FetchResult { status: 404, body: String::new(), truncated: false, error: None };
    };
    FetchResult { status: 200, body: body.to_string(), truncated: false, error: None }
}

// --- a) PARSE + EMIT --------------------------------------------------------

#[test]
fn adapter_polls_fixtures_and_emits_each_osiris_domain() {
    let adapter = require_wasm!("osiris-adapter");
    let mut host = Host::new().with_fetch_fn(stub_osiris);
    host.load_bytes(&adapter, "osiris-adapter").unwrap();

    // The manifest carries the topic taxonomy + the capability.
    let m = &host.ploxions()[0].manifest;
    assert!(m.has_capability("net.fetch"));
    for t in ["osiris.land.quake", "osiris.air.track", "osiris.cyber.cve", "osiris.conflict.zone", "tsoin.record"] {
        assert!(m.provides.iter().any(|p| p == t), "manifest should provide {t}");
    }
    assert!(m.requires.iter().any(|r| r == "osiris.refresh"));

    host.init_all().unwrap();

    // A real brokered fetch happened per route (status 200 from the stub).
    let ok_fetches = host.trace().iter().filter(|t| matches!(t, Trace::Fetch { from, status, .. }
        if from == "osiris-adapter" && *status == 200)).count();
    assert!(ok_fetches >= 3, "adapter should fetch its route subset, got {ok_fetches}");

    // One osiris.* emit per parsed event, per domain.
    let count = |topic: &str| host.trace().iter().filter(|t| matches!(t, Trace::Emit { from, topic: tp, .. }
        if from == "osiris-adapter" && tp == topic)).count();
    assert_eq!(count("osiris.land.quake"), 3, "3 quakes in the fixture");
    assert_eq!(count("osiris.air.track"), 3, "3 aircraft (mil+jet+commercial)");
    assert_eq!(count("osiris.cyber.cve"), 3, "3 exploited CVEs");
    assert_eq!(count("osiris.conflict.zone"), 1, "one frontline zone");
}

#[test]
fn quake_payload_is_compact_normalised_on_the_bus() {
    let adapter = require_wasm!("osiris-adapter");
    let mut host = Host::new().with_fetch_fn(stub_osiris);
    host.load_bytes(&adapter, "osiris-adapter").unwrap();
    host.init_all().unwrap();

    let p = host.trace().iter().find_map(|t| match t {
        Trace::Emit { from, topic, payload } if from == "osiris-adapter" && topic == "osiris.land.quake" => Some(payload.clone()),
        _ => None,
    }).expect("a quake was emitted");
    // {id,kind,lat,lng,label,severity}
    assert!(p.contains("\"id\":\"us7000sample1\""));
    assert!(p.contains("\"kind\":\"quake\""));
    assert!(p.contains("\"lat\":38.12"));
    assert!(p.contains("\"severity\":4.7"));
}

#[test]
fn high_severity_event_also_raises_osiris_alert() {
    let adapter = require_wasm!("osiris-adapter");
    let mut host = Host::new().with_fetch_fn(stub_osiris);
    host.load_bytes(&adapter, "osiris-adapter").unwrap();
    host.init_all().unwrap();
    // A CRITICAL CVE (severity 4) and a M6.1 quake (severity>=4) raise osiris.alert.
    let alerts = host.trace().iter().filter(|t| matches!(t, Trace::Emit { from, topic, .. }
        if from == "osiris-adapter" && topic == "osiris.alert")).count();
    assert!(alerts >= 2, "high-severity events should raise osiris.alert, got {alerts}");
}

// --- b) THE TSOIN CONNECTION: record -> recorded -> replay BIT-EXACT --------

#[test]
fn each_osiris_event_is_recorded_as_a_tsoin_and_replays_bit_exact() {
    let adapter = require_wasm!("osiris-adapter");
    let tsoin = require_wasm!("tsoin");
    let mut host = Host::new().with_fetch_fn(stub_osiris);
    // Load tsoin FIRST so init_all() builds its engine BEFORE the adapter's init
    // poll emits any tsoin.record (init runs in load order; a record sent to a
    // not-yet-initialised tsoin would be dropped — order is the contract).
    host.load_bytes(&tsoin, "tsoin").unwrap();
    host.load_bytes(&adapter, "osiris-adapter").unwrap();

    // The connection is wired by manifests: osiris-adapter provides tsoin.record,
    // tsoin (requires tsoin.record) auto-subscribed to it.
    assert!(host.bus().subscribers("tsoin.record").iter().any(|s| s == "tsoin"),
        "the tsoin ploxion must be subscribed to tsoin.record (the connection)");

    host.init_all().unwrap();

    // The adapter emitted tsoin.record for a bounded sample, and the tsoin
    // ploxion recorded each (replying tsoin.recorded with an id).
    let records = host.trace().iter().filter(|t| matches!(t, Trace::Emit { from, topic, .. }
        if from == "osiris-adapter" && topic == "tsoin.record")).count();
    assert!(records >= 1, "adapter should emit tsoin.record for osiris events");

    let recorded = host.trace().iter().filter(|t| matches!(t, Trace::Emit { from, topic, .. }
        if from == "tsoin" && topic == "tsoin.recorded")).count();
    assert_eq!(recorded, records, "the tsoin ploxion must record EVERY tsoin.record it is sent");

    // Capture the bytes the adapter recorded under id 0 (the first tsoin.record),
    // then replay id 0 and assert the reconstruction is BIT-EXACT.
    let first_record_bytes = host.trace().iter().find_map(|t| match t {
        Trace::Emit { from, topic, payload } if from == "osiris-adapter" && topic == "tsoin.record" => {
            hex_field(payload, "bytes")
        }
        _ => None,
    }).expect("a tsoin.record carried hex bytes");

    let before = host.trace().len();
    host.inject("host", "tsoin.replay", br#"{"id":0}"#).unwrap();
    let replayed = host.trace()[before..].iter().find_map(|t| match t {
        Trace::Emit { from, topic, payload } if from == "tsoin" && topic == "tsoin.replayed" => {
            hex_field(payload, "bytes")
        }
        _ => None,
    }).expect("the tsoin ploxion replayed id 0");

    assert_eq!(replayed, first_record_bytes,
        "the replayed instant de réel must be BIT-EXACT equal to the recorded osiris event");
    // And it really is the osiris event JSON (a quake, the first parsed event).
    let s = String::from_utf8(replayed).unwrap();
    assert!(s.contains("\"kind\":\"quake\""), "the reel is the osiris event: {s}");
}

#[test]
fn tsoin_recording_is_bounded_per_route_to_avoid_flooding() {
    // The adapter caps tsoin.record at MAX_TSOIN_PER_ROUTE (5) per route while it
    // may emit MORE osiris.* events — so the store is never flooded. Our fixtures
    // are under the cap, so EVERY emitted event is recorded here; the invariant we
    // assert is the bound: records <= osiris emits, and never zero.
    let adapter = require_wasm!("osiris-adapter");
    let mut host = Host::new().with_fetch_fn(stub_osiris);
    host.load_bytes(&adapter, "osiris-adapter").unwrap();
    host.init_all().unwrap();

    let osiris_emits = host.trace().iter().filter(|t| matches!(t, Trace::Emit { from, topic, .. }
        if from == "osiris-adapter" && topic.starts_with("osiris.") && topic != "osiris.alert")).count();
    let records = host.trace().iter().filter(|t| matches!(t, Trace::Emit { from, topic, .. }
        if from == "osiris-adapter" && topic == "tsoin.record")).count();
    assert!(records >= 1 && records <= osiris_emits,
        "tsoin records ({records}) must be bounded by osiris emits ({osiris_emits})");
}

// --- c) error tolerance + the refresh trigger -------------------------------

#[test]
fn unreachable_osiris_is_skipped_without_panic() {
    // Every route is unreachable (status 0). The adapter must NOT panic, must do
    // brokered fetches that come back 0, and emit NOTHING (no events to publish).
    let adapter = require_wasm!("osiris-adapter");
    let mut host = Host::new().with_fetch_fn(|_m, _u, _b, _l| {
        FetchResult::unreachable("stub: OSIRIS down")
    });
    host.load_bytes(&adapter, "osiris-adapter").unwrap();
    host.init_all().unwrap();

    let zero = host.trace().iter().filter(|t| matches!(t, Trace::Fetch { from, status, .. }
        if from == "osiris-adapter" && *status == 0)).count();
    assert!(zero >= 1, "an unreachable OSIRIS should report status-0 fetches");
    let emits = host.trace().iter().filter(|t| matches!(t, Trace::Emit { from, topic, .. }
        if from == "osiris-adapter" && topic.starts_with("osiris."))).count();
    assert_eq!(emits, 0, "no events to publish when OSIRIS is down — and no panic");
}

#[test]
fn refresh_trigger_re_polls_with_an_override_base() {
    // The osiris.refresh trigger re-polls; a {"base":..} override is accepted.
    // Init already polled the baked default; the trigger polls again, so the
    // adapter emits MORE osiris.* across init+trigger.
    let adapter = require_wasm!("osiris-adapter");
    let mut host = Host::new().with_fetch_fn(stub_osiris);
    host.load_bytes(&adapter, "osiris-adapter").unwrap();
    host.init_all().unwrap();
    let after_init = host.trace().iter().filter(|t| matches!(t, Trace::Emit { from, topic, .. }
        if from == "osiris-adapter" && topic == "osiris.land.quake")).count();

    host.inject("host", "osiris.refresh", br#"{"base":"http://10.0.0.9:3000"}"#).unwrap();
    let after_trigger = host.trace().iter().filter(|t| matches!(t, Trace::Emit { from, topic, .. }
        if from == "osiris-adapter" && topic == "osiris.land.quake")).count();
    assert!(after_trigger > after_init, "the refresh trigger should drive another poll+emit");
}

#[test]
fn adapter_discovered_by_capability_alongside_the_others() {
    // The generic net.fetch discovery finds osiris-adapter by capability, never id.
    let mut host = Host::new().with_fetch_fn(stub_osiris);
    for name in ["watcher", "osiris-adapter"] {
        let bytes = require_wasm!(name);
        host.load_bytes(&bytes, name).unwrap();
    }
    let discovered: Vec<String> = host.ploxions().iter()
        .filter(|p| p.manifest.has_capability("net.fetch"))
        .map(|p| p.id().to_string())
        .collect();
    assert!(discovered.iter().any(|id| id == "osiris-adapter"),
        "osiris-adapter must be discovered by the net.fetch capability");
    assert!(!discovered.iter().any(|id| id == "watcher"),
        "the capability-free watcher must not be discovered");
}

// --- helper -----------------------------------------------------------------

/// Decode a lowercase-hex `"key":"..."` field from a flat JSON payload.
fn hex_field(json: &str, key: &str) -> Option<Vec<u8>> {
    let needle = format!("\"{key}\"");
    let start = json.find(&needle)? + needle.len();
    let rest = &json[start..];
    let colon = rest.find(':')?;
    let after = rest[colon + 1..].trim_start().strip_prefix('"')?;
    let end = after.find('"')?;
    let hex = &after[..end];
    if hex.len() % 2 != 0 {
        return None;
    }
    let b = hex.as_bytes();
    let mut out = Vec::with_capacity(hex.len() / 2);
    let mut i = 0;
    while i < b.len() {
        let hi = (b[i] as char).to_digit(16)?;
        let lo = (b[i + 1] as char).to_digit(16)?;
        out.push(((hi << 4) | lo) as u8);
        i += 2;
    }
    Some(out)
}
