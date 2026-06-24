//! Integration tests for the **native service connector** + the **watcher**
//! WASM ploxion, driven over the XERB0XI0N bus.
//!
//! These are DETERMINISTIC and OFFLINE: they feed a fixture `runtime.json` and a
//! STUB health function (never the network). They load the ACTUAL compiled
//! `watcher` wasm module and assert the end-to-end property: the connector emits
//! exactly one `service.health` per deployed service, the watcher receives each
//! and counts UP/DOWN correctly, a DOWN service (code 000/5xx) yields `up:false`
//! and a watcher DOWN log + an `alert`, and a non-subscriber (`tracer`) gets
//! NONE of them (isolation / droit au silence).
//!
//! The LIVE network is exercised only by the `services` demo subcommand, never
//! here. If the watcher wasm is missing the wasm-dependent tests skip with a
//! clear message (so `cargo test` never silently passes on nothing); the build
//! script stages it first in CI/demo.

use std::path::PathBuf;

use xerboxion_host::connector::{
    parse_services, HealthResult, ServiceConnector, CONNECTOR_ID, HEALTH_TOPIC,
};
use xerboxion_host::{Host, Trace};

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

/// A fixture runtime.json: 2 deployed-with-health, plus deployed-without-health,
/// plus not-deployed — so we also prove the connector filters correctly.
const FIXTURE: &[u8] = br#"{
    "ploxions": {
        "alpha":   {"deployed": true,  "health": "https://alpha.test"},
        "bravo":   {"deployed": true,  "health": "https://bravo.test"},
        "charlie": {"deployed": true},
        "delta":   {"deployed": false, "health": "https://delta.test"}
    }
}"#;

/// A deterministic stub: alpha is UP (200), bravo is DOWN (unreachable, 000).
fn stub_check(url: &str) -> HealthResult {
    if url.contains("alpha") {
        HealthResult::from_code(200)
    } else {
        HealthResult::unreachable()
    }
}

#[test]
fn connector_filters_to_deployed_with_health() {
    let svcs = parse_services(FIXTURE).unwrap();
    let ids: Vec<&str> = svcs.iter().map(|s| s.id.as_str()).collect();
    // Only alpha + bravo qualify (deployed AND a health URL), sorted.
    assert_eq!(ids, vec!["alpha", "bravo"]);
}

#[test]
fn connector_emits_one_service_health_per_deployed_entry() {
    let watcher = require_wasm!("watcher");
    let mut host = Host::new();
    host.load_bytes(&watcher, "watcher").unwrap();

    // The watcher's manifest auto-subscribed it to service.health.
    assert!(host.bus().subscribers(HEALTH_TOPIC).iter().any(|s| s == "watcher"));

    host.init_all().unwrap();

    let conn = ServiceConnector::new(parse_services(FIXTURE).unwrap());
    let rows = host.connector_sweep(&conn, &stub_check).unwrap();

    // One sweep row + one bus emit per deployed-with-health service.
    assert_eq!(rows.len(), 2, "exactly one row per deployed-with-health service");
    let emits = host
        .trace()
        .iter()
        .filter(|t| matches!(t, Trace::Emit { from, topic, .. }
            if from == CONNECTOR_ID && topic == HEALTH_TOPIC))
        .count();
    assert_eq!(emits, 2, "connector must emit exactly one service.health per service");

    // The connector registered as the native provider of service.health.
    let provides_health = host
        .native_participants()
        .any(|(id, topics)| id == CONNECTOR_ID && topics.iter().any(|t| t == HEALTH_TOPIC));
    assert!(provides_health, "connector should register as native provider of service.health");
}

#[test]
fn watcher_counts_up_and_down_correctly() {
    let watcher = require_wasm!("watcher");
    let mut host = Host::new();
    host.load_bytes(&watcher, "watcher").unwrap();
    host.init_all().unwrap();

    let conn = ServiceConnector::new(parse_services(FIXTURE).unwrap());
    let rows = host.connector_sweep(&conn, &stub_check).unwrap();

    // The sweep itself: alpha up, bravo down.
    let alpha = rows.iter().find(|r| r.id == "alpha").unwrap();
    let bravo = rows.iter().find(|r| r.id == "bravo").unwrap();
    assert!(alpha.result.up && alpha.result.code == 200);
    assert!(!bravo.result.up && bravo.result.code == 0);

    // Every service.health was delivered into the watcher's plc_on_event.
    let delivered = host
        .trace()
        .iter()
        .filter(|t| matches!(t, Trace::Deliver { to, topic, .. }
            if to == "watcher" && topic == HEALTH_TOPIC))
        .count();
    assert_eq!(delivered, 2, "watcher must receive each service.health");

    // The watcher logged alpha UP and bravo DOWN (its own reaction).
    let logged_up = host.trace().iter().any(|t| matches!(t, Trace::Log { from, line }
        if from == "watcher" && line.contains("alpha") && line.contains("UP")));
    let logged_down = host.trace().iter().any(|t| matches!(t, Trace::Log { from, line }
        if from == "watcher" && line.contains("bravo") && line.contains("DOWN")));
    assert!(logged_up, "watcher should log alpha UP");
    assert!(logged_down, "watcher should log bravo DOWN");

    // The final count line shows up=1 down=1 (the running counters in its sandbox).
    let final_count = host.trace().iter().any(|t| matches!(t, Trace::Log { from, line }
        if from == "watcher" && line.contains("[up=1 down=1]")));
    assert!(final_count, "watcher's running UP/DOWN counters are wrong");
}

