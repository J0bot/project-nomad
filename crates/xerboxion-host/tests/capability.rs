//! Integration tests for the **consented capability layer** (PLC v1.1).
//!
//! These are DETERMINISTIC and OFFLINE: the live network is never touched here
//! (the real-HTTP parts belong only to the `capdemo`/`services` subcommands).
//! The `plc_fetch` engine is replaced by a STUB via [`Host::with_fetch_fn`],
//! exactly how the connector tests inject a stub [`HealthCheck`].
//!
//! What is proven:
//!   a) GATE — a ploxion that DECLARES `net.fetch` gets `plc_fetch` linked and
//!      CAN fetch; a wasm that IMPORTS `plc_fetch` WITHOUT declaring it is
//!      REJECTED cleanly at load (the host does not link it). load-bearing.
//!   b) ROUND-TRIP — the `health-adapter`'s fetched `service.health` reaches the
//!      `watcher` unchanged (same topic/shape as the native connector).
//!   c) ERROR PATH — an unreachable url => status 0, no panic; the adapter still
//!      emits a `down` service.health and the watcher reacts.

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
                eprintln!(
                    "SKIP: {}.wasm not built — run scripts/build-ploxions.sh first",
                    $name
                );
                return;
            }
        }
    };
}

/// The hand-written negative fixture: imports `xerboxion.plc_fetch` but its
/// manifest declares NO capability. Compiled from
/// `tests/fixtures/sneaky_fetcher.wat` by `wasm-tools` and committed as a
/// `.wasm` so the test needs no extra toolchain.
const SNEAKY_FETCHER: &[u8] = include_bytes!("fixtures/sneaky_fetcher.wasm");

/// A deterministic stub fetch fn: returns 200 with a small body for any
/// localhost health url, and an unreachable (status 0) for a sentinel "down"
/// url. Never touches the network.
fn stub_fetch(_method: &str, url: &str, _body: &[u8], _limits: FetchLimits) -> FetchResult {
    if url.contains("unreachable") {
        FetchResult::unreachable("stub: simulated connection refused")
    } else {
        FetchResult {
            status: 200,
            body: r#"{"status":"ok"}"#.to_string(),
            truncated: false,
            error: None,
        }
    }
}

// --- a) the GATE -----------------------------------------------------------

#[test]
fn declaring_ploxion_gets_plc_fetch_linked_and_loads() {
    // The health-adapter declares capabilities:["net.fetch"] and imports
    // plc_fetch. With a stub fetch fn it must load AND instantiate cleanly.
    let adapter = require_wasm!("health-adapter");
    let mut host = Host::new().with_fetch_fn(stub_fetch);
    host.load_bytes(&adapter, "health-adapter")
        .expect("an adapter that DECLARES net.fetch must load with plc_fetch linked");

    let m = &host.ploxions()[0].manifest;
    assert_eq!(m.id, "health-adapter");
    assert!(m.has_capability("net.fetch"), "manifest should carry net.fetch");
    assert!(m.provides.iter().any(|t| t == "service.health"));
}

#[test]
fn undeclared_fetch_import_is_rejected_cleanly() {
    // The sneaky fixture imports plc_fetch but declares NO capability. The host
    // must reject it at load — NOT panic, NOT half-register it. This is the gate
    // being load-bearing: no consent in the manifest => no power linked => the
    // module cannot resolve its import.
    let mut host = Host::new().with_fetch_fn(stub_fetch);
    let res = host.load_bytes(SNEAKY_FETCHER, "sneaky-fetcher");

    assert!(
        res.is_err(),
        "a ploxion importing plc_fetch WITHOUT declaring net.fetch must be rejected"
    );
    let msg = format!("{:#}", res.unwrap_err());
    assert!(
        msg.contains("net.fetch") || msg.contains("plc_fetch") || msg.contains("capability"),
        "rejection should name the missing capability/import, got: {msg}"
    );
    // Nothing half-loaded — droit au silence even for a rejected module.
    assert_eq!(host.ploxions().len(), 0, "rejected module must not register");
}

