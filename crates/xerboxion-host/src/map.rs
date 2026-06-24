//! # `map` — a live snapshot of the xerboxion-core, made VISIBLE.
//!
//! WASM ploxions, the bus wiring, the event trace, and the connected services
//! are all real, in-memory host state — but invisible unless you read a terminal.
//! This module **loads a representative scene** (every example ploxion + a sample
//! designer recipe), **runs a short scenario** that exercises the bus end to end,
//! then **derives a flat JSON [`Snapshot`]** from the *actual* [`Host`] state and
//! [`Trace`] journal. Nothing here is invented: every field comes from a real
//! ploxion manifest, a real bus subscription, or a real traced hop.
//!
//! The snapshot is the contract between the running core and any viewer (the
//! standalone `web-map/xerboxion-map.html` bakes one in so the core renders ALIVE
//! from `file://` with zero setup — the same trick `tsoin-web` uses for its wasm).
//!
//! ## Determinism
//! `map` must produce the **same** snapshot every run (so the baked HTML is
//! stable and the test can assert on it). So, unlike the live `services` /
//! `capdemo` subcommands, the scene runs entirely OFFLINE: the brokered
//! `plc_fetch` and the native connector's health check are both replaced by
//! deterministic stubs ([`stub_fetch`] / [`stub_health`]). The shapes the
//! ploxions see are byte-identical to the live ones — they cannot tell the
//! difference — but no socket is ever opened.
//!
//! ## Snapshot schema (v1)
//! ```json
//! {
//!   "generated_by": "xerboxion-rt map",
//!   "core_commit":  "<git short sha>",
//!   "scenario":     "<one-line description of what was run>",
//!   "ploxions": [
//!     {"id","version","provides":[],"requires":[],"capabilities":[],
//!      "kind":"wasm"|"native"|"recipe","health":0}
//!   ],
//!   "bus": [ {"topic","from","to":[]} ],
//!   "trace": [ {"seq","kind","from","topic","payload","note"} ],
//!   "services": [ {"id","url","code","up"} ]
//! }
//! ```
//! - `ploxions[].kind` — `wasm` (sandboxed module), `native` (host-side
//!   participant, e.g. the service connector), or `recipe` (a compiled designer
//!   rule running as a native participant). `health` is the live `plc_health`
//!   result for wasm ploxions (`0` = ok), `0` for native/recipe (not swept).
//! - `bus[]` — one row per `provider -> topic -> [subscribers]` edge. `from` is
//!   the provider id; `to` the delivered-to subscriber ids. A topic with no
//!   declared provider (e.g. a host-injected `tick`) has `from:""`.
//! - `trace[]` — the journal in order. `kind` is one of `emit`, `route`,
//!   `deliver` is folded into `route`, `log`, `life`, `fetch`, `recipe`. `seq` is
//!   the 0-based index. `from`/`topic`/`payload`/`note` are filled per kind (a
//!   field that does not apply is `""`).
//! - `services[]` — the deployed services the native connector bridged onto the
//!   bus this run, with the (stubbed, deterministic) health result.

use anyhow::Result;
use serde::Serialize;

use crate::connector::{self, HealthResult, Service, ServiceConnector};
use crate::fetch::{FetchLimits, FetchResult};
use crate::{Host, Recipe, Trace};

/// The sample designer recipe baked into the scene — same shape as the
/// `recipedemo`'s inline recipe and `examples/alerter.recipe.json`: an `alerter`
/// that fires `alert.notify` ONLY for a DOWN service, interpolating id + code.
pub const SAMPLE_RECIPE: &str = r#"{
  "ploxion": "alerter",
  "blocks": [
    {"kind": "quand",  "ref": "service.health"},
    {"kind": "si",     "param": "up == false"},
    {"kind": "action", "ref": "alert.notify", "param": "{id} is DOWN (code {code})"}
  ]
}"#;

/// The representative deployed services the scene bridges onto the bus. Fixed
/// here (rather than read from `runtime.json`) so `map` is fully deterministic
/// and self-contained — the snapshot is identical on any machine. One is up, one
/// is down, so the alerter recipe + watcher both have something to react to.
const SCENE_SERVICES: &[(&str, &str)] = &[
    ("repoverse", "https://repoverse.j0bot.ch"),
    ("filesystem", "https://fs.j0bot.ch"),
    ("ideas-map", "https://ideas.j0bot.ch"),
];

