//! Integration test for the **`map` snapshot** — the live-core render contract.
//!
//! Loads the real example ploxions (via the staged `target/ploxions/*.wasm`),
//! builds the representative scene through [`map::build_snapshot`], and asserts
//! the snapshot is well-formed: valid JSON, enough ploxions, a non-empty trace,
//! and that the fields are DERIVED from real state (the kinds, the bus edges, the
//! recipe firing, the services) — nothing hardcoded.
//!
//! Like the other wasm-backed tests it SKIPs (never silently passes) if the
//! ploxions have not been built yet.

use std::path::PathBuf;

use xerboxion_host::map;

fn ploxion_dir() -> PathBuf {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest
        .parent()
        .and_then(|p| p.parent())
        .map(|p| p.join("target/ploxions"))
        .unwrap()
}

/// How many ploxions the scene must surface for the snapshot to be meaningful.
/// The 7 wasm example ploxions + 1 recipe + 1 native connector = 9; we assert a
/// conservative floor so a missing optional ploxion still passes but an empty
/// scene fails.
const MIN_PLOXIONS: usize = 6;

fn skip_if_unbuilt() -> bool {
    if !ploxion_dir().join("ping.wasm").exists() {
        eprintln!(
            "SKIP: target/ploxions/*.wasm not built — run scripts/build-ploxions.sh first"
        );
        return true;
    }
    false
}

#[test]
fn map_json_is_valid_with_enough_ploxions_and_a_trace() {
    if skip_if_unbuilt() {
        return;
    }
    let snap = map::build_snapshot(&ploxion_dir(), "testsha").expect("scene must build");

    // The snapshot serializes to JSON that round-trips through a parser.
    let json = snap.to_json_pretty();
    let parsed: serde_json::Value =
        serde_json::from_str(&json).expect("map --json must produce VALID JSON");
    assert!(parsed.is_object());

    // >= N ploxions and a non-empty trace (the headline requirement).
    assert!(
        snap.ploxions.len() >= MIN_PLOXIONS,
        "expected >= {MIN_PLOXIONS} participants, got {}",
        snap.ploxions.len()
    );
    assert!(!snap.trace.is_empty(), "the trace must not be empty");

    // The top-level keys the viewer relies on are all present.
    for key in [
        "generated_by",
        "core_commit",
        "scenario",
        "ploxions",
        "bus",
        "trace",
        "services",
    ] {
        assert!(parsed.get(key).is_some(), "snapshot missing key {key:?}");
    }
    assert_eq!(snap.generated_by, "xerboxion-rt map");
    assert_eq!(snap.core_commit, "testsha");
}

#[test]
fn snapshot_fields_are_derived_from_real_state() {
    if skip_if_unbuilt() {
        return;
    }
    let snap = map::build_snapshot(&ploxion_dir(), "testsha").unwrap();

    // Every node carries a known kind — and all three kinds appear (wasm
    // ploxions, the native connector, the compiled recipe).
    for p in &snap.ploxions {
        assert!(
            matches!(p.kind.as_str(), "wasm" | "native" | "recipe"),
            "unknown node kind {:?}",
            p.kind
        );
    }
    assert!(snap.ploxions.iter().any(|p| p.kind == "wasm"));
    assert!(
        snap.ploxions.iter().any(|p| p.kind == "recipe"),
        "the sample alerter recipe must appear as a recipe node"
    );
    assert!(
        snap.ploxions.iter().any(|p| p.kind == "native"),
        "the native service-connector must appear as a native node"
    );

    // The health-adapter's declared capability is surfaced from its manifest.
    let adapter = snap
        .ploxions
        .iter()
        .find(|p| p.id == "health-adapter")
        .expect("the health-adapter ploxion must be in the scene");
    assert!(
        adapter.capabilities.iter().any(|c| c == "net.fetch"),
        "the adapter's net.fetch capability must be derived from its manifest"
    );

    // A real bus edge: service.health is provided and delivered to subscribers.
    let health_edge = snap
        .bus
        .iter()
        .find(|e| e.topic == "service.health")
        .expect("service.health must be a wired bus edge");
    assert!(
        health_edge.to.iter().any(|t| t == "watcher"),
        "service.health must be delivered to the watcher"
    );

    // The scenario actually exercised the bus: a fetch hop, an emit hop, a route
    // hop, and the recipe firing on the DOWN service are all in the trace.
    assert!(snap.trace.iter().any(|r| r.kind == "fetch"));
    assert!(snap.trace.iter().any(|r| r.kind == "emit"));
    assert!(snap.trace.iter().any(|r| r.kind == "route"));
    assert!(
        snap.trace.iter().any(|r| r.kind == "recipe"
            && r.payload.contains("FIRE")
            && r.payload.contains("ideas-map")),
        "the alerter recipe must FIRE on the down ideas-map service"
    );

    // Services: exactly the three bridged, with the down one reported down.
    assert_eq!(snap.services.len(), 3);
    let down = snap
        .services
        .iter()
        .find(|s| s.id == "ideas-map")
        .expect("ideas-map must be a bridged service");
    assert!(!down.up && down.code == 503, "ideas-map must be DOWN (503)");
}
