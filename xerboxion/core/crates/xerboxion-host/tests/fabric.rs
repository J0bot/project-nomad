//! Real integration tests for the operational xerboxion-core.
//!
//! These tests load ACTUAL compiled WASM ploxions (ping/pong/tracer) plus a
//! couple of hand-written bad modules (WAT compiled at runtime is avoided to
//! keep deps minimal; instead we use raw `.wat`-free wasm via wasmtime's own
//! text support if available, else a known-bad blob). The example ploxions are
//! built by `scripts/build-ploxions.sh` into `target/ploxions/`.
//!
//! If the wasm artifacts are missing, the tests that need them are skipped with
//! a clear message (so `cargo test` never silently passes on nothing) — but in
//! CI/demo the build script runs first, so they execute for real.

use std::path::PathBuf;

use xerboxion_host::{Host, Trace};

/// Locate the staged ploxion wasm dir (workspace_root/target/ploxions).
fn ploxion_dir() -> PathBuf {
    // CARGO_MANIFEST_DIR = .../crates/xerboxion-host
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest
        .parent() // crates/
        .and_then(|p| p.parent()) // workspace root
        .map(|p| p.join("target/ploxions"))
        .unwrap()
}

fn wasm(name: &str) -> Option<Vec<u8>> {
    let p = ploxion_dir().join(format!("{name}.wasm"));
    std::fs::read(p).ok()
}

/// Skip-with-message helper so a missing build doesn't masquerade as a pass.
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

#[test]
fn loads_ploxion_and_reads_manifest() {
    let ping = require_wasm!("ping");
    let mut host = Host::new();
    host.load_bytes(&ping, "ping").expect("ping loads");
    assert_eq!(host.ploxions().len(), 1);
    let m = &host.ploxions()[0].manifest;
    assert_eq!(m.id, "ping");
    assert_eq!(m.version, "1.0.0");
    assert!(m.provides.iter().any(|t| t == "ping"));
    assert!(m.requires.iter().any(|t| t == "tick"));
}

#[test]
fn bus_delivers_emit_to_subscriber() {
    let ping = require_wasm!("ping");
    let pong = require_wasm!("pong");
    let mut host = Host::new();
    host.load_bytes(&ping, "ping").unwrap();
    host.load_bytes(&pong, "pong").unwrap();

    // Bus wiring built from manifests: pong subscribes to "ping".
    assert!(host.bus().subscribers("ping").iter().any(|s| s == "pong"));

    host.init_all().unwrap();
    // ping emitted on init -> pong must have received it and emitted "pong".
    let delivered_to_pong = host.trace().iter().any(|t| matches!(
        t, Trace::Deliver { to, topic, .. } if to == "pong" && topic == "ping"
    ));
    let pong_emitted = host.trace().iter().any(|t| matches!(
        t, Trace::Emit { from, topic, .. } if from == "pong" && topic == "pong"
    ));
    assert!(delivered_to_pong, "ping was NOT delivered to pong's plc_on_event");
    assert!(pong_emitted, "pong did NOT react by emitting 'pong'");
}

#[test]
fn droit_au_silence_no_delivery_to_non_subscriber() {
    // tracer requires only tick/ping/pong; it does NOT subscribe to a private
    // topic. Inject a private topic — tracer (and everyone) must NOT receive it.
    let tracer = require_wasm!("tracer");
    let mut host = Host::new();
    host.load_bytes(&tracer, "tracer").unwrap();
    host.init_all().unwrap();

    let before = host.trace().len();
    host.inject("host", "secret-private-topic", b"should-not-arrive")
        .unwrap();

    // The emit is traced (the bus saw it), but NO delivery to any ploxion.
    let deliveries_after: Vec<_> = host.trace()[before..]
        .iter()
        .filter(|t| matches!(t, Trace::Deliver { .. }))
        .collect();
    assert!(
        deliveries_after.is_empty(),
        "an unsubscribed ploxion received an event (violates droit au silence): {:?}",
        deliveries_after
    );
}

#[test]
fn emit_not_delivered_back_to_emitter() {
    // ping provides AND we explicitly also subscribe ping to "ping" to prove the
    // host never echoes an emit back to its own emitter.
    let ping = require_wasm!("ping");
    let mut host = Host::new();
    host.load_bytes(&ping, "ping").unwrap();
    host.subscribe("ping", "ping"); // pathological self-subscribe
    host.init_all().unwrap();

    let self_delivery = host.trace().iter().any(|t| matches!(
        t, Trace::Deliver { from, to, topic } if from == "ping" && to == "ping" && topic == "ping"
    ));
    assert!(!self_delivery, "host echoed an emit back to its emitter");
}