/// The one-line scenario description recorded in the snapshot, so a viewer can
/// see exactly what produced the trace.
pub const SCENARIO: &str = "load ping/pong/tracer/watcher/health-adapter/state-client/tsoin + \
    register an alerter recipe; init all (ping->pong, tsoin record/replay, adapter fetch->emit), \
    sweep 3 deployed services through the native connector (1 down), recipe fires on the down one";

/// Cap on the number of trace hops included in a `/snapshot` RESPONSE: only the
/// most-recent `TRACE_RESPONSE_CAP` rows are serialized (each keeping its real
/// `seq`). This bounds the response payload for an unbounded, ever-growing journal.
/// It does NOT truncate the host's real trace journal nor the tsoin recorder input
/// — only the serialized view.
const TRACE_RESPONSE_CAP: usize = 256;

// ---------------------------------------------------------------------------
// Deterministic offline stubs — the scene NEVER touches the network.
// ---------------------------------------------------------------------------

/// Deterministic stub for the brokered `plc_fetch` (used by the health-adapter).
/// Returns a 200 with a tiny health body for any url — so the adapter always
/// emits an UP service.health for its two default targets. No socket is opened.
fn stub_fetch(_method: &str, _url: &str, _body: &[u8], _limits: FetchLimits) -> FetchResult {
    FetchResult {
        status: 200,
        body: r#"{"status":"ok"}"#.to_string(),
        truncated: false,
        error: None,
    }
}

/// Deterministic stub for the native connector's health check. `ideas-map` is
/// reported DOWN (503) so the watcher alerts and the alerter recipe fires;
/// everything else is UP (200). Pure function of the url — no network.
fn stub_health(url: &str) -> HealthResult {
    if url.contains("ideas") {
        HealthResult::from_code(503)
    } else {
        HealthResult::from_code(200)
    }
}

// ---------------------------------------------------------------------------
// The snapshot schema (serde-serialized to the JSON documented above).
// ---------------------------------------------------------------------------

/// A complete, flat snapshot of the live core — the render contract for viewers.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Snapshot {
    /// Always `"xerboxion-rt map"` — identifies the producer.
    pub generated_by: String,
    /// The short git sha of the core this snapshot was produced from.
    pub core_commit: String,
    /// One-line description of the scenario that produced the trace.
    pub scenario: String,
    /// Every participant on the bus this run: wasm ploxions, native participants,
    /// and compiled recipes.
    pub ploxions: Vec<PloxionView>,
    /// The bus wiring as `provider -> topic -> subscribers` edges.
    pub bus: Vec<BusEdge>,
    /// The full event trace, in order.
    pub trace: Vec<TraceRow>,
    /// The deployed services the native connector bridged onto the bus.
    pub services: Vec<ServiceView>,
    /// **This run's tsoin root**: the hex Merkle/timeline root of recording THIS
    /// snapshot's trace (every hop, in order) as a tsoin timeline via the real
    /// engine — the thesis applied reflexively, so the live map can show
    /// "this run's tsoin root". Empty only if the trace was empty. Derived from
    /// the engine (`crate::recorder`), never faked; see `xerboxion-rt record` for
    /// the full record/replay/fork demonstration over the same trace.
    pub tsoin_root: String,
}

/// One participant (node in the graph). `kind` colours the node in the viewer.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct PloxionView {
    pub id: String,
    pub version: String,
    pub provides: Vec<String>,
    pub requires: Vec<String>,
    pub capabilities: Vec<String>,
    /// `"wasm"`, `"native"`, or `"recipe"`.
    pub kind: String,
    /// Live `plc_health` for wasm ploxions (0 = ok); 0 for native/recipe.
    pub health: i32,
}

/// One bus edge: a topic, the participant that provides it, and the subscribers
/// it is delivered to.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct BusEdge {
    pub topic: String,
    /// Provider id (`""` for a host-injected / externally-sourced topic).
    pub from: String,
    pub to: Vec<String>,
}

/// One traced hop, flattened to a uniform row. Fields that do not apply to a
/// given `kind` are empty strings.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct TraceRow {
    /// 0-based position in the journal.
    pub seq: usize,
    /// `"emit"`, `"route"`, `"log"`, `"life"`, `"fetch"`, or `"recipe"`.
    pub kind: String,
    /// The acting/source participant id.
    pub from: String,
    /// The bus topic (empty for log/life/recipe).
    pub topic: String,
    /// The payload / line / detail.
    pub payload: String,
    /// An extra note (route target, http status, lifecycle phase, …).
    pub note: String,
}