#[test]
fn watcher_without_capability_has_no_fetch_import() {
    // The watcher declares no capability and does not import plc_fetch. It loads
    // fine (proving the gate doesn't break a normal ploxion) and the host links
    // it WITHOUT plc_fetch — exactly today's behaviour, additive.
    let watcher = require_wasm!("watcher");
    let mut host = Host::new();
    host.load_bytes(&watcher, "watcher")
        .expect("a capability-free ploxion must load exactly as before");
    let m = &host.ploxions()[0].manifest;
    assert!(m.capabilities.is_empty(), "watcher declares no capabilities");
}

// --- b) the ROUND-TRIP: adapter fetch -> service.health -> watcher ----------

#[test]
fn adapter_fetch_emits_service_health_and_watcher_reacts() {
    let adapter = require_wasm!("health-adapter");
    let watcher = require_wasm!("watcher");

    // All probes succeed (stub returns 200) => the adapter emits UP health.
    let mut host = Host::new().with_fetch_fn(stub_fetch);
    host.load_bytes(&adapter, "health-adapter").unwrap();
    host.load_bytes(&watcher, "watcher").unwrap();

    // The watcher auto-subscribed to service.health from its manifest.
    assert!(host.bus().subscribers("service.health").iter().any(|s| s == "watcher"));

    // init drives the adapter: it plc_fetches each default target and emits.
    host.init_all().unwrap();

    // The adapter actually called plc_fetch (traced as Fetch hops).
    let fetches = host
        .trace()
        .iter()
        .filter(|t| matches!(t, Trace::Fetch { from, status, .. }
            if from == "health-adapter" && *status == 200))
        .count();
    assert!(fetches >= 1, "adapter should have performed >=1 brokered fetch");

    // It emitted service.health on the bus...
    let emits = host
        .trace()
        .iter()
        .filter(|t| matches!(t, Trace::Emit { from, topic, .. }
            if from == "health-adapter" && topic == "service.health"))
        .count();
    assert!(emits >= 1, "adapter should emit service.health");
    assert_eq!(emits, fetches, "one service.health emitted per fetch");

    // ...and the watcher RECEIVED each into plc_on_event and logged UP.
    let delivered = host
        .trace()
        .iter()
        .filter(|t| matches!(t, Trace::Deliver { to, topic, .. }
            if to == "watcher" && topic == "service.health"))
        .count();
    assert_eq!(delivered, emits, "every adapter service.health reached the watcher");

    let watcher_logged_up = host.trace().iter().any(|t| matches!(t, Trace::Log { from, line }
        if from == "watcher" && line.contains("UP")));
    assert!(watcher_logged_up, "watcher should react to the adapter's UP health");
}

#[test]
fn adapter_health_is_indistinguishable_from_native_connector_shape() {
    // The adapter's emitted payload must be the SAME flat shape the native
    // connector uses, so the watcher cannot tell native from sandboxed.
    let adapter = require_wasm!("health-adapter");
    let watcher = require_wasm!("watcher");
    let mut host = Host::new().with_fetch_fn(stub_fetch);
    host.load_bytes(&adapter, "health-adapter").unwrap();
    host.load_bytes(&watcher, "watcher").unwrap();
    host.init_all().unwrap();

    let health_payload = host.trace().iter().find_map(|t| match t {
        Trace::Emit { from, topic, payload } if from == "health-adapter" && topic == "service.health" => {
            Some(payload.clone())
        }
        _ => None,
    });
    let p = health_payload.expect("adapter emitted a service.health payload");
    // {"id":..,"url":..,"code":..,"up":..} — the connector's exact fields.
    assert!(p.contains("\"id\":"));
    assert!(p.contains("\"url\":"));
    assert!(p.contains("\"code\":200"));
    assert!(p.contains("\"up\":true"));
}

// --- c) the ERROR PATH: unreachable => status 0, no panic -------------------

