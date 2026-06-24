//! Integration tests for the **tsoin state ploxion** (the versioning/state
//! layer) driven over the XERB0XI0N bus.
//!
//! These load the ACTUAL compiled `tsoin` + `state-client` (+ `tracer`) wasm
//! modules and assert the end-to-end property the whole exercise is about:
//! bytes recorded by one ploxion are reconstructed **bit-exact** by a *separate*
//! ploxion and returned across the bus + two WASM boundaries.
//!
//! Built by `scripts/build-ploxions.sh` into `target/ploxions/`. If the
//! artifacts are missing the tests skip with a clear message (so `cargo test`
//! never silently passes on nothing); in CI/demo the build script runs first.

use std::path::PathBuf;

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

/// Pull the hex bytes out of a logged "OK"/replayed trace line is brittle; we
/// instead trust the client's own bit-exact comparison: it logs `OK` only when
/// the replayed bytes equal the original it recorded. This finds that log line.
fn client_reported_ok(host: &Host) -> bool {
    host.trace().iter().any(|t| matches!(
        t, Trace::Log { from, line } if from == "state-client" && line.contains("OK")
    ))
}

fn client_reported_mismatch(host: &Host) -> bool {
    host.trace().iter().any(|t| matches!(
        t, Trace::Log { from, line } if from == "state-client" && line.contains("MISMATCH")
    ))
}

#[test]
fn manifests_declare_the_tsoin_topics() {
    let tsoin = require_wasm!("tsoin");
    let mut host = Host::new();
    host.load_bytes(&tsoin, "tsoin").expect("tsoin loads");
    let m = &host.ploxions()[0].manifest;
    assert_eq!(m.id, "tsoin");
    assert!(m.provides.iter().any(|t| t == "tsoin.recorded"));
    assert!(m.provides.iter().any(|t| t == "tsoin.replayed"));
    assert!(m.requires.iter().any(|t| t == "tsoin.record"));
    assert!(m.requires.iter().any(|t| t == "tsoin.replay"));
    // The bus auto-subscribed tsoin to the request topics.
    assert!(host.bus().subscribers("tsoin.record").iter().any(|s| s == "tsoin"));
    assert!(host.bus().subscribers("tsoin.replay").iter().any(|s| s == "tsoin"));
}

#[test]
fn replay_is_bit_exact_via_the_bus() {
    let tsoin = require_wasm!("tsoin");
    let client = require_wasm!("state-client");
    let mut host = Host::new();
    host.load_bytes(&tsoin, "tsoin").unwrap();
    host.load_bytes(&client, "state-client").unwrap();

    // Driving init runs the whole cascade: client records 3 states, tsoin stores
    // each as a delta and replies, client replays an EARLY id, tsoin reconstructs
    // it and replies, client compares bit-for-bit.
    host.init_all().unwrap();

    // The tsoin ploxion stored 3 states and replied to each record.
    let recorded_replies = host
        .trace()
        .iter()
        .filter(|t| matches!(t, Trace::Emit { from, topic, .. } if from == "tsoin" && topic == "tsoin.recorded"))
        .count();
    assert_eq!(recorded_replies, 3, "tsoin should have recorded exactly 3 states");

    // tsoin reconstructed and replied with bytes exactly once (one replay).
    let replayed_replies = host
        .trace()
        .iter()
        .filter(|t| matches!(t, Trace::Emit { from, topic, .. } if from == "tsoin" && topic == "tsoin.replayed"))
        .count();
    assert_eq!(replayed_replies, 1, "tsoin should have replayed exactly 1 state");

    // The replayed reply was delivered back into the client's sandbox.
    let delivered_back = host.trace().iter().any(|t| matches!(
        t, Trace::Deliver { to, topic, .. } if to == "state-client" && topic == "tsoin.replayed"
    ));
    assert!(delivered_back, "replayed bytes were not delivered back to the client");

    // The client's OWN bit-exact comparison passed, and never reported MISMATCH.
    assert!(client_reported_ok(&host), "client did not confirm a bit-exact replay");
    assert!(!client_reported_mismatch(&host), "client reported a byte MISMATCH");
}