/// One bridged service with its (stubbed, deterministic) health result.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ServiceView {
    pub id: String,
    pub url: String,
    pub code: u16,
    pub up: bool,
}

// ---------------------------------------------------------------------------
// Scene construction + snapshot derivation.
// ---------------------------------------------------------------------------

/// A finished scene run: the LIVE [`Host`] (with its full [`Trace`] journal still
/// intact — NOT shut down), the connector sweep rows, and the per-ploxion health.
/// Returned by [`run_scene`] so callers can either derive a [`Snapshot`] or record
/// the host's own bus trace as a tsoin (the `record` subcommand). Holding the live
/// host means the trace is the EXACT ordered event stream the scenario produced.
pub struct SceneRun {
    /// The host after the scenario, before shutdown — `host.trace()` is the full
    /// ordered bus event stream.
    pub host: Host,
    /// The connector sweep rows (one per bridged service).
    pub rows: Vec<connector::SweepRow>,
    /// Per-ploxion live `plc_health` results (id, status).
    pub health: Vec<(String, i32)>,
}

/// Build the representative scene and RUN the scenario, returning the live host (so
/// its bus trace can be derived into a snapshot OR recorded as a tsoin). The whole
/// run is OFFLINE + deterministic (stubbed fetch + health). Does NOT shut the host
/// down — the caller owns it and may inspect/record its trace first.
pub fn run_scene(dir: &std::path::Path) -> Result<SceneRun> {
    // The host with the deterministic fetch stub linked (so the gated
    // health-adapter fetches through it instead of the live network).
    let mut host = Host::new().with_fetch_fn(stub_fetch);

    // Load the full example fleet, in a stable order, skipping any that the
    // build did not produce (so the scene degrades gracefully rather than failing).
    for name in [
        "ping",
        "pong",
        "tracer",
        "watcher",
        "health-adapter",
        "state-client",
        "tsoin",
    ] {
        let path = dir.join(format!("{name}.wasm"));
        if path.exists() {
            host.load_file(&path).map_err(|e| {
                anyhow::anyhow!("loading {} for the map scene: {e}", path.display())
            })?;
        }
    }
    if host.ploxions().is_empty() {
        return Err(anyhow::anyhow!(
            "no ploxions loaded from {} — run scripts/build-ploxions.sh first",
            dir.display()
        ));
    }

    // Register the sample designer recipe as a native reactive participant. We
    // subscribe the WASM tracer to its output topic so the recipe's emit is SEEN
    // routed to a real ploxion (exactly as the recipedemo does).
    let recipe = Recipe::from_json(SAMPLE_RECIPE.as_bytes())
        .map_err(|e| anyhow::anyhow!("sample recipe did not parse: {e}"))?;
    let compiled = host.register_recipe(&recipe);
    for topic in compiled.provides() {
        host.subscribe("tracer", topic);
    }

    // --- Run the scenario: init the whole fleet (drives ping->pong, the tsoin
    //     record/replay round trip, and the adapter's brokered fetch -> emit). --
    host.init_all()?;

    // A host tick, so the ping->pong loop and tracer show a driven hop too.
    host.inject("host", "tick", b"tick#1")?;

    // --- Bridge the deployed services onto the bus via the NATIVE connector.
    //     Each service.health cascades to the watcher AND the alerter recipe;
    //     the down one fires the recipe (alert.notify -> tracer). --------------
    let svc_list: Vec<Service> = SCENE_SERVICES
        .iter()
        .map(|(id, url)| Service {
            id: (*id).to_string(),
            url: (*url).to_string(),
        })
        .collect();
    let conn = ServiceConnector::new(svc_list);
    let rows = host.connector_sweep(&conn, &stub_health)?;

    // A health sweep so each wasm ploxion's live plc_health lands in the trace
    // and we can stamp it onto its node.
    let health = host.health_sweep()?;

    Ok(SceneRun { host, rows, health })
}

/// Build the representative scene, run the scenario, and derive the snapshot.
///
/// `dir` is the ploxion directory (`target/ploxions`); `core_commit` is the
/// short git sha to stamp into the snapshot. The whole run is OFFLINE and
/// deterministic (stubbed fetch + health), so the snapshot is reproducible.
pub fn build_snapshot(dir: &std::path::Path, core_commit: &str) -> Result<Snapshot> {
    let SceneRun {
        mut host,
        rows,
        health,
    } = run_scene(dir)?;

    // --- Derive the snapshot from the LIVE host state + trace. ----------------
    let snapshot = derive(&host, &rows, &health, core_commit);

    // Clean shutdown (droit au silence) — does not affect the already-derived
    // snapshot, but keeps the scene honest end to end.
    host.shutdown()?;

    Ok(snapshot)
}