#[test]
fn unreachable_fetch_is_status_zero_and_adapter_emits_down() {
    // Stub: every probe is unreachable (status 0). The adapter must NOT panic;
    // it emits service.health with up:false, and the watcher alerts.
    let adapter = require_wasm!("health-adapter");
    let watcher = require_wasm!("watcher");

    let mut host = Host::new().with_fetch_fn(|_m, _u, _b, _l| {
        FetchResult::unreachable("stub: nothing listening")
    });
    host.load_bytes(&adapter, "health-adapter").unwrap();
    host.load_bytes(&watcher, "watcher").unwrap();
    host.init_all().unwrap();

    // The brokered fetches all came back status 0 (traced) — and we got here, so
    // no panic crossed the host boundary.
    let zero_fetches = host
        .trace()
        .iter()
        .filter(|t| matches!(t, Trace::Fetch { from, status, .. }
            if from == "health-adapter" && *status == 0))
        .count();
    assert!(zero_fetches >= 1, "unreachable fetch should report status 0");

    // The adapter emitted DOWN health, and the watcher reacted with an alert.
    let down_health = host.trace().iter().any(|t| matches!(t, Trace::Emit { from, topic, payload }
        if from == "health-adapter" && topic == "service.health" && payload.contains("\"up\":false")));
    assert!(down_health, "adapter should emit up:false for an unreachable service");

    let watcher_alerted = host.trace().iter().any(|t| matches!(t, Trace::Emit { from, topic, .. }
        if from == "watcher" && topic == "alert"));
    assert!(watcher_alerted, "watcher should alert on the adapter's DOWN health");
}

// --- d) the GENERIC DRIVER: discover by capability, drive, get emits --------

/// The discovery the `adapters` subcommand uses: collect every loaded ploxion
/// that DECLARES `net.fetch` — by capability, NEVER by a hardcoded id. Mirrors
/// `main.rs::discover_net_fetch_adapters` against the public `Host` API so the
/// generic mechanism is asserted offline.
fn discover_net_fetch_ids(host: &Host) -> Vec<String> {
    host.ploxions()
        .iter()
        .filter(|p| p.manifest.has_capability("net.fetch"))
        .map(|p| p.id().to_string())
        .collect()
}

#[test]
fn generic_discovery_finds_every_net_fetch_adapter_by_capability_not_id() {
    // Load the THREE oracle adapters in a deliberately scrambled order, plus a
    // capability-FREE ploxion (watcher) that must NOT be discovered. Discovery
    // is by manifest capability, so all three adapters are found and the watcher
    // is not — and a hypothetical 4th adapter would be found the same way.
    let mut host = Host::new().with_fetch_fn(stub_fetch);
    for name in ["watcher", "repoverse-adapter", "health-adapter", "ideas-map-adapter"] {
        let bytes = require_wasm!(name);
        host.load_bytes(&bytes, name).unwrap();
    }

    let mut discovered = discover_net_fetch_ids(&host);
    discovered.sort();
    assert_eq!(
        discovered,
        vec![
            "health-adapter".to_string(),
            "ideas-map-adapter".to_string(),
            "repoverse-adapter".to_string(),
        ],
        "discovery must find all three net.fetch adapters by capability — not the watcher"
    );

    // And it is genuinely by CAPABILITY: every discovered ploxion carries the
    // token, and the excluded watcher does not.
    for id in &discovered {
        let m = &host.ploxions().iter().find(|p| p.id() == id).unwrap().manifest;
        assert!(m.has_capability("net.fetch"), "{id} discovered => must declare net.fetch");
    }
    let watcher_m = &host.ploxions().iter().find(|p| p.id() == "watcher").unwrap().manifest;
    assert!(!watcher_m.has_capability("net.fetch"), "watcher must be excluded");
}

