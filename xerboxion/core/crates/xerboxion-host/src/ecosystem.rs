//! # `ecosystem` — the WHOLE ploxion computer, not just the running processes.
//!
//! The live [`Snapshot`] (`map`) is the host's *running set*: the ~10 WASM
//! ploxions actually loaded into THIS host, their live bus wiring, their health.
//! That is the equivalent of "the processes running right now". But a computer is
//! more than its running processes: there are **installed apps** that are not
//! running, and **services** deployed elsewhere on the machine.
//!
//! This module assembles that full picture — the [`Ecosystem`] — by AGGREGATING
//! three real, read-only sources (José: « montre TOUS les ploxions … comme si
//! c'était un ordinateur ») :
//!
//! 1. **LOADED** ploxions — the live host set, taken verbatim from the live
//!    [`Snapshot`]. State `"running"`. Carries the real `provides`/`requires`/
//!    `kind`/`health` from the running host, plus the live bus wiring.
//! 2. **REGISTERED** ploxions — every entry in the runtime registry
//!    `ploxi0ns.json` ([`crate::registry`]) that is NOT already loaded. State
//!    `"registered"` ("available but not running"). The registry is the SOURCE
//!    OF TRUTH for what exists in the ecosystem.
//! 3. **DEPLOYED** services — the `deployed:true` services from `runtime.json`
//!    ([`crate::connector`]) that are NOT already loaded/registered as a node.
//!    State `"deployed"`.
//!
//! ## The registry read IS the API connector
//! Reading `ploxi0ns.json` (+ `runtime.json`) is itself a native participant —
//! the host's adapter to the ecosystem's registry API (José: « connecte ça au
//! service de l'API … c'est un ploxion »). The [`Ecosystem::connector`] field
//! names that participant and records where the data came from, so the 3D view
//! can show the connection explicitly. The marketplace HTTP registry
//! (`labo.j0bot.ch/ploxions/registry.json`) needs auth and is deliberately NOT a
//! dependency: the LOCAL `ploxi0ns.json` is the source.
//!
//! ## Graceful degradation
//! If the registry file is ABSENT/unreadable, [`crate::registry::read_registry`]
//! returns an empty list and the ecosystem degrades to exactly the loaded set
//! (no crash). Same for `runtime.json`. So `/ecosystem` is always at least the
//! superset-of-one: the live host.
//!
//! ## Dedupe + state precedence
//! Dedup key is the `id`; each id appears EXACTLY ONCE. State precedence is
//! `running` > `deployed` > `registered`:
//!
//! - loaded in this host => `running` (running always wins).
//! - else deployed as a service (`runtime.json`) => `deployed`. A registered
//!   ploxion that is ALSO deployed is shown as deployed — it is really running
//!   as a service, not merely "available"; it keeps its registry name and gains
//!   the service health URL.
//! - else just known to the registry => `registered` ("available, not running").
//!
//! So the 3D view distinguishes the WHOLE computer: live processes (running),
//! deployed services (deployed), and installed-but-idle apps (registered).

use std::path::Path;

use serde::Serialize;

use crate::connector::{self, Service};
use crate::map::{BusEdge, PloxionView, Snapshot};
use crate::registry::{self, RegistryEntry};

/// The three lifecycle states a ploxion/service can be in, mirroring "process vs
/// installed app vs deployed service" on a computer.
pub mod state {
    /// Loaded + live in THIS host (a running WASM/native participant).
    pub const RUNNING: &str = "running";
    /// Known to the registry but NOT loaded here (available, not running).
    pub const REGISTERED: &str = "registered";
    /// A deployed service from `runtime.json` (running elsewhere on the box).
    pub const DEPLOYED: &str = "deployed";
}