/// Derive a [`Snapshot`] from ANY live [`Host`] — the reusable contract the
/// persistent daemon serves at `GET /snapshot`. Unlike [`build_snapshot`] this
/// does NOT construct or run a scene: it photographs the host EXACTLY as it is
/// right now (its loaded ploxions, current bus wiring, native participants,
/// recipes, and the full trace journal so far). The connector sweep rows are not
/// part of a generic host's state, so `services` is left empty; `health` is the
/// last-known per-ploxion status the caller passes in (or empty for `0`s).
///
/// Used by the daemon's host thread, which owns the `Host` on one thread and can
/// therefore hand a `&Host` here safely (wasmtime `Store`s are not `Sync`).
pub fn snapshot_of(host: &Host, health: &[(String, i32)], core_commit: &str) -> Snapshot {
    derive(host, &[], health, core_commit)
}

/// Turn the finished host run into the flat [`Snapshot`]. Every field is read
/// from real state: manifests, the bus, the native participants, the recipes,
/// the sweep rows, and the trace journal.
fn derive(
    host: &Host,
    rows: &[connector::SweepRow],
    health: &[(String, i32)],
    core_commit: &str,
) -> Snapshot {
    // 1. Ploxion views: wasm modules, then native participants, then recipes.
    let mut ploxions: Vec<PloxionView> = Vec::new();

    for p in host.ploxions() {
        let m = &p.manifest;
        let h = health
            .iter()
            .find(|(id, _)| id == &m.id)
            .map(|(_, s)| *s)
            .unwrap_or(0);
        ploxions.push(PloxionView {
            id: m.id.clone(),
            version: m.version.clone(),
            provides: m.provides.clone(),
            requires: m.requires.clone(),
            capabilities: m.capabilities.clone(),
            kind: "wasm".to_string(),
            health: h,
        });
    }

    // Recipe ids (so a native participant that is actually a recipe is tagged
    // `recipe`, not `native`).
    let recipe_ids: Vec<&str> = host.recipes().iter().map(|r| r.id.as_str()).collect();

    for (id, provides) in host.native_participants() {
        let is_recipe = recipe_ids.contains(&id.as_str());
        // A recipe's `requires` (trigger topics) come from the compiled rule.
        let requires = if is_recipe {
            host.recipes()
                .iter()
                .find(|r| &r.id == id)
                .map(|r| r.requires().to_vec())
                .unwrap_or_default()
        } else {
            Vec::new()
        };
        ploxions.push(PloxionView {
            id: id.clone(),
            version: String::new(),
            provides: provides.clone(),
            requires,
            capabilities: Vec::new(),
            kind: if is_recipe { "recipe" } else { "native" }.to_string(),
            health: 0,
        });
    }

    // 2. Bus edges: for every topic with subscribers, attribute a provider.
    //    A provider is the wasm ploxion / native participant / recipe that
    //    declares the topic in `provides`. Topics with no declared provider
    //    (host-injected `tick`) get an empty `from`.
    let mut bus: Vec<BusEdge> = Vec::new();
    let mut topics: Vec<(&String, &Vec<String>)> = host.bus().wiring().collect();
    topics.sort_by(|a, b| a.0.cmp(b.0));
    for (topic, subs) in topics {
        let from = provider_of(host, topic);
        let mut to = subs.clone();
        to.sort();
        bus.push(BusEdge {
            topic: topic.clone(),
            from,
            to,
        });
    }
    // Also surface PROVIDED topics that currently have no subscriber (a real
    // edge of the wiring — the topic exists on the bus even if nothing listens),
    // so the graph shows every provide->topic relationship, not only the ones
    // with a live subscriber.
    let mut provided_no_sub: Vec<(String, String)> = Vec::new();
    for p in host.ploxions() {
        for t in &p.manifest.provides {
            if host.bus().subscribers(t).is_empty() && !bus.iter().any(|e| &e.topic == t) {
                provided_no_sub.push((t.clone(), p.id().to_string()));
            }
        }
    }
    provided_no_sub.sort();
    provided_no_sub.dedup();
    for (topic, from) in provided_no_sub {
        bus.push(BusEdge {
            topic,
            from,
            to: Vec::new(),
        });
    }

    // 3. Trace rows: flatten the journal in order, BUT cap the RESPONSE to the
    //    most-recent `TRACE_RESPONSE_CAP` hops. The host's real trace journal and
    //    the tsoin recorder are UNTOUCHED — only this serialized payload is bounded
    //    so an ever-growing journal cannot bloat (and slow) every `/snapshot`.
    //    Each row keeps its real `seq` (the journal index), so clients still see a
    //    faithful global index even though older rows are elided.
    let full_len = host.trace().len();
    let trace: Vec<TraceRow> = host
        .trace()
        .iter()
        .enumerate()
        .skip(full_len.saturating_sub(TRACE_RESPONSE_CAP))
        .map(|(seq, t)| trace_row(seq, t))
        .collect();

    // 4. Services: the sweep rows.
    let services: Vec<ServiceView> = rows
        .iter()
        .map(|r| ServiceView {
            id: r.id.clone(),
            url: r.url.clone(),
            code: r.result.code,
            up: r.result.up,
        })
        .collect();

    // 5. This run's tsoin root: record the SAME ordered trace as a tsoin timeline
    //    through the real engine and read the head/root hash back out. The map
    //    becomes reflexive — it shows the root identity of the core's own life
    //    this run. Empty string for an empty trace.
    //    HOT-PATH NOTE: we use the cheap `trace_root` (BLAKE3/delta recording only)
    //    instead of `record_trace` + `stats`, because `stats` runs zstd-19 over the
    //    whole ever-growing delta store (O(trace) per call) just to compute
    //    footprints we never read here. Same root identity, none of the zstd cost.
    let tsoin_root = crate::recorder::trace_root(host.trace());

    Snapshot {
        generated_by: "xerboxion-rt map".to_string(),
        core_commit: core_commit.to_string(),
        scenario: SCENARIO.to_string(),
        ploxions,
        bus,
        trace,
        services,
        tsoin_root,
    }
}