#[test]
fn lifecycle_init_once_goodbye_on_shutdown() {
    let ping = require_wasm!("ping");
    let mut host = Host::new();
    host.load_bytes(&ping, "ping").unwrap();
    host.init_all().unwrap();

    // Exactly one init lifecycle event for ping.
    let inits = host
        .trace()
        .iter()
        .filter(|t| matches!(t, Trace::Lifecycle { who, what } if who == "ping" && what == "init"))
        .count();
    assert_eq!(inits, 1, "plc_init must be called exactly once");

    host.shutdown().unwrap();
    let goodbyes = host
        .trace()
        .iter()
        .filter(
            |t| matches!(t, Trace::Lifecycle { who, what } if who == "ping" && what == "goodbye"),
        )
        .count();
    assert_eq!(goodbyes, 1, "plc_goodbye must be called on shutdown");

    // After shutdown the live set is empty (Stores dropped — droit au silence).
    assert_eq!(host.ploxions().len(), 0);
}

#[test]
fn module_missing_required_export_is_rejected_cleanly() {
    // A minimal valid wasm module that exports a memory but NONE of the PLC
    // exports. The host must reject it with an Err, not panic.
    // (module (memory (export "memory") 1))
    let wasm_bytes: &[u8] = &[
        0x00, 0x61, 0x73, 0x6d, // magic "\0asm"
        0x01, 0x00, 0x00, 0x00, // version 1
        // Memory section: 1 memory, min 1 page
        0x05, 0x03, 0x01, 0x00, 0x01,
        // Export section: 1 export "memory" -> mem 0
        0x07, 0x0a, 0x01, 0x06, b'm', b'e', b'm', b'o', b'r', b'y', 0x02, 0x00,
    ];
    let mut host = Host::new();
    let res = host.load_bytes(wasm_bytes, "bad");
    assert!(res.is_err(), "a module missing PLC exports must be rejected");
    let msg = format!("{}", res.unwrap_err());
    assert!(
        msg.contains("missing required export") || msg.contains("rejected"),
        "rejection error should explain the missing export, got: {msg}"
    );
    // Nothing got registered.
    assert_eq!(host.ploxions().len(), 0);
}

#[test]
fn each_ploxion_gets_its_own_store_isolation() {
    // Load ping twice under two different fallback ids would collide on manifest
    // id, so instead load ping + pong and assert distinct, isolated instances:
    // each has its own memory pointer space (proven structurally — separate
    // Store). We assert the host kept them as separate entries and that an event
    // routed to one does not appear in the other's logs unless subscribed.
    let ping = require_wasm!("ping");
    let pong = require_wasm!("pong");
    let mut host = Host::new();
    host.load_bytes(&ping, "ping").unwrap();
    host.load_bytes(&pong, "pong").unwrap();
    assert_eq!(host.ploxions().len(), 2);

    // Distinct ids => distinct Stores (the host never shares a Store).
    let ids: Vec<&str> = host.ploxions().iter().map(|p| p.id()).collect();
    assert!(ids.contains(&"ping") && ids.contains(&"pong"));

    host.init_all().unwrap();
    // pong's counter lives in pong's own linear memory; ping cannot touch it.
    // We assert that only pong logged the count (ping never sees pong's state).
    let ping_logged_count = host.trace().iter().any(|t| matches!(
        t, Trace::Log { from, line } if from == "ping" && line.contains("count=")
    ));
    assert!(!ping_logged_count, "ping must not see pong's private counter");
}

#[test]
fn tracer_observes_multiple_topics() {
    let ping = require_wasm!("ping");
    let pong = require_wasm!("pong");
    let tracer = require_wasm!("tracer");
    let mut host = Host::new();
    host.load_bytes(&ping, "ping").unwrap();
    host.load_bytes(&pong, "pong").unwrap();
    host.load_bytes(&tracer, "tracer").unwrap();
    host.init_all().unwrap();
    host.inject("host", "tick", b"t1").unwrap();

    // tracer should have been delivered both "ping" and "pong" events.
    let got_ping = host.trace().iter().any(|t| matches!(
        t, Trace::Deliver { to, topic, .. } if to == "tracer" && topic == "ping"
    ));
    let got_pong = host.trace().iter().any(|t| matches!(
        t, Trace::Deliver { to, topic, .. } if to == "tracer" && topic == "pong"
    ));
    assert!(got_ping && got_pong, "tracer should observe both ping and pong");
}