/// One node in the ecosystem view — a ploxion (running or registered) or a
/// deployed service. Flat + JSON-friendly so the 3D view can render it directly.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct EcoNode {
    /// The stable id (dedup key across the three sources).
    pub id: String,
    /// A human name (display name, or the id when none is known).
    pub name: String,
    /// `"wasm"`/`"native"`/`"recipe"` for loaded ploxions; the registry
    /// `category` for registered ones; `"service"` for deployed services.
    pub kind: String,
    /// One of [`state::RUNNING`], [`state::REGISTERED`], [`state::DEPLOYED`].
    pub state: String,
    /// Topics this node provides (only known for the loaded set).
    pub provides: Vec<String>,
    /// Topics this node requires (only known for the loaded set).
    pub requires: Vec<String>,
    /// The registry category, when known (`""` otherwise).
    pub category: String,
    /// Live `plc_health` for running wasm ploxions (0 = ok). 0 for the rest.
    pub health: i32,
    /// For a deployed service: its health URL. Empty otherwise.
    pub url: String,
}

/// The native participant that connects the host to the ecosystem registry API
/// (José: « c'est un ploxion »). Surfaced so the 3D view can show the connection.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ConnectorInfo {
    /// The participant id of the registry connector.
    pub id: String,
    /// Where the registered set was read from (the registry file path).
    pub registry_source: String,
    /// Where the deployed set was read from (the runtime overlay path).
    pub runtime_source: String,
    /// How many entries the registry surfaced (0 if absent/unreadable).
    pub registered_from_registry: usize,
    /// Whether the registry file was present and read.
    pub registry_present: bool,
}

/// Per-state tallies for the HUD ("N running · M registered · K services").
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct EcoCounts {
    pub running: usize,
    pub registered: usize,
    pub deployed: usize,
    /// `running + registered + deployed`.
    pub total: usize,
}

/// The full ecosystem: the superset of the live [`Snapshot`]. `/snapshot` stays
/// the running host; `/ecosystem` is THIS — every ploxion + every service.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Ecosystem {
    /// Always `"xerboxion-rt ecosystem"` — identifies the producer.
    pub generated_by: String,
    /// The short git sha of the core (same as the snapshot's).
    pub core_commit: String,
    /// Every node: running, then registered, then deployed.
    pub nodes: Vec<EcoNode>,
    /// The LIVE bus wiring (carried over verbatim from the snapshot — only the
    /// running set has live edges).
    pub bus: Vec<BusEdge>,
    /// Per-state tallies.
    pub counts: EcoCounts,
    /// The registry-API connector participant (the read IS the connection).
    pub connector: ConnectorInfo,
}

/// The connector id naming the registry-API read as a participant.
pub const REGISTRY_CONNECTOR_ID: &str = "registry-connector";

impl Ecosystem {
    /// Serialize to pretty JSON.
    pub fn to_json_pretty(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".to_string())
    }
}

/// Build the ecosystem from the live snapshot + the read-only registry/runtime
/// files at their canonical paths. Convenience wrapper over [`build`].
///
/// Missing registry/runtime files degrade gracefully (the ecosystem becomes just
/// the loaded set). The daemon calls this per `GET /ecosystem` request so a file
/// appearing/changing is picked up live.
pub fn build_default(snapshot: &Snapshot) -> Ecosystem {
    build(
        snapshot,
        registry::DEFAULT_REGISTRY,
        connector::DEFAULT_RUNTIME,
    )
}