/// Find the id of the participant that *provides* `topic` (a wasm ploxion, a
/// native participant, or a recipe). Empty string when none declares it (the
/// topic is sourced externally, e.g. a host-injected `tick`).
fn provider_of(host: &Host, topic: &str) -> String {
    if let Some(p) = host
        .ploxions()
        .iter()
        .find(|p| p.manifest.provides.iter().any(|t| t == topic))
    {
        return p.id().to_string();
    }
    if let Some((id, _)) = host
        .native_participants()
        .find(|(_, topics)| topics.iter().any(|t| t == topic))
    {
        return id.clone();
    }
    String::new()
}

/// Flatten one [`Trace`] into a uniform [`TraceRow`].
fn trace_row(seq: usize, t: &Trace) -> TraceRow {
    match t {
        Trace::Emit {
            from,
            topic,
            payload,
        } => TraceRow {
            seq,
            kind: "emit".to_string(),
            from: from.clone(),
            topic: topic.clone(),
            payload: payload.clone(),
            note: String::new(),
        },
        Trace::Deliver { from, to, topic } => TraceRow {
            seq,
            kind: "route".to_string(),
            from: from.clone(),
            topic: topic.clone(),
            payload: String::new(),
            note: format!("=> {to}"),
        },
        Trace::Log { from, line } => TraceRow {
            seq,
            kind: "log".to_string(),
            from: from.clone(),
            topic: String::new(),
            payload: line.clone(),
            note: String::new(),
        },
        Trace::Lifecycle { who, what } => TraceRow {
            seq,
            kind: "life".to_string(),
            from: who.clone(),
            topic: String::new(),
            payload: what.clone(),
            note: String::new(),
        },
        Trace::Fetch {
            from,
            method,
            url,
            status,
        } => TraceRow {
            seq,
            kind: "fetch".to_string(),
            from: from.clone(),
            topic: String::new(),
            payload: format!("{method} {url}"),
            note: format!("status {status}"),
        },
        Trace::Recipe { who, step } => TraceRow {
            seq,
            kind: "recipe".to_string(),
            from: who.clone(),
            topic: String::new(),
            payload: step.clone(),
            note: String::new(),
        },
    }
}

impl Snapshot {
    /// Serialize to pretty JSON (for `--json` / the baked HTML payload).
    pub fn to_json_pretty(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".to_string())
    }
}