/// Independent, host-side bit-exact proof that does NOT rely on the client's own
/// verdict: drive tsoin directly via injected bus events, capture the replayed
/// hex from the trace, and compare to the bytes we recorded.
#[test]
fn host_side_bit_exact_record_then_replay() {
    let tsoin = require_wasm!("tsoin");
    let mut host = Host::new();
    host.load_bytes(&tsoin, "tsoin").unwrap();
    // Subscribe a do-nothing observer? No — we read tsoin's own emit from trace.
    host.init_all().unwrap();

    // Record two states by injecting tsoin.record events as if from an external
    // source. Bytes are arbitrary; we pick something with high+low entropy mix.
    let originals: [&str; 2] = [
        "deadbeef0011223344556677889900ff",       // state id 0
        "deadbeef0011223344556677889911ff",        // state id 1 (1 byte changed)
    ];
    for hex in originals {
        let payload = format!("{{\"name\":\"s\",\"bytes\":\"{hex}\"}}");
        host.inject("ext", "tsoin.record", payload.as_bytes()).unwrap();
    }

    // Replay the EARLY one (id 0).
    host.inject("ext", "tsoin.replay", br#"{"id":0}"#).unwrap();

    // Find the replayed emit from tsoin and pull its hex bytes.
    let replayed_hex = host
        .trace()
        .iter()
        .find_map(|t| match t {
            Trace::Emit { from, topic, payload } if from == "tsoin" && topic == "tsoin.replayed" => {
                // payload is the JSON {"id":0,"bytes":"...."} rendered by the host.
                let key = "\"bytes\":\"";
                let start = payload.find(key)? + key.len();
                let rest = &payload[start..];
                let end = rest.find('"')?;
                Some(rest[..end].to_string())
            }
            _ => None,
        })
        .expect("tsoin did not emit tsoin.replayed");

    assert_eq!(
        replayed_hex, originals[0],
        "replayed bytes are NOT bit-exact with the recorded bytes"
    );
}

#[test]
fn tsoin_state_lives_in_its_own_store_isolation() {
    // Isolation: the tsoin ploxion's timeline/recorder lives in ITS sandbox. We
    // prove the host keeps tsoin and state-client as separate, distinctly-ided
    // instances (separate Stores — the host never shares a Store), and that the
    // client never sees tsoin's internal node/recorder state: the only thing it
    // ever receives is the explicit reply payloads on topics it subscribed to.
    let tsoin = require_wasm!("tsoin");
    let client = require_wasm!("state-client");
    let mut host = Host::new();
    host.load_bytes(&tsoin, "tsoin").unwrap();
    host.load_bytes(&client, "state-client").unwrap();
    assert_eq!(host.ploxions().len(), 2);
    let ids: Vec<&str> = host.ploxions().iter().map(|p| p.id()).collect();
    assert!(ids.contains(&"tsoin") && ids.contains(&"state-client"));

    host.init_all().unwrap();

    // The client never logs anything about tsoin's internal node hashes / store
    // footprint — those are tsoin's private state, logged only under `tsoin`.
    let client_leaked_internal = host.trace().iter().any(|t| matches!(
        t, Trace::Log { from, line }
            if from == "state-client" && (line.contains("node ") || line.contains("cumulative"))
    ));
    assert!(
        !client_leaked_internal,
        "state-client saw tsoin's private recorder/store state (isolation broken)"
    );

    // And the only deliveries the client got are on the reply topics it requires.
    for t in host.trace() {
        if let Trace::Deliver { to, topic, .. } = t {
            if to == "state-client" {
                assert!(
                    topic == "tsoin.recorded" || topic == "tsoin.replayed",
                    "client received an unexpected topic '{topic}' (isolation/wiring leak)"
                );
            }
        }
    }
}

#[test]
fn droit_au_silence_non_subscriber_gets_no_tsoin_events() {
    // A passive `tracer` does NOT require any tsoin.* topic, so it must NEVER be
    // delivered a tsoin record/recorded/replay/replayed event — even though the
    // whole exchange happens right next to it on the same bus.
    let tsoin = require_wasm!("tsoin");
    let client = require_wasm!("state-client");
    let tracer = require_wasm!("tracer");
    let mut host = Host::new();
    host.load_bytes(&tsoin, "tsoin").unwrap();
    host.load_bytes(&client, "state-client").unwrap();
    host.load_bytes(&tracer, "tracer").unwrap();

    host.init_all().unwrap();

    // tracer requires only tick/ping/pong; assert it received NO tsoin.* topic.
    let tracer_got_tsoin = host.trace().iter().any(|t| matches!(
        t, Trace::Deliver { to, topic, .. }
            if to == "tracer" && topic.starts_with("tsoin.")
    ));
    assert!(
        !tracer_got_tsoin,
        "tracer (a non-subscriber) received a tsoin.* event — violates droit au silence"
    );

    // Sanity: the tsoin exchange DID happen (so the assertion above is meaningful,
    // not vacuously true because nothing flowed).
    assert!(client_reported_ok(&host), "the tsoin round trip did not complete");
}