/// Build the ecosystem from an explicit snapshot + registry path + runtime path
/// (split out so tests can point at fixtures). Reads both files read-only;
/// neither is ever written. A read error on either is treated as "absent"
/// (empty), so this never fails — it always returns at least the loaded set.
pub fn build(
    snapshot: &Snapshot,
    registry_path: impl AsRef<Path>,
    runtime_path: impl AsRef<Path>,
) -> Ecosystem {
    let registry_path = registry_path.as_ref();
    let runtime_path = runtime_path.as_ref();

    // (a) LOADED — verbatim from the live snapshot. These win every dedup.
    let mut nodes: Vec<EcoNode> = snapshot.ploxions.iter().map(node_from_loaded).collect();
    let loaded_ids: std::collections::BTreeSet<String> =
        nodes.iter().map(|n| n.id.clone()).collect();

    // (b) REGISTERED — every registry entry NOT already loaded. The registry is
    //     the source of truth for "what exists". Absent file => empty list. Index
    //     where each registered node landed so (c) can promote it to `deployed`.
    let registry_present = registry_path.exists();
    let registered = registry::read_registry(registry_path).unwrap_or_default();
    let registered_from_registry = registered.len();
    let mut node_index: std::collections::BTreeMap<String, usize> =
        std::collections::BTreeMap::new();
    for e in &registered {
        if loaded_ids.contains(&e.id) {
            continue; // already loaded (running) — running wins
        }
        if node_index.contains_key(&e.id) {
            continue; // duplicate registry entry — keep first
        }
        node_index.insert(e.id.clone(), nodes.len());
        nodes.push(node_from_registered(e));
    }

    // (c) DEPLOYED — services from runtime.json. `read_services` keeps only the
    //     deployed-with-health entries. State precedence deployed > registered:
    //     a service id that is ALSO a registered node is PROMOTED to `deployed`
    //     in place (keeps its registry name, gains the health url) rather than
    //     hidden as merely "registered". A loaded (running) service stays running.
    //     A brand-new service id is appended as its own deployed node.
    let services = connector::read_services(runtime_path).unwrap_or_default();
    for s in &services {
        if loaded_ids.contains(&s.id) {
            continue; // running here — running wins
        }
        if let Some(&idx) = node_index.get(&s.id) {
            // promote the existing registered node to deployed
            nodes[idx].state = state::DEPLOYED.to_string();
            nodes[idx].url = s.url.clone();
        } else {
            node_index.insert(s.id.clone(), nodes.len());
            nodes.push(node_from_service(s));
        }
    }

    let counts = count(&nodes);
    let connector = ConnectorInfo {
        id: REGISTRY_CONNECTOR_ID.to_string(),
        registry_source: registry_path.display().to_string(),
        runtime_source: runtime_path.display().to_string(),
        registered_from_registry,
        registry_present,
    };

    Ecosystem {
        generated_by: "xerboxion-rt ecosystem".to_string(),
        core_commit: snapshot.core_commit.clone(),
        nodes,
        bus: snapshot.bus.clone(),
        counts,
        connector,
    }
}

/// A loaded ploxion (live host participant) -> a `running` node, verbatim.
fn node_from_loaded(p: &PloxionView) -> EcoNode {
    EcoNode {
        id: p.id.clone(),
        name: p.id.clone(),
        kind: p.kind.clone(),
        state: state::RUNNING.to_string(),
        provides: p.provides.clone(),
        requires: p.requires.clone(),
        category: String::new(),
        health: p.health,
        url: String::new(),
    }
}

/// A registry entry (not loaded) -> a `registered` node. `kind` carries the
/// registry category so the view can colour by domain; provides/requires are
/// unknown for a non-loaded ploxion (it has no live manifest here).
fn node_from_registered(e: &RegistryEntry) -> EcoNode {
    let name = if !e.display_name.is_empty() {
        e.display_name.clone()
    } else if !e.name.is_empty() {
        e.name.clone()
    } else {
        e.id.clone()
    };
    EcoNode {
        id: e.id.clone(),
        name,
        kind: if e.category.is_empty() {
            "registered".to_string()
        } else {
            e.category.clone()
        },
        state: state::REGISTERED.to_string(),
        provides: Vec::new(),
        requires: Vec::new(),
        category: e.category.clone(),
        health: 0,
        url: String::new(),
    }
}