#[test]
fn driving_discovered_adapters_produces_an_emit_per_adapter() {
    // The driver's contract: drive every discovered adapter (init + inject its
    // declared `requires` trigger) and each emits its declared `provides` topic
    // onto the bus. OFFLINE: the stub returns 200 so each probe is "up". This
    // proves the GENERIC drive (not the hardcoded capdemo) yields emits for the
    // ideas-map and repoverse adapters too — by topic derived from the manifest.
    let mut host = Host::new().with_fetch_fn(stub_fetch);
    for name in ["health-adapter", "ideas-map-adapter", "repoverse-adapter"] {
        let bytes = require_wasm!(name);
        host.load_bytes(&bytes, name).unwrap();
    }

    // Snapshot each adapter's manifest-derived (id, provides[0], requires[0])
    // BEFORE driving — by capability, exactly as the driver does.
    struct A { id: String, provides: String, requires: Vec<String> }
    let adapters: Vec<A> = host
        .ploxions()
        .iter()
        .filter(|p| p.manifest.has_capability("net.fetch"))
        .map(|p| A {
            id: p.id().to_string(),
            provides: p.manifest.provides[0].clone(),
            requires: p.manifest.requires.clone(),
        })
        .collect();
    assert_eq!(adapters.len(), 3, "three net.fetch adapters expected");

    // DRIVE: init (defaults) then inject each adapter's trigger topic. A `{}`
    // payload makes the data adapters re-probe their baked URL; health-adapter's
    // trigger needs a url, so its `{}` trigger is a graceful no-op — still valid.
    host.init_all().unwrap();
    for a in &adapters {
        for trigger in &a.requires {
            host.inject("host", trigger, b"{}").unwrap();
        }
    }

    // Each adapter emitted its declared topic at least once (from init), and did
    // a real brokered fetch. Generic: keyed off the manifest topic, not an id.
    for a in &adapters {
        let emits = host.trace().iter().filter(|t| matches!(t, Trace::Emit { from, topic, .. }
            if from == &a.id && topic == &a.provides)).count();
        assert!(emits >= 1, "{} should emit [{}] at least once", a.id, a.provides);

        let fetches = host.trace().iter().filter(|t| matches!(t, Trace::Fetch { from, .. }
            if from == &a.id)).count();
        assert!(fetches >= 1, "{} should have done a brokered plc_fetch", a.id);
    }

    // The data adapters (whose trigger re-probes with the baked url) emit MORE
    // than once across init+trigger — proving the trigger path drives them too.
    for id in ["ideas-map-adapter", "repoverse-adapter"] {
        let a = adapters.iter().find(|a| a.id == id).unwrap();
        let emits = host.trace().iter().filter(|t| matches!(t, Trace::Emit { from, topic, .. }
            if from == &a.id && topic == &a.provides)).count();
        assert!(emits >= 2, "{id} should emit on both init and the trigger re-probe");
    }
}

#[test]
fn down_service_is_a_valid_outcome_for_a_data_adapter_not_a_crash() {
    // A DOWN/unreachable service must be handled gracefully: the adapter emits
    // its topic with code 0 / up:false — a valid outcome, never a panic. Drive
    // the repoverse adapter with an unreachable stub and assert it still emits.
    let bytes = require_wasm!("repoverse-adapter");
    let mut host = Host::new().with_fetch_fn(|_m, _u, _b, _l| {
        FetchResult::unreachable("stub: simulated down service")
    });
    host.load_bytes(&bytes, "repoverse-adapter").unwrap();
    host.init_all().unwrap();

    // Status 0 fetch traced, no panic crossed the boundary (we got here).
    let zero = host.trace().iter().filter(|t| matches!(t, Trace::Fetch { from, status, .. }
        if from == "repoverse-adapter" && *status == 0)).count();
    assert!(zero >= 1, "an unreachable service should report a status-0 fetch");

    // It still emitted repo.list, marked down — a valid outcome.
    let down = host.trace().iter().any(|t| matches!(t, Trace::Emit { from, topic, payload }
        if from == "repoverse-adapter" && topic == "repo.list" && payload.contains("\"up\":false")));
    assert!(down, "a data adapter should emit its topic with up:false when the service is down");
}

#[test]
fn health_check_trigger_drives_an_adhoc_fetch() {
    // The adapter `requires` health.check: inject one and it must probe that url
    // and emit service.health for it (the runtime-trigger path, not just init).
    let adapter = require_wasm!("health-adapter");
    let mut host = Host::new().with_fetch_fn(stub_fetch);
    host.load_bytes(&adapter, "health-adapter").unwrap();
    host.init_all().unwrap();

    let before = host.trace().len();
    host.inject(
        "host",
        "health.check",
        br#"{"id":"adhoc-svc","url":"http://example.test/health"}"#,
    )
    .unwrap();

    let emitted_for_adhoc = host.trace()[before..].iter().any(|t| matches!(
        t, Trace::Emit { from, topic, payload }
        if from == "health-adapter" && topic == "service.health" && payload.contains("adhoc-svc")
    ));
    assert!(emitted_for_adhoc, "a health.check trigger should drive an ad-hoc fetch+emit");
}