#[test]
fn down_service_yields_up_false_and_watcher_alert() {
    let watcher = require_wasm!("watcher");
    let mut host = Host::new();
    host.load_bytes(&watcher, "watcher").unwrap();
    host.init_all().unwrap();

    let conn = ServiceConnector::new(parse_services(FIXTURE).unwrap());
    let rows = host.connector_sweep(&conn, &stub_check).unwrap();

    // bravo is down -> up:false in both the row and the emitted payload.
    let bravo = rows.iter().find(|r| r.id == "bravo").unwrap();
    assert!(!bravo.result.up);
    assert!(bravo.payload.contains("\"up\":false"));

    // The watcher REACTED to the down service by emitting an "alert".
    let alert = host.trace().iter().any(|t| matches!(t, Trace::Emit { from, topic, payload }
        if from == "watcher" && topic == "alert" && payload.contains("bravo")));
    assert!(alert, "watcher must emit an 'alert' for a DOWN service");

    // No alert for the UP service.
    let alert_for_alpha = host.trace().iter().any(|t| matches!(t, Trace::Emit { from, topic, payload }
        if from == "watcher" && topic == "alert" && payload.contains("alpha")));
    assert!(!alert_for_alpha, "watcher must NOT alert on an UP service");
}

#[test]
fn five_hundred_is_reachable_but_down() {
    // A 5xx is reachable (code preserved) but NOT up — distinct from code 000.
    let watcher = require_wasm!("watcher");
    let mut host = Host::new();
    host.load_bytes(&watcher, "watcher").unwrap();
    host.init_all().unwrap();

    let conn = ServiceConnector::new(parse_services(FIXTURE).unwrap());
    // Stub: alpha 500 (reachable, unhealthy), bravo 200.
    let rows = host
        .connector_sweep(&conn, &|url| {
            if url.contains("alpha") {
                HealthResult::from_code(500)
            } else {
                HealthResult::from_code(200)
            }
        })
        .unwrap();

    let alpha = rows.iter().find(|r| r.id == "alpha").unwrap();
    assert_eq!(alpha.result.code, 500);
    assert!(!alpha.result.up, "a 5xx must be up:false");

    // The watcher logged alpha DOWN with code 500 (003-digit padded -> 500).
    let logged = host.trace().iter().any(|t| matches!(t, Trace::Log { from, line }
        if from == "watcher" && line.contains("alpha") && line.contains("DOWN") && line.contains("500")));
    assert!(logged, "watcher should log a 5xx service as DOWN with its code");
}

#[test]
fn droit_au_silence_non_subscriber_gets_no_service_health() {
    // A passive `tracer` requires only tick/ping/pong — NOT service.health. Even
    // though the whole sweep happens on the same bus next to it, it must receive
    // NONE of the service.health events.
    let watcher = require_wasm!("watcher");
    let tracer = require_wasm!("tracer");
    let mut host = Host::new();
    host.load_bytes(&watcher, "watcher").unwrap();
    host.load_bytes(&tracer, "tracer").unwrap();
    host.init_all().unwrap();

    let conn = ServiceConnector::new(parse_services(FIXTURE).unwrap());
    host.connector_sweep(&conn, &stub_check).unwrap();

    let tracer_got_health = host.trace().iter().any(|t| matches!(t, Trace::Deliver { to, topic, .. }
        if to == "tracer" && topic == HEALTH_TOPIC));
    assert!(
        !tracer_got_health,
        "tracer (a non-subscriber) received a service.health — violates droit au silence"
    );

    // Sanity: the sweep DID flow to the watcher (assertion above isn't vacuous).
    let watcher_got_health = host.trace().iter().any(|t| matches!(t, Trace::Deliver { to, topic, .. }
        if to == "watcher" && topic == HEALTH_TOPIC));
    assert!(watcher_got_health, "the sweep should have reached the watcher");
}

#[test]
fn watcher_health_mirrors_the_fleet() {
    // The watcher's own plc_health goes DEGRADED iff it saw a DOWN service.
    let watcher = require_wasm!("watcher");

    // All-up fleet -> watcher health ok.
    {
        let mut host = Host::new();
        host.load_bytes(&watcher, "watcher").unwrap();
        host.init_all().unwrap();
        let conn = ServiceConnector::new(parse_services(FIXTURE).unwrap());
        host.connector_sweep(&conn, &|_| HealthResult::from_code(200)).unwrap();
        let h = host.health_sweep().unwrap();
        let (_, status) = h.iter().find(|(id, _)| id == "watcher").unwrap();
        assert_eq!(*status, 0, "watcher should be healthy when all services are UP");
    }

    // One-down fleet -> watcher health degraded.
    {
        let mut host = Host::new();
        host.load_bytes(&watcher, "watcher").unwrap();
        host.init_all().unwrap();
        let conn = ServiceConnector::new(parse_services(FIXTURE).unwrap());
        host.connector_sweep(&conn, &stub_check).unwrap(); // bravo down
        let h = host.health_sweep().unwrap();
        let (_, status) = h.iter().find(|(id, _)| id == "watcher").unwrap();
        assert_ne!(*status, 0, "watcher should be DEGRADED when a service is DOWN");
    }
}