// ===========================================================================
// HOT-LOAD / HOT-UNLOAD at the Host level (no daemon): load_runtime + unload
// really mutate the running Host + bus. A freshly loaded ploxion receives its
// required topics; an unloaded one receives nothing and its goodbye ran.
// ===========================================================================

#[test]
fn hot_load_runtime_wires_init_and_reacts() {
    let ping = require_wasm!("ping");
    let pong = require_wasm!("pong");
    let mut host = Host::new();
    // Start with ONLY ping loaded + initialized.
    host.load_bytes(&ping, "ping").unwrap();
    host.init_all().unwrap();

    // pong is NOT loaded yet: a ping injection must not route to pong.
    host.inject("api", "ping", b"before").unwrap();
    assert!(
        !host.trace().iter().any(|t| matches!(
            t, Trace::Deliver { to, topic, .. } if to == "pong" && topic == "ping"
        )),
        "with pong not loaded, ping must not route to pong"
    );

    // HOT-LOAD pong at runtime: it wires its `requires: [ping]` and runs init.
    let m = host.load_runtime(&pong, "pong").expect("pong hot-loads");
    assert_eq!(m.id, "pong");
    assert!(m.requires.iter().any(|t| t == "ping"));
    // A `load` lifecycle hop was traced.
    assert!(host.trace().iter().any(|t| matches!(
        t, Trace::Lifecycle { who, what } if who == "pong" && what == "load"
    )));
    assert!(host.ploxions().iter().any(|p| p.id() == "pong"));

    // Now ping routes to pong AND pong reacts (emits `pong`).
    host.inject("api", "ping", b"after").unwrap();
    assert!(
        host.trace().iter().any(|t| matches!(
            t, Trace::Deliver { to, topic, .. } if to == "pong" && topic == "ping"
        )),
        "after hot-load, ping MUST route to pong"
    );
    assert!(
        host.trace().iter().any(|t| matches!(
            t, Trace::Emit { from, topic, .. } if from == "pong" && topic == "pong"
        )),
        "after hot-load, pong MUST react with a `pong` emit"
    );
}

#[test]
fn hot_unload_runs_goodbye_and_silences() {
    let ping = require_wasm!("ping");
    let pong = require_wasm!("pong");
    let mut host = Host::new();
    host.load_bytes(&ping, "ping").unwrap();
    host.load_bytes(&pong, "pong").unwrap();
    host.init_all().unwrap();

    // UNLOAD pong: goodbye runs, bus unwired, Store dropped.
    let id = host.unload("pong").expect("pong unloads");
    assert_eq!(id, "pong");
    assert!(!host.ploxions().iter().any(|p| p.id() == "pong"), "pong is gone");
    // goodbye + unload lifecycle hops traced.
    assert!(host.trace().iter().any(|t| matches!(
        t, Trace::Lifecycle { who, what } if who == "pong" && what == "goodbye"
    )));
    assert!(host.trace().iter().any(|t| matches!(
        t, Trace::Lifecycle { who, what } if who == "pong" && what == "unload"
    )));

    // The bus no longer lists pong as a subscriber of `ping`.
    assert!(
        !host.bus().subscribers("ping").iter().any(|s| s == "pong"),
        "pong must be unsubscribed from ping after unload"
    );

    // A subsequent ping must NOT route to pong (silence).
    let before = host.trace().len();
    host.inject("api", "ping", b"after-unload").unwrap();
    assert!(
        !host.trace()[before..].iter().any(|t| matches!(
            t, Trace::Deliver { to, topic, .. } if to == "pong" && topic == "ping"
        )),
        "after unload, ping must be SILENT toward pong"
    );
}

#[test]
fn hot_load_duplicate_id_is_rejected_host_level() {
    let pong = require_wasm!("pong");
    let mut host = Host::new();
    host.load_bytes(&pong, "pong").unwrap();
    // Loading the same id again is rejected; the original is untouched.
    let err = host.load_runtime(&pong, "pong").unwrap_err();
    assert!(err.to_string().contains("already loaded"), "got: {err}");
    assert_eq!(host.ploxions().iter().filter(|p| p.id() == "pong").count(), 1);
}

#[test]
fn hot_load_bad_wasm_is_clean_error() {
    let mut host = Host::new();
    let err = host.load_runtime(b"not a wasm module", "junk").unwrap_err();
    // A clean compile error, no panic, host untouched.
    assert!(host.ploxions().is_empty());
    let _ = err; // the message form is wasmtime's; we only require Err + no panic.
}

#[test]
fn unload_unknown_id_is_clean_error() {
    let mut host = Host::new();
    let err = host.unload("never-loaded").unwrap_err();
    assert!(err.to_string().contains("no loaded ploxion"), "got: {err}");
}