/// A deployed service (from `runtime.json`) -> a `deployed` node.
fn node_from_service(s: &Service) -> EcoNode {
    EcoNode {
        id: s.id.clone(),
        name: s.id.clone(),
        kind: "service".to_string(),
        state: state::DEPLOYED.to_string(),
        provides: Vec::new(),
        requires: Vec::new(),
        category: String::new(),
        health: 0,
        url: s.url.clone(),
    }
}

/// Tally nodes by state.
fn count(nodes: &[EcoNode]) -> EcoCounts {
    let mut running = 0;
    let mut registered = 0;
    let mut deployed = 0;
    for n in nodes {
        match n.state.as_str() {
            state::RUNNING => running += 1,
            state::REGISTERED => registered += 1,
            state::DEPLOYED => deployed += 1,
            _ => {}
        }
    }
    EcoCounts {
        running,
        registered,
        deployed,
        total: nodes.len(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::{BusEdge, Snapshot};

    /// A minimal snapshot with two loaded ploxions (`ping` wasm, `pong` wasm),
    /// one bus edge, used as the "running set".
    fn fake_snapshot() -> Snapshot {
        Snapshot {
            generated_by: "test".into(),
            core_commit: "deadbee".into(),
            scenario: "test".into(),
            ploxions: vec![
                PloxionView {
                    id: "ping".into(),
                    version: "1".into(),
                    provides: vec!["ping".into()],
                    requires: vec![],
                    capabilities: vec![],
                    kind: "wasm".into(),
                    health: 0,
                },
                PloxionView {
                    id: "pong".into(),
                    version: "1".into(),
                    provides: vec!["pong".into()],
                    requires: vec!["ping".into()],
                    capabilities: vec![],
                    kind: "wasm".into(),
                    health: 0,
                },
            ],
            bus: vec![BusEdge {
                topic: "ping".into(),
                from: "ping".into(),
                to: vec!["pong".into()],
            }],
            trace: vec![],
            services: vec![],
            tsoin_root: String::new(),
        }
    }

    const REGISTRY_FIXTURE: &str = r#"{
        "version":"1.0.0",
        "ploxions":[
            {"id":"ping","category":"core","status":"active","displayName":"Ping"},
            {"id":"cubion","category":"storage","status":"active","displayName":"Cubion"},
            {"id":"xerax","category":"ai","status":"active","displayName":"Xerax"}
        ]
    }"#;

    const RUNTIME_FIXTURE: &str = r#"{
        "ploxions":{
            "repoverse":{"deployed":true,"health":"https://repoverse.example"},
            "cubion":{"deployed":false}
        }
    }"#;

    fn write_tmp(name: &str, body: &str) -> std::path::PathBuf {
        let p = std::env::temp_dir().join(name);
        std::fs::write(&p, body).unwrap();
        p
    }

    #[test]
    fn ecosystem_is_superset_of_loaded_when_registry_present() {
        let snap = fake_snapshot();
        let reg = write_tmp("xerb_eco_reg.json", REGISTRY_FIXTURE);
        let run = write_tmp("xerb_eco_run.json", RUNTIME_FIXTURE);

        let eco = build(&snap, &reg, &run);

        // 2 loaded (ping, pong) + 2 registered-not-loaded (cubion, xerax) +
        // 1 deployed-not-otherwise-present (repoverse) = 5 nodes.
        assert!(
            eco.nodes.len() > snap.ploxions.len(),
            "ecosystem ({}) must exceed the loaded set ({})",
            eco.nodes.len(),
            snap.ploxions.len()
        );
        assert_eq!(eco.counts.running, 2);
        assert_eq!(eco.counts.registered, 2); // cubion + xerax (ping deduped)
        assert_eq!(eco.counts.deployed, 1); // repoverse
        assert_eq!(eco.counts.total, 5);

        // Dedup: `ping` is in BOTH the loaded set and the registry — exactly once,
        // and as `running` (running wins).
        let pings: Vec<&EcoNode> = eco.nodes.iter().filter(|n| n.id == "ping").collect();
        assert_eq!(pings.len(), 1, "ping must appear exactly once");
        assert_eq!(pings[0].state, state::RUNNING);

        // The connector reports the real source + count.
        assert!(eco.connector.registry_present);
        assert_eq!(eco.connector.registered_from_registry, 3);
        assert_eq!(eco.connector.id, REGISTRY_CONNECTOR_ID);

        // The live bus wiring is carried over.
        assert_eq!(eco.bus.len(), 1);

        let _ = std::fs::remove_file(&reg);
        let _ = std::fs::remove_file(&run);
    }

    #[test]
    fn ecosystem_degrades_to_loaded_set_when_registry_absent() {
        let snap = fake_snapshot();
        let eco = build(
            &snap,
            "/no/such/registry.json",
            "/no/such/runtime.json",
        );
        // No registry, no runtime => exactly the loaded set, no crash.
        assert_eq!(eco.nodes.len(), snap.ploxions.len());
        assert_eq!(eco.counts.running, 2);
        assert_eq!(eco.counts.registered, 0);
        assert_eq!(eco.counts.deployed, 0);
        assert!(!eco.connector.registry_present);
        assert_eq!(eco.connector.registered_from_registry, 0);
        // Every node is running.
        assert!(eco.nodes.iter().all(|n| n.state == state::RUNNING));
    }

    #[test]
    fn deployed_service_promotes_registered_in_place() {
        // A service id that is also a registry entry must NOT double-count: it is
        // one node, PROMOTED from registered to deployed (deployed > registered),
        // keeping its registry identity and gaining the health url.
        let snap = fake_snapshot();
        let reg = write_tmp(
            "xerb_eco_reg2.json",
            r#"{"ploxions":[{"id":"repoverse","category":"infra","status":"active","displayName":"RepoVerse"}]}"#,
        );
        let run = write_tmp(
            "xerb_eco_run2.json",
            r#"{"ploxions":{"repoverse":{"deployed":true,"health":"https://r.example"}}}"#,
        );
        let eco = build(&snap, &reg, &run);
        let rv: Vec<&EcoNode> = eco.nodes.iter().filter(|n| n.id == "repoverse").collect();
        assert_eq!(rv.len(), 1, "repoverse must appear exactly once");
        // Promoted: state deployed, registry name + category preserved, url set.
        assert_eq!(rv[0].state, state::DEPLOYED);
        assert_eq!(rv[0].name, "RepoVerse");
        assert_eq!(rv[0].category, "infra");
        assert_eq!(rv[0].url, "https://r.example");
        assert_eq!(eco.counts.deployed, 1);
        assert_eq!(eco.counts.registered, 0);
        let _ = std::fs::remove_file(&reg);
        let _ = std::fs::remove_file(&run);
    }

    #[test]
    fn loaded_wins_over_deployed_and_registered() {
        // tsoin-style: a ploxion loaded here AND in registry AND deployed as a
        // service stays `running` — running always wins.
        let snap = fake_snapshot(); // loads ping + pong
        let reg = write_tmp(
            "xerb_eco_reg3.json",
            r#"{"ploxions":[{"id":"ping","category":"core","status":"active"}]}"#,
        );
        let run = write_tmp(
            "xerb_eco_run3.json",
            r#"{"ploxions":{"ping":{"deployed":true,"health":"https://ping.example"}}}"#,
        );
        let eco = build(&snap, &reg, &run);
        let pings: Vec<&EcoNode> = eco.nodes.iter().filter(|n| n.id == "ping").collect();
        assert_eq!(pings.len(), 1);
        assert_eq!(pings[0].state, state::RUNNING);
        assert_eq!(eco.counts.deployed, 0);
        assert_eq!(eco.counts.registered, 0);
        let _ = std::fs::remove_file(&reg);
        let _ = std::fs::remove_file(&run);
    }
}
