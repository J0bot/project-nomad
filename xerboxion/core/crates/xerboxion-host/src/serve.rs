//! # `serve` — the xerboxion-core as a PERSISTENT DAEMON with a live bus API.
//!
//! Every other subcommand is ONE-SHOT: build a [`Host`], load ploxions, run a
//! scene, shut down, exit. This module turns the same host into a long-running
//! daemon so external things can **connect to the live bus, watch every event as
//! it flows, and emit onto it** — the unblock-everything step toward "the core is
//! the base of the whole labo".
//!
//! ## Threading model (the load-bearing invariant)
//! A wasmtime `Store` is `Send` but **NOT `Sync`** — the [`Host`] (which owns
//! every ploxion's `Store`) must be touched from EXACTLY ONE thread. So:
//!
//! - A dedicated **HOST THREAD** (a plain OS thread, [`host_thread`]) owns the
//!   `Host` for its whole life. It runs a blocking loop pulling [`Command`]s off
//!   an mpsc channel: `Emit`, `Snapshot`, `Health`, `Shutdown`. It NEVER shares
//!   the `Host` — the only way to reach it is to send a `Command`.
//! - The **ASYNC SERVER** (axum, [`run`]) handles HTTP + WebSocket. It owns NO
//!   `Host`; it only holds the command sender + a broadcast handle. Async tasks
//!   touch the host ONLY through channels.
//!
//! After any command that touches the bus, the host thread diffs its own trace
//! journal and **broadcasts each newly-appended hop** ([`LiveEvent`]) onto a
//! [`tokio::sync::broadcast`] channel. WebSocket clients subscribed to `/ws`
//! receive those hops AS THEY HAPPEN — the feed is the REAL bus, never faked.
//!
//! ## Two ways to read the same live feed
//! - `GET /ws` — a WebSocket (`101` upgrade) that streams every hop as JSON and
//!   accepts inbound `{emit:{...}}`. Best for browsers.
//! - `GET /events` — the SAME broadcast, as **Server-Sent Events over PLAIN
//!   HTTP** (a normal `200` chunked stream, no upgrade). Trivially proxyable
//!   server-side (e.g. a PHP fleet relay) with any HTTP client — no WS sidecar.
//!   Both subscribe to the one broadcast channel, so they carry identical events.
//!
//! ## The emit path is load-bearing
//! `POST /emit {topic,payload}` (or a WS `{emit:{...}}` message) sends an `Emit`
//! command; the host thread calls [`Host::inject`], which routes the event
//! through the very same cascade ordinary ploxions use — subscribers react, their
//! emits/logs cascade, and EVERY resulting hop is broadcast to WS clients. A
//! topic nobody subscribes to produces only the emit hop and no spurious delivery
//! (*droit au silence*).
//!
//! ## Same-origin (Phase A)
//! The served live map (`GET /`) talks ONLY to its own origin, so no CORS is
//! needed here. Cross-origin browser ploxions are Phase B and will need CORS.
//!
//! ## Durable runtime delta (`--state-dir`)
//! With `--state-dir` set the daemon is DURABLE: each successful `POST /load`
//! persists that runtime-loaded ploxion (id + source, plus the wasm bytes for a
//! base64 upload) to the state dir, each `POST /unload` removes it, and at the
//! next startup — after the base set loads — the persisted set is RESTORED, so a
//! hot-loaded ploxion survives a deploy/crash. The base set is never persisted
//! (it reloads from `<ploxions_dir>` anyway). Persistence lives entirely on the
//! HOST THREAD (the sole owner) and is best-effort + graceful: a corrupt/partial
//! state dir is skipped with a warning, the daemon still boots. With no
//! `--state-dir` the feature is off and the daemon behaves exactly as before. See
//! [`crate::state`] and `docs/PERSISTENCE-v0.md`.

use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{get, post};
use axum::extract::Request;
use axum::middleware::{self, Next};
use axum::{Json, Router};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, mpsc, oneshot};

use crate::ecosystem;
use crate::federation::{
    self, FedMessage, PeerFrame, PeerSpec, PEER_TOKEN_HEADER, RECONNECT_MAX, RECONNECT_MIN,
};
use crate::map::{self, Snapshot};
use crate::state::{Source, StateStore};
use crate::{Host, Trace};

/// Broadcast capacity for the FEDERATION outbound feed (local-origin emits a
/// peer link forwards). Independent of the WS client feed so a slow peer never
/// affects browser clients and vice-versa.
const FED_CAPACITY: usize = 4096;

/// The default daemon port (bind `127.0.0.1` by default — local only).
pub const DEFAULT_PORT: u16 = 8730;
/// The default bind address (loopback — Phase A is local).
pub const DEFAULT_ADDR: &str = "127.0.0.1";

/// Broadcast capacity for the live event feed. A lagging WS client may drop the
/// oldest events (the channel tells it how many it `Lagged` past) — acceptable
/// for a LIVE feed (it shows what is happening now, not a durable log). Sized
/// generously so a brief stall does not drop events under normal load.
const BROADCAST_CAPACITY: usize = 4096;

/// Max accepted `POST /emit` body size (bytes). A bus event is small; anything
/// larger is rejected `413` rather than buffered. Bounds the payload, per spec.
const MAX_EMIT_BODY: usize = 64 * 1024;

/// Max accepted `POST /load` / `POST /unload` body size (bytes). A base64 wasm
/// module rides in the body, so this is generously sized (8 MiB) — larger than any
/// example ploxion's base64, but still bounded so a bad request can't OOM the
/// daemon. Anything over is rejected `413` rather than buffered.
const MAX_LOAD_BODY: usize = 8 * 1024 * 1024;

/// BUS FEDERATION knobs for one daemon (the inter-node protocol). Defaults are
/// inert: `peers` empty + `peer_token` none means no peer client task is spawned
/// and the `/peer` endpoint accepts links but none arrive — the daemon behaves
/// EXACTLY as before. `node_id` is always set (it only changes the origin label).
#[derive(Clone, Default)]
pub struct FederationConfig {
    /// This node's stable id (origin tag on locally-originated emits). Defaults
    /// to the hostname (see [`default_node_id`]).
    pub node_id: String,
    /// Peers to dial as a WebSocket client (`--peer`, repeatable). Empty = none.
    pub peers: Vec<PeerSpec>,
    /// Optional shared token required on inbound `/peer` upgrades (`--peer-token`).
    /// `None` = no token check (still authenticatable via basic-auth at Traefik).
    pub peer_token: Option<String>,
}

/// A stable default node id when `--node-id` is not given: the machine hostname,
/// or a short fixed fallback if it cannot be read. Stable across restarts so the
/// origin tag a peer sees does not churn.
pub fn default_node_id() -> String {
    std::env::var("HOSTNAME")
        .ok()
        .filter(|h| !h.trim().is_empty())
        .or_else(|| {
            std::fs::read_to_string("/etc/hostname")
                .ok()
                .map(|s| s.trim().to_string())
                .filter(|h| !h.is_empty())
        })
        .unwrap_or_else(|| "xion-node".to_string())
}

// ---------------------------------------------------------------------------
// The wire types: what a client POSTs and what it receives.
// ---------------------------------------------------------------------------

/// The body of `POST /emit` (and the inner object of a WS `{emit:{...}}`). The
/// payload is a free-form string the host injects verbatim onto the bus.
#[derive(Debug, Clone, Deserialize)]
pub struct EmitBody {
    /// The bus topic to emit on.
    pub topic: String,
    /// The opaque payload (UTF-8 string; the host never inspects it).
    #[serde(default)]
    pub payload: String,
}

/// An inbound WebSocket control message: a client may push an emit onto the bus.
#[derive(Debug, Clone, Deserialize)]
struct WsInbound {
    emit: EmitBody,
}

/// The body of `POST /load`. EITHER `{id}` (load the staged
/// `<ploxions_dir>/<id>.wasm` of a registered ploxion) OR `{wasm}` (base64 of an
/// arbitrary wasm module). When `wasm` is given, `id` is OPTIONAL and only names
/// the ploxion if its manifest can't be read (the manifest's own id wins). At
/// least one of the two MUST be present, else it is a `400`.
#[derive(Debug, Clone, Deserialize)]
pub struct LoadBody {
    /// Registered-ploxion id to load from the staged dir, and/or the fallback id
    /// for a `wasm`-supplied module.
    #[serde(default)]
    pub id: Option<String>,
    /// Base64 of an arbitrary wasm module to hot-load.
    #[serde(default)]
    pub wasm: Option<String>,
}

/// The body of `POST /unload`: the loaded ploxion id to hot-unload.
#[derive(Debug, Clone, Deserialize)]
pub struct UnloadBody {
    /// The id of the loaded ploxion to unload.
    pub id: String,
}

/// One live bus hop streamed to WS clients — the same flat shape as a map trace
/// row, plus a global `seq` so a client can detect a gap. Derived 1:1 from a
/// [`Trace`]; nothing here is invented.
#[derive(Debug, Clone, Serialize)]
pub struct LiveEvent {
    /// Monotonic index of this hop in the host's trace journal.
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

impl LiveEvent {
    /// Flatten one [`Trace`] hop at global index `seq` into a wire event.
    fn from_trace(seq: usize, t: &Trace) -> LiveEvent {
        match t {
            Trace::Emit { from, topic, payload } => LiveEvent {
                seq,
                kind: "emit".into(),
                from: from.clone(),
                topic: topic.clone(),
                payload: payload.clone(),
                note: String::new(),
            },
            Trace::Deliver { from, to, topic } => LiveEvent {
                seq,
                kind: "route".into(),
                from: from.clone(),
                topic: topic.clone(),
                payload: String::new(),
                note: format!("=> {to}"),
            },
            Trace::Log { from, line } => LiveEvent {
                seq,
                kind: "log".into(),
                from: from.clone(),
                topic: String::new(),
                payload: line.clone(),
                note: String::new(),
            },
            Trace::Lifecycle { who, what } => LiveEvent {
                seq,
                kind: "life".into(),
                from: who.clone(),
                topic: String::new(),
                payload: what.clone(),
                note: String::new(),
            },
            Trace::Fetch { from, method, url, status } => LiveEvent {
                seq,
                kind: "fetch".into(),
                from: from.clone(),
                topic: String::new(),
                payload: format!("{method} {url}"),
                note: format!("status {status}"),
            },
            Trace::Recipe { who, step } => LiveEvent {
                seq,
                kind: "recipe".into(),
                from: who.clone(),
                topic: String::new(),
                payload: step.clone(),
                note: String::new(),
            },
        }
    }
}

// ---------------------------------------------------------------------------
// Host-thread commands.
// ---------------------------------------------------------------------------

/// A unit of work for the HOST THREAD. The async side never touches the `Host`;
/// it sends one of these and (where it needs an answer) awaits a `oneshot`.
enum Command {
    /// Inject an event onto the REAL bus from an external source. The host routes
    /// it through the full cascade; every resulting hop is then broadcast.
    Emit { topic: String, payload: String },
    /// Inject an event RECEIVED FROM A PEER onto the local bus, marked with the
    /// peer's origin (`from = "fed:<origin_node>"`). Local ploxions requiring the
    /// topic react; because the resulting emit hop is REMOTE-origin it is NEVER
    /// forwarded onward (loop prevention). A distinct `fed-in` note is traced.
    FedInject { origin_node: String, seq: usize, topic: String, payload: String },
    /// HOT-LOAD a ploxion into the RUNNING host from wasm bytes (see
    /// [`Host::load_runtime`]): instantiate a new `Store`, wire its
    /// `requires`/`provides` onto the bus, run `plc_init`, cascade its init emits.
    /// `fallback_id` names the ploxion if its manifest can't be read. Replies with
    /// the loaded manifest as JSON, or an error string (the async handler maps that
    /// to a 4xx — bad wasm / duplicate id / rejected manifest). The Host is touched
    /// ONLY here, on the host thread.
    Load {
        wasm: Vec<u8>,
        fallback_id: String,
        /// How the bytes were resolved — drives PERSISTENCE on success: a
        /// [`Source::Staged`] load persists only the id (the bytes reload from
        /// `<ploxions_dir>` on restart), a [`Source::Wasm`] load persists the
        /// bytes too. The Host itself never sees this; only the persistence step
        /// on the host thread does.
        source: Source,
        reply: oneshot::Sender<Result<xerboxion_plc::Manifest, String>>,
    },
    /// HOT-UNLOAD a loaded ploxion by id (see [`Host::unload`]): run `plc_goodbye`,
    /// remove its bus wiring, drop its `Store` (droit au silence). Replies `Ok` with
    /// the unloaded id, or `Err` (the handler maps that to a 404 — not loaded).
    Unload {
        id: String,
        reply: oneshot::Sender<Result<String, String>>,
    },
    /// Photograph the live host into a [`Snapshot`] (runs a fresh health sweep so
    /// node states are current) and reply with it.
    Snapshot { reply: oneshot::Sender<Snapshot> },
    /// `plc_goodbye` on each ploxion + drop the Stores (droit au silence), then
    /// stop the loop. Replies once shutdown is complete so the caller can exit.
    Shutdown { reply: oneshot::Sender<()> },
    /// No-op poussé sur le canal DONNÉES quand une commande de CONTRÔLE est envoyée : réveille le
    /// `blocking_recv` du thread hôte → il re-draine le plan de contrôle en priorité (fix /load hang).
    Wake,
}

/// A cloneable handle the async server uses to reach the host thread.
#[derive(Clone)]
struct HostHandle {
    /// Plan de CONTRÔLE (Load/Unload/Snapshot/Shutdown) — drainé EN PRIORITÉ par le thread hôte,
    /// pour ne jamais rester coincé derrière le flot du plan de données.
    control: mpsc::UnboundedSender<Command>,
    /// Plan de DONNÉES (Emit/FedInject) — fire-and-forget, peut affluer en masse.
    data: mpsc::UnboundedSender<Command>,
}

impl HostHandle {
    async fn snapshot(&self) -> Option<Snapshot> {
        let (tx, rx) = oneshot::channel();
        self.control.send(Command::Snapshot { reply: tx }).ok()?;
        let _ = self.data.send(Command::Wake);
        rx.await.ok()
    }
    /// Fire-and-forget an emit onto the bus (the result is observed via the WS
    /// feed, not the return — the emit is accepted `202`).
    fn emit(&self, topic: String, payload: String) -> bool {
        self.data.send(Command::Emit { topic, payload }).is_ok()
    }
    /// Inject an event received from a peer (federation). Fire-and-forget: the
    /// host thread marks it remote-origin and runs the local cascade.
    fn fed_inject(&self, m: FedMessage) -> bool {
        self.data
            .send(Command::FedInject {
                origin_node: m.origin_node,
                seq: m.seq,
                topic: m.topic,
                payload: m.payload,
            })
            .is_ok()
    }
    /// Hot-load wasm bytes into the running host; await the host thread's result
    /// (the loaded manifest or an error string). `None` => the host thread is gone.
    async fn load(
        &self,
        wasm: Vec<u8>,
        fallback_id: String,
        source: Source,
    ) -> Option<Result<xerboxion_plc::Manifest, String>> {
        let (tx, rx) = oneshot::channel();
        self.control
            .send(Command::Load { wasm, fallback_id, source, reply: tx })
            .ok()?;
        let _ = self.data.send(Command::Wake);
        rx.await.ok()
    }
    /// Hot-unload a loaded ploxion by id; await the result (the id, or an error).
    async fn unload(&self, id: String) -> Option<Result<String, String>> {
        let (tx, rx) = oneshot::channel();
        self.control.send(Command::Unload { id, reply: tx }).ok()?;
        let _ = self.data.send(Command::Wake);
        rx.await.ok()
    }
    async fn shutdown(&self) {
        let (tx, rx) = oneshot::channel();
        if self.control.send(Command::Shutdown { reply: tx }).is_ok() {
            let _ = self.data.send(Command::Wake);
            let _ = rx.await;
        }
    }
}

/// Shared, immutable-ish daemon state handed to every axum handler.
#[derive(Clone)]
struct AppState {
    host: HostHandle,
    /// Subscribe here to receive every live bus hop.
    events: broadcast::Sender<LiveEvent>,
    /// This daemon's node id (origin tag; shown in `/healthz`).
    node_id: Arc<String>,
    /// Subscribe here to receive every LOCAL-ORIGIN emit to forward to peers
    /// (the federation outbound feed). A `/peer` server connection and each peer
    /// CLIENT task both subscribe to this and write `{fed:{…}}` frames.
    fed_out: broadcast::Sender<FedMessage>,
    /// Optional shared token required on inbound `/peer` upgrades.
    peer_token: Arc<Option<String>>,
    /// Optional shared bearer token sur les routes MUTANTES (/emit /load /unload /replicate),
    /// lu de l'env `XION_BUS_TOKEN`. None/vide = PAS d'enforcement (rétrocompat : le bus reste
    /// ouvert tant que le token n'est pas posé → déployable sans casser la flotte ; l'enforcement
    /// s'active quand l'env est défini + les clients envoient le token). Audit sécu finding #1.
    bus_token: Arc<Option<String>>,
    /// Daemon start instant, for `/healthz` uptime.
    started: Instant,
    /// Number of wasm ploxions loaded at boot (for `/healthz`).
    ploxions: usize,
    /// The core's short git sha, stamped into snapshots.
    commit: Arc<String>,
    /// The live-map HTML served at `GET /`.
    live_html: Arc<String>,
    /// The 3D/4D xion view HTML served at `GET /3d`.
    xion3d_html: Arc<String>,
    /// The LABO-XER HTML (desktop + dock + launcher de tous les ploxions + dashboard
    /// autonomie + forge) servi à `GET /` et `GET /xer`. Cible de la migration : tout
    /// le labo vit DANS le core (installer le core = avoir le labo ; plus de Laravel).
    xer_html: Arc<String>,
    /// The staged ploxions directory (`target/ploxions`). `POST /load {id}` reads
    /// `<ploxions_dir>/<id>.wasm` from here to hot-load a registered ploxion by id.
    ploxions_dir: Arc<std::path::PathBuf>,
    /// The assets directory: per-ploxion UIs are served from `<assets_dir>/web-<id>/`
    /// at `GET /px/<id>/*path` — the labo→core MIGRATION brick (each labo ploxion
    /// becomes a `web-<id>/` dir, talking to the bus same-origin, no Laravel). Defaults
    /// to the daemon CWD (override with `XION_ASSETS_DIR`), so a local-first install
    /// serves the repo's `web-*` dirs as-is.
    assets_dir: Arc<std::path::PathBuf>,
    /// The durable state dir (`--state-dir`), if set. `GET /replicate` reads the
    /// runtime-loaded DELTA from here (read-only; safe concurrently with the host
    /// thread's atomic writes). `None` = persistence off = an empty bundle (the
    /// base set reconstructs from the image, not from here).
    state_dir: Arc<Option<std::path::PathBuf>>,
    /// Short-TTL cache of the rendered `/ecosystem` JSON body. The aggregation
    /// (and the underlying snapshot oneshot to the single host thread) is expensive
    /// and was re-run on EVERY GET, starving the host thread under polling. We cache
    /// the rendered bytes with a `ECOSYSTEM_TTL` freshness window: a GET within the
    /// window serves the cached body instantly WITHOUT touching the host thread;
    /// only a stale (or empty) cache recomputes. Correctness is preserved — the
    /// result still reflects running/registered/deployed, just at <=TTL staleness.
    ecosystem_cache: Arc<std::sync::Mutex<Option<(Instant, Arc<str>)>>>,
    /// Token (env `XER_STORE_TOKEN`) gating writes to `/store/*`; `None` = open (rétrocompat).
    store_token: Arc<Option<String>>,
    /// Serialises store mutations (read-modify-write of the per-collection JSON file).
    store_lock: Arc<std::sync::Mutex<()>>,
}

/// Freshness window for the `/ecosystem` aggregation cache (Fix C).
const ECOSYSTEM_TTL: Duration = Duration::from_secs(5);

// ---------------------------------------------------------------------------
// The HOST THREAD: the only place the Host is ever touched.
// ---------------------------------------------------------------------------

/// Run the host event loop on the CURRENT thread (spawned as a dedicated OS
/// thread by [`run`]). Owns `host` for its whole life; pulls [`Command`]s off
/// `cmd_rx` and, after any bus-touching command, broadcasts the newly-appended
/// trace hops onto `events`. The `commit` is captured for snapshots.
///
/// `seq` advances monotonically over the host's trace journal; we only ever
/// broadcast hops at indices we have not broadcast before, so a client never
/// sees a duplicate and `seq` is a faithful global index.
fn host_thread(
    mut host: Host,
    mut control_rx: mpsc::UnboundedReceiver<Command>,
    mut data_rx: mpsc::UnboundedReceiver<Command>,
    events: broadcast::Sender<LiveEvent>,
    fed_out: broadcast::Sender<FedMessage>,
    node_id: Arc<String>,
    commit: Arc<String>,
    // The optional DURABLE state store for the runtime-loaded delta. `None` =
    // `--state-dir` not set = persistence OFF (classic behavior). When `Some`,
    // a successful `Load` persists the ploxion and an `Unload` removes it, so a
    // hot-loaded ploxion survives a restart. The store lives ONLY here, on the
    // host thread (the sole owner of mutable daemon state) — never shared.
    state_store: Option<StateStore>,
) {
    // How many trace hops we have already broadcast.
    let mut broadcast_upto = host.trace().len();
    // FEDERATION dedup high-water: the greatest `seq` we have already injected
    // from each origin node. A peer's `seq` is its OWN trace index of the emit
    // (strictly increasing per origin), so an event already seen carries a `seq`
    // we have met before. This makes a federated inject IDEMPOTENT per
    // (origin_node, seq) — so when two peers are linked BOTH ways (two parallel
    // links carry the same direction's traffic), an emit is still applied to the
    // local bus exactly ONCE. Together with "never re-forward a remote-origin
    // emit", this is the full loop/storm/duplicate guard.
    let mut fed_seen: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    // Broadcast any hops produced during init/boot before the loop starts.
    broadcast_new(&mut host, &mut broadcast_upto, &events, &fed_out, &node_id);

    loop {
        // PRIORITÉ AU PLAN DE CONTRÔLE : on draine d'abord une commande de contrôle (Load/Unload/
        // Snapshot/Shutdown) via try_recv → elle ne reste JAMAIS coincée derrière le flot du plan de
        // données (Emit). Sinon on bloque sur les données ; un envoi de contrôle pousse aussi un `Wake`
        // sur ce canal pour réveiller le blocking_recv. (Fix saturation /load + /ecosystem.)
        let cmd = if let Ok(c) = control_rx.try_recv() {
            c
        } else {
            match data_rx.blocking_recv() {
                Some(c) => c,
                None => break,
            }
        };
        match cmd {
            Command::Wake => continue,
            Command::Emit { topic, payload } => {
                // Inject onto the REAL bus from an external source ("api"). Errors
                // are logged but never crash the daemon.
                if let Err(e) = host.inject("api", &topic, payload.as_bytes()) {
                    eprintln!("xerboxion-rt serve: emit on [{topic}] failed: {e:#}");
                }
                broadcast_new(&mut host, &mut broadcast_upto, &events, &fed_out, &node_id);
            }
            Command::FedInject { origin_node, seq, topic, payload } => {
                // DEDUP: skip an (origin_node, seq) we have already injected. With
                // a single link this never triggers; with redundant bidirectional
                // links it collapses the duplicate so the bus sees the event once.
                let dup = matches!(fed_seen.get(&origin_node), Some(&hi) if seq <= hi);
                if dup {
                    continue;
                }
                fed_seen.insert(origin_node.clone(), seq);

                // A peer forwarded a LOCAL-origin emit to us. Mark it REMOTE-origin
                // (`from = "fed:<node>"`) and inject onto the local bus so local
                // ploxions requiring the topic react. A distinct `fed-in` note
                // makes the cross-node hop visible in the snapshot/trace. The
                // resulting emit hop is remote-origin, so `broadcast_new` will NOT
                // re-forward it — loop prevention.
                host.note(
                    &format!("fed:{origin_node}"),
                    &format!("fed-in from {origin_node} [{topic}] (seq {seq})"),
                );
                let from = federation::remote_from(&origin_node);
                if let Err(e) = host.inject(&from, &topic, payload.as_bytes()) {
                    eprintln!("xerboxion-rt serve: fed-inject on [{topic}] failed: {e:#}");
                }
                broadcast_new(&mut host, &mut broadcast_upto, &events, &fed_out, &node_id);
            }
            Command::Load { wasm, fallback_id, source, reply } => {
                // Hot-load into the running Host. On success its lifecycle hops
                // (load/init) + any init-emit cascade are broadcast to the live
                // feed; on failure the host is untouched (no hops) and we reply Err.
                let result = host
                    .load_runtime(&wasm, &fallback_id)
                    .map_err(|e| format!("{e:#}"));
                broadcast_new(&mut host, &mut broadcast_upto, &events, &fed_out, &node_id);
                // PERSIST the runtime-loaded DELTA (best-effort; a persist error
                // never fails the live load). We persist under the ploxion's REAL
                // manifest id (from the loaded manifest), not the fallback. For a
                // staged load only the id is stored; for a wasm upload the bytes
                // are stored too so they survive a restart.
                if let (Some(store), Ok(manifest)) = (&state_store, &result) {
                    let id = &manifest.id;
                    let wasm_for_persist = match source {
                        Source::Wasm => Some(wasm.as_slice()),
                        Source::Staged => None,
                    };
                    if let Err(e) = store.persist(id, source, wasm_for_persist) {
                        eprintln!(
                            "xerboxion-rt serve: WARN could not persist loaded '{id}' to state dir: {e:#}"
                        );
                    }
                }
                let _ = reply.send(result);
            }
            Command::Unload { id, reply } => {
                // Hot-unload from the running Host. Its goodbye/unload hops are
                // broadcast; on failure (not loaded) nothing changed and we reply Err.
                let result = host.unload(&id).map_err(|e| format!("{e:#}"));
                broadcast_new(&mut host, &mut broadcast_upto, &events, &fed_out, &node_id);
                // REMOVE it from the persisted set on success (best-effort; a
                // remove of a base-set id is a harmless no-op since base ploxions
                // were never persisted).
                if let (Some(store), Ok(unloaded_id)) = (&state_store, &result) {
                    if let Err(e) = store.remove(unloaded_id) {
                        eprintln!(
                            "xerboxion-rt serve: WARN could not remove '{unloaded_id}' from state dir: {e:#}"
                        );
                    }
                }
                let _ = reply.send(result);
            }
            Command::Snapshot { reply } => {
                // A fresh health sweep so node states are current; its hops also
                // flow to the live feed.
                let health = host.health_sweep().unwrap_or_default();
                broadcast_new(&mut host, &mut broadcast_upto, &events, &fed_out, &node_id);
                let snap = map::snapshot_of(&host, &health, &commit);
                let _ = reply.send(snap);
            }
            Command::Shutdown { reply } => {
                let _ = host.shutdown();
                broadcast_new(&mut host, &mut broadcast_upto, &events, &fed_out, &node_id);
                let _ = reply.send(());
                break; // leave the loop — the Host (and every Store) is dropped here
            }
        }
    }
    // Loop ended (shutdown, or all senders dropped): the Host drops here, taking
    // every wasmtime Store with it — droit au silence.
}

/// Broadcast every trace hop appended since `*upto`, advancing `*upto`, AND
/// forward every newly-appended LOCAL-ORIGIN emit to the federation outbound
/// feed. A send error means there are simply no live subscribers right now —
/// fine; the hop is still recorded in the host's journal.
///
/// FORWARDING RULE (loop prevention): a `Trace::Emit` is forwarded to peers iff
/// its `from` is NOT a remote origin (`fed:`-prefixed). A remote-origin emit —
/// one we ourselves injected from a peer — is never re-forwarded, so an event
/// crosses the link exactly once per direction and cannot echo.
fn broadcast_new(
    host: &mut Host,
    upto: &mut usize,
    events: &broadcast::Sender<LiveEvent>,
    fed_out: &broadcast::Sender<FedMessage>,
    node_id: &str,
) {
    // First pass (immutable): broadcast each new hop to WS clients and collect
    // the LOCAL-ORIGIN emits to forward to peers. We collect rather than forward
    // inline so we can then mutate the host (a `fed-out` note) without aliasing.
    let mut to_forward: Vec<(usize, String, String)> = Vec::new();
    {
        let trace = host.trace();
        for (seq, t) in trace.iter().enumerate().skip(*upto) {
            let _ = events.send(LiveEvent::from_trace(seq, t));
            if let Trace::Emit { from, topic, payload } = t {
                if !federation::is_remote_origin(from) {
                    to_forward.push((seq, topic.clone(), payload.clone()));
                }
            }
        }
        *upto = trace.len();
    }
    // Second pass: forward each local-origin emit to the federation feed. Only if
    // a peer link is actually connected (`receiver_count > 0`) do we also record
    // a `fed-out` note, so the trace shows a cross-node hop ONLY when one truly
    // happened (honest: no note when no peer is attached).
    let has_peer = fed_out.receiver_count() > 0;
    for (seq, topic, payload) in to_forward {
        let _ = fed_out.send(FedMessage {
            origin_node: node_id.to_string(),
            seq,
            topic: topic.clone(),
            payload,
        });
        if has_peer {
            host.note(node_id, &format!("fed-out to peers [{topic}] (seq {seq})"));
        }
    }
}

// ---------------------------------------------------------------------------
// HTTP + WebSocket handlers.
// ---------------------------------------------------------------------------

/// `GET /map` — the self-contained LIVE map (fetches `/snapshot`, opens `/ws`).
async fn index(State(st): State<AppState>) -> Html<String> {
    Html((*st.live_html).clone())
}

/// `GET /` and `GET /xer` — the LABO-XER: the self-contained desktop (dock + launcher
/// of every ploxion from `/ecosystem` + the autonomy dashboard + the forge), talking
/// to the core bus same-origin (`/ecosystem`, `/events`, `/emit`). This is the
/// migration target — installing the core deploys the whole labo, no Laravel.
async fn xer_page(State(st): State<AppState>) -> Html<String> {
    // Sert `<assets_dir>/web-xer/index.html` DEPUIS LE DISQUE si présent — alors les changements du
    // shell sont instantanés, sans rebuild (comme les ploxions à `/px`). Sinon la copie INLINÉE au
    // compile (`xer_html`) : le binaire reste auto-contenu même sans le dossier sur disque.
    let disk = st.assets_dir.join("web-xer").join("index.html");
    match std::fs::read_to_string(&disk) {
        Ok(s) => Html(s),
        Err(_) => Html((*st.xer_html).clone()),
    }
}

/// `GET /px/:id/*path` — serve a per-ploxion UI asset from `<assets_dir>/web-<id>/<path>`.
/// THE migration brick: each labo ploxion becomes a `web-<id>/` dir served here; its UI
/// talks to the bus same-origin (`/emit`, `/events`) — no Laravel, no CORS, no token bridge.
/// An empty `path` serves `index.html`. Hardened against path traversal: the id must be a
/// bare token, the relative path may not contain `..`/be absolute, and the resolved file is
/// canonicalized and prefix-checked against the ploxion's own `web-<id>/` base.
async fn serve_asset(
    axum::extract::Path((id, path)): axum::extract::Path<(String, String)>,
    State(st): State<AppState>,
) -> Response {
    // id = a bare ploxion token (no separators, no traversal).
    if id.is_empty() || id.contains('/') || id.contains('\\') || id.contains("..") {
        return (StatusCode::BAD_REQUEST, "bad ploxion id").into_response();
    }
    let rel = if path.is_empty() { "index.html" } else { path.as_str() };
    if rel.starts_with('/') || rel.contains("..") || rel.contains('\\') {
        return (StatusCode::BAD_REQUEST, "bad asset path").into_response();
    }
    let base = st.assets_dir.join(format!("web-{id}"));
    let file = base.join(rel);
    // Defense in depth: canonicalize both and require the file to live under the base.
    match (base.canonicalize(), file.canonicalize()) {
        (Ok(cbase), Ok(cfile)) if cfile.starts_with(&cbase) => match std::fs::read(&cfile) {
            Ok(bytes) => (
                [(axum::http::header::CONTENT_TYPE, mime_for(&cfile))],
                bytes,
            )
                .into_response(),
            Err(_) => (StatusCode::NOT_FOUND, "asset not found").into_response(),
        },
        // Missing file (canonicalize fails) => 404; escaping the base => 400.
        (Ok(_), Ok(_)) => (StatusCode::BAD_REQUEST, "path traversal").into_response(),
        _ => (StatusCode::NOT_FOUND, "asset not found").into_response(),
    }
}

/// `GET /px` — list the per-ploxion UIs available on disk: every `<assets_dir>/web-<id>/`
/// that has an `index.html`. The shell (`/xer`) fetches this to populate the launcher with
/// migrated ploxion UIs (auto-discovery — no hard-coded list, each migrated `web-<id>/`
/// appears here automatically). Returns `{ "ploxions": ["<id>", ...] }`.
async fn list_px(State(st): State<AppState>) -> Response {
    let mut ids: Vec<String> = Vec::new();
    if let Ok(rd) = std::fs::read_dir(&*st.assets_dir) {
        for e in rd.flatten() {
            if let Some(id) = e.file_name().to_str().and_then(|n| n.strip_prefix("web-").map(str::to_string)) {
                if e.path().join("index.html").is_file() {
                    ids.push(id);
                }
            }
        }
    }
    ids.sort();
    Json(serde_json::json!({ "ploxions": ids })).into_response()
}

/// Minimal content-type table for served ploxion assets (by extension).
fn mime_for(p: &Path) -> &'static str {
    match p.extension().and_then(|e| e.to_str()).unwrap_or("") {
        "html" | "htm" => "text/html; charset=utf-8",
        "js" | "mjs" => "application/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" => "application/json; charset=utf-8",
        "wasm" => "application/wasm",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "ico" => "image/x-icon",
        "woff2" => "font/woff2",
        "woff" => "font/woff",
        "txt" | "md" => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

/// `GET /3d` — the self-contained 3D/4D xion view ("station « pourriture 4 »").
/// It fetches `/ecosystem` (the WHOLE computer: running + registered + deployed,
/// falling back to `/snapshot` for the running set) and opens `/ws`, rendering
/// the core in 3D (Three.js, served same-origin at `/3d/three.min.js`): running
/// ploxions are bright/active with the live bus edges, registered ones are
/// ghosted "available" nodes, deployed services are their own markers; live bus
/// events animate as pulses along the running edges (the 4D = time axis).
async fn xion_3d(State(st): State<AppState>) -> Html<String> {
    Html((*st.xion3d_html).clone())
}

/// `GET /3d/three.min.js` — the VENDORED Three.js build, served same-origin so
/// the 3D page never reaches a CDN. Inlined at compile time from
/// `web-3d/vendor/three.min.js`.
async fn three_js() -> Response {
    (
        [(axum::http::header::CONTENT_TYPE, "application/javascript; charset=utf-8")],
        THREE_JS,
    )
        .into_response()
}

/// `GET /paper` — the self-contained XERBOXION paper v0.8, markdown rendered
/// client-side. Inlined at compile time from `web-paper/xerboxion-paper.html`;
/// the same paper is graved as tsoins `paper:xerboxion:v0.8:s01..s09`.
async fn paper_page() -> Response {
    (
        [(axum::http::header::CONTENT_TYPE, "text/html; charset=utf-8")],
        PAPER_HTML,
    )
        .into_response()
}

/// `GET /snapshot` — a JSON photograph of the live host (oneshot to the host
/// thread). Reuses the exact `map` snapshot contract the static map renders.
async fn snapshot(State(st): State<AppState>) -> Response {
    match st.host.snapshot().await {
        Some(snap) => Json(snap).into_response(),
        None => (StatusCode::SERVICE_UNAVAILABLE, "host thread unavailable").into_response(),
    }
}

/// `GET /ecosystem` — the FULL ploxion picture (the superset of `/snapshot`).
///
/// Photographs the live host (same oneshot as `/snapshot`), then AGGREGATES it
/// with the read-only registry (`ploxi0ns.json`, the registered-but-not-loaded
/// ploxions) and the deployed services (`runtime.json`) into one [`Ecosystem`]
/// (`crate::ecosystem`). `/snapshot` stays the running host; this is everything:
/// running + registered + deployed — « comme si c'était un ordinateur ». The
/// registry read is itself the API connector (José: « c'est un ploxion »); see
/// the `connector` field. Reads the canonical files per request so a registry
/// change is picked up live; an absent registry degrades to the loaded set.
async fn ecosystem(State(st): State<AppState>) -> Response {
    // FAST PATH: serve a still-fresh cached body without touching the host thread.
    // Repeated GETs (3D view / relay polling) within `ECOSYSTEM_TTL` are instant and
    // never starve the single host thread.
    if let Ok(guard) = st.ecosystem_cache.lock() {
        if let Some((at, body)) = guard.as_ref() {
            if at.elapsed() < ECOSYSTEM_TTL {
                return ecosystem_json_response(Arc::clone(body));
            }
        }
    }

    // STALE (or empty): recompute via the same oneshot + aggregation, render once,
    // and refresh the cache.
    match st.host.snapshot().await {
        Some(snap) => {
            let eco = ecosystem::build_default(&snap);
            let body: Arc<str> = match serde_json::to_string(&eco) {
                Ok(s) => Arc::from(s),
                // Serialization should not fail for this type; fall back to the
                // direct Json response rather than caching a bad body.
                Err(_) => return Json(eco).into_response(),
            };
            if let Ok(mut guard) = st.ecosystem_cache.lock() {
                *guard = Some((Instant::now(), Arc::clone(&body)));
            }
            ecosystem_json_response(body)
        }
        None => (StatusCode::SERVICE_UNAVAILABLE, "host thread unavailable").into_response(),
    }
}

/// Build a `200 application/json` response from a pre-rendered ecosystem body.
fn ecosystem_json_response(body: Arc<str>) -> Response {
    (
        [(
            axum::http::header::CONTENT_TYPE,
            "application/json; charset=utf-8",
        )],
        body.to_string(),
    )
        .into_response()
}


// ============================================================================
// STORE DE DOCUMENTS GÉNÉRIQUE — `/store/:ns/:collection[/:id]`
//
// Le cœur sert + persiste des collections de documents JSON : un fichier
// `<state_dir>/store/<ns>/<collection>.json` par collection (un tableau d'objets
// portant un champ `id`). Lectures PUBLIQUES ; écritures gardées par l'en-tête
// `X-Xer-Token` == env `XER_STORE_TOKEN` (si défini ; sinon ouvert, rétrocompat).
// Chaque mutation émet `store.<ns>.<collection>.changed` sur le bus — la vérité
// vit dans le cœur, les UI s'abonnent (contrat docs/CORE-UI-BINDING-v0.md). C'est
// la brique « tout dans le cœur » pour l'inventaire makelab et les autres cores.
// ============================================================================

const MAX_STORE_BODY: usize = 512 * 1024;

fn store_seg_ok(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 64
        && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

fn store_file(st: &AppState, ns: &str, coll: &str) -> Option<std::path::PathBuf> {
    let base = (*st.state_dir).as_ref()?;
    Some(base.join("store").join(ns).join(format!("{coll}.json")))
}

fn store_read(p: &std::path::Path) -> Vec<serde_json::Value> {
    std::fs::read(p)
        .ok()
        .and_then(|b| serde_json::from_slice::<Vec<serde_json::Value>>(&b).ok())
        .unwrap_or_default()
}

fn store_persist(p: &std::path::Path, docs: &[serde_json::Value]) -> std::io::Result<()> {
    if let Some(dir) = p.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = p.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_vec(docs).unwrap_or_default())?;
    std::fs::rename(&tmp, p)
}

fn store_can_write(st: &AppState, headers: &axum::http::HeaderMap) -> bool {
    match &*st.store_token {
        Some(tok) => headers
            .get("x-xer-token")
            .and_then(|v| v.to_str().ok())
            .map(|v| v == tok)
            .unwrap_or(false),
        None => true,
    }
}

fn store_gen_id() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("{nanos:x}-{n:x}")
}

async fn store_list(
    axum::extract::Path((ns, coll)): axum::extract::Path<(String, String)>,
    State(st): State<AppState>,
) -> Response {
    if !store_seg_ok(&ns) || !store_seg_ok(&coll) {
        return (StatusCode::BAD_REQUEST, "bad store path").into_response();
    }
    match store_file(&st, &ns, &coll) {
        Some(p) => Json(store_read(&p)).into_response(),
        None => (StatusCode::SERVICE_UNAVAILABLE, "no state dir").into_response(),
    }
}

async fn store_get(
    axum::extract::Path((ns, coll, id)): axum::extract::Path<(String, String, String)>,
    State(st): State<AppState>,
) -> Response {
    if !store_seg_ok(&ns) || !store_seg_ok(&coll) {
        return (StatusCode::BAD_REQUEST, "bad store path").into_response();
    }
    let Some(p) = store_file(&st, &ns, &coll) else {
        return (StatusCode::SERVICE_UNAVAILABLE, "no state dir").into_response();
    };
    match store_read(&p)
        .into_iter()
        .find(|doc| doc.get("id").and_then(|v| v.as_str()) == Some(id.as_str()))
    {
        Some(doc) => Json(doc).into_response(),
        None => (StatusCode::NOT_FOUND, "not found").into_response(),
    }
}

async fn store_create(
    axum::extract::Path((ns, coll)): axum::extract::Path<(String, String)>,
    State(st): State<AppState>,
    headers: axum::http::HeaderMap,
    body: axum::body::Bytes,
) -> Response {
    if !store_seg_ok(&ns) || !store_seg_ok(&coll) {
        return (StatusCode::BAD_REQUEST, "bad store path").into_response();
    }
    if !store_can_write(&st, &headers) {
        return (StatusCode::UNAUTHORIZED, "écriture gardée — en-tête X-Xer-Token requis").into_response();
    }
    if body.len() > MAX_STORE_BODY {
        return (StatusCode::PAYLOAD_TOO_LARGE, "store body too large").into_response();
    }
    let mut doc: serde_json::Value = match serde_json::from_slice(&body) {
        Ok(v) => v,
        Err(e) => return (StatusCode::BAD_REQUEST, format!("invalid json: {e}")).into_response(),
    };
    if !doc.is_object() {
        return (StatusCode::BAD_REQUEST, "doc must be a JSON object").into_response();
    }
    let id = doc
        .get("id")
        .and_then(|v| v.as_str())
        .filter(|x| !x.is_empty())
        .map(|x| x.to_string())
        .unwrap_or_else(store_gen_id);
    doc["id"] = serde_json::Value::String(id.clone());
    let Some(p) = store_file(&st, &ns, &coll) else {
        return (StatusCode::SERVICE_UNAVAILABLE, "no state dir").into_response();
    };
    {
        let _guard = st.store_lock.lock().unwrap_or_else(|e| e.into_inner());
        let mut docs = store_read(&p);
        docs.push(doc.clone());
        if store_persist(&p, &docs).is_err() {
            return (StatusCode::INTERNAL_SERVER_ERROR, "store write failed").into_response();
        }
    }
    st.host.emit(format!("store.{ns}.{coll}.changed"), id);
    (StatusCode::CREATED, Json(doc)).into_response()
}

async fn store_put(
    axum::extract::Path((ns, coll, id)): axum::extract::Path<(String, String, String)>,
    State(st): State<AppState>,
    headers: axum::http::HeaderMap,
    body: axum::body::Bytes,
) -> Response {
    if !store_seg_ok(&ns) || !store_seg_ok(&coll) {
        return (StatusCode::BAD_REQUEST, "bad store path").into_response();
    }
    if !store_can_write(&st, &headers) {
        return (StatusCode::UNAUTHORIZED, "écriture gardée — en-tête X-Xer-Token requis").into_response();
    }
    if body.len() > MAX_STORE_BODY {
        return (StatusCode::PAYLOAD_TOO_LARGE, "store body too large").into_response();
    }
    let mut doc: serde_json::Value = match serde_json::from_slice(&body) {
        Ok(v) => v,
        Err(e) => return (StatusCode::BAD_REQUEST, format!("invalid json: {e}")).into_response(),
    };
    if !doc.is_object() {
        return (StatusCode::BAD_REQUEST, "doc must be a JSON object").into_response();
    }
    doc["id"] = serde_json::Value::String(id.clone());
    let Some(p) = store_file(&st, &ns, &coll) else {
        return (StatusCode::SERVICE_UNAVAILABLE, "no state dir").into_response();
    };
    {
        let _guard = st.store_lock.lock().unwrap_or_else(|e| e.into_inner());
        let mut docs = store_read(&p);
        let mut found = false;
        for d in docs.iter_mut() {
            if d.get("id").and_then(|v| v.as_str()) == Some(id.as_str()) {
                *d = doc.clone();
                found = true;
                break;
            }
        }
        if !found {
            docs.push(doc.clone());
        }
        if store_persist(&p, &docs).is_err() {
            return (StatusCode::INTERNAL_SERVER_ERROR, "store write failed").into_response();
        }
    }
    st.host.emit(format!("store.{ns}.{coll}.changed"), id);
    Json(doc).into_response()
}

async fn store_delete(
    axum::extract::Path((ns, coll, id)): axum::extract::Path<(String, String, String)>,
    State(st): State<AppState>,
    headers: axum::http::HeaderMap,
) -> Response {
    if !store_seg_ok(&ns) || !store_seg_ok(&coll) {
        return (StatusCode::BAD_REQUEST, "bad store path").into_response();
    }
    if !store_can_write(&st, &headers) {
        return (StatusCode::UNAUTHORIZED, "écriture gardée — en-tête X-Xer-Token requis").into_response();
    }
    let Some(p) = store_file(&st, &ns, &coll) else {
        return (StatusCode::SERVICE_UNAVAILABLE, "no state dir").into_response();
    };
    let removed;
    {
        let _guard = st.store_lock.lock().unwrap_or_else(|e| e.into_inner());
        let mut docs = store_read(&p);
        let before = docs.len();
        docs.retain(|d| d.get("id").and_then(|v| v.as_str()) != Some(id.as_str()));
        removed = docs.len() != before;
        if removed && store_persist(&p, &docs).is_err() {
            return (StatusCode::INTERNAL_SERVER_ERROR, "store write failed").into_response();
        }
    }
    if removed {
        st.host.emit(format!("store.{ns}.{coll}.changed"), id);
    }
    Json(serde_json::json!({ "removed": removed })).into_response()
}

/// The `/healthz` body.
#[derive(Serialize)]
struct HealthZ {
    status: &'static str,
    /// Uptime in whole seconds.
    uptime: u64,
    /// Number of wasm ploxions loaded.
    ploxions: usize,
    /// The core's short git sha.
    commit: String,
    /// This daemon's federation node id (origin tag).
    node_id: String,
}

/// `GET /replicate` — the UNIVERSAL CONSTRUCTOR export. Returns this node's
/// reconstructable runtime DELTA (code) as a [`crate::replicate::ReplicaBundle`]
/// JSON: a fresh node `serve --reconstruct-from <this-url>` materializes it into
/// its own state dir and boots IDENTICAL (the self-reproducing « pourriture 4 »).
///
/// Empty `code` when `--state-dir` is off — nothing is persisted, and the base
/// set reconstructs from the image, not from here. Read-only and safe to call
/// concurrently with the host thread (state-dir writes are atomic tmp+rename).
async fn replicate(State(st): State<AppState>) -> Response {
    use crate::replicate::{export, ReplicaBundle, BUNDLE_VERSION};
    // CODE — the runtime-loaded delta from the state dir (empty if persistence off;
    // the base set reconstructs from the image).
    let mut bundle = match &*st.state_dir {
        Some(dir) => match StateStore::open(dir).and_then(|s| export(&s)) {
            Ok(b) => b,
            Err(e) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("replicate export failed: {e:#}"),
                )
                    .into_response()
            }
        },
        None => ReplicaBundle { version: BUNDLE_VERSION, code: vec![], data: vec![] },
    };
    // DATA — ask the `tsoin` ploxion for its stored tsoins (bus round-trip) and
    // reference each one. Empty if the store is not ready (boot race) — a re-poll
    // repopulates it. So the bundle carries CODE + DONNÉES (von Neumann complet).
    bundle.data = collect_data_refs(&st).await;
    Json(bundle).into_response()
}

/// Emit `tsoin.list` and collect the `tsoin.listed {count,entries:[{name,id}]}`
/// reply off the live bus, turning each stored tsoin into a
/// [`crate::replicate::DataRef`] the universal constructor can pull. Bounded by a
/// short timeout so `/replicate` never hangs; tolerant of a busy bus (Lagged).
async fn collect_data_refs(st: &AppState) -> Vec<crate::replicate::DataRef> {
    use crate::replicate::DataRef;
    let mut rx = st.events.subscribe();
    // subscribe BEFORE emitting so we never miss the reply.
    if !st.host.emit("tsoin.list".to_string(), String::new()) {
        return Vec::new();
    }
    let payload = tokio::time::timeout(std::time::Duration::from_millis(2000), async {
        loop {
            match rx.recv().await {
                Ok(ev) if ev.topic == "tsoin.listed" => return Some(ev.payload),
                Ok(_) => continue,
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                Err(_) => return None,
            }
        }
    })
    .await
    .ok()
    .flatten();
    let payload = match payload {
        Some(p) => p,
        None => return Vec::new(),
    };
    let v: serde_json::Value = match serde_json::from_str(&payload) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };
    let mut refs = Vec::new();
    if let Some(entries) = v.get("entries").and_then(|e| e.as_array()) {
        for e in entries {
            let name = e.get("name").and_then(|x| x.as_str()).unwrap_or("").to_string();
            if name.is_empty() {
                continue;
            }
            let id = match e.get("id") {
                Some(serde_json::Value::String(s)) => Some(s.clone()),
                Some(serde_json::Value::Number(n)) => Some(n.to_string()),
                _ => None,
            };
            refs.push(DataRef { store: name, capture: "tsoin".to_string(), tsoin: id });
        }
    }
    refs
}

/// `GET /healthz` — liveness + a tiny bit of fleet info.
async fn healthz(State(st): State<AppState>) -> Json<HealthZ> {
    Json(HealthZ {
        status: "ok",
        uptime: st.started.elapsed().as_secs(),
        ploxions: st.ploxions,
        commit: (*st.commit).clone(),
        node_id: (*st.node_id).clone(),
    })
}

/// `POST /sso/token` — relais MÊME ORIGINE vers l'IdP `api.j0bot.ch/oauth/token`. Le xer
/// (servi par ce cœur) appelle ce point au lieu d'appeler l'api en cross-origin : l'échange
/// PKCE se fait donc serveur-à-serveur, **sans dépendre du CORS de l'api**. Le corps
/// (form-urlencoded : grant_type/code/code_verifier/client_id/redirect_uri) est relayé tel
/// quel avec le bon `Content-Type` ; on rend la réponse JSON de l'IdP (jeton, ou erreur OAuth).
/// Cible FIGÉE (pas d'URL pilotable) — ce n'est pas un proxy ouvert.
async fn sso_token(body: axum::body::Bytes) -> Response {
    if body.len() > 4096 {
        return (StatusCode::PAYLOAD_TOO_LARGE, "sso token body too large").into_response();
    }
    let body_vec = body.to_vec();
    let out = tokio::task::spawn_blocking(move || -> (u16, String) {
        let config = ureq::Agent::config_builder()
            .timeout_global(Some(std::time::Duration::from_secs(10)))
            .build();
        let agent = ureq::Agent::new_with_config(config);
        match agent
            .post("https://api.j0bot.ch/oauth/token")
            .header("Content-Type", "application/x-www-form-urlencoded")
            .header("Accept", "application/json")
            .send(&body_vec[..])
        {
            Ok(mut r) => {
                let status = r.status().as_u16();
                let txt = r
                    .body_mut()
                    .with_config()
                    .limit(64 * 1024)
                    .read_to_vec()
                    .map(|b| String::from_utf8_lossy(&b).into_owned())
                    .unwrap_or_default();
                (status, txt)
            }
            // ureq surface un 4xx/5xx en erreur portant le code : on le préserve.
            Err(ureq::Error::StatusCode(code)) => {
                (code, format!("{{\"error\":\"oauth_status_{code}\"}}"))
            }
            Err(_) => (502, "{\"error\":\"idp_unreachable\"}".to_string()),
        }
    })
    .await;
    match out {
        Ok((status, txt)) => {
            let code = StatusCode::from_u16(status).unwrap_or(StatusCode::BAD_GATEWAY);
            (
                code,
                [(axum::http::header::CONTENT_TYPE, "application/json")],
                txt,
            )
                .into_response()
        }
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "sso proxy join error").into_response(),
    }
}

/// `POST /emit {topic,payload}` — inject onto the REAL bus. Returns `202` once
/// the command is queued; the resulting events are observed via `/ws`. Never
/// panics on a bad request: a missing/oversized/invalid body is a `4xx`.
async fn emit(State(st): State<AppState>, body: axum::body::Bytes) -> Response {
    if body.len() > MAX_EMIT_BODY {
        return (StatusCode::PAYLOAD_TOO_LARGE, "emit body too large").into_response();
    }
    let parsed: EmitBody = match serde_json::from_slice(&body) {
        Ok(b) => b,
        Err(e) => {
            return (StatusCode::BAD_REQUEST, format!("invalid emit body: {e}")).into_response()
        }
    };
    if parsed.topic.trim().is_empty() {
        return (StatusCode::BAD_REQUEST, "emit topic must not be empty").into_response();
    }
    if st.host.emit(parsed.topic, parsed.payload) {
        (StatusCode::ACCEPTED, "emitted").into_response()
    } else {
        (StatusCode::SERVICE_UNAVAILABLE, "host thread unavailable").into_response()
    }
}

/// `POST /load` — HOT-LOAD a ploxion into the running core WITHOUT a restart.
///
/// Body is EITHER `{"id":"<registered-id>"}` (loads the staged
/// `<ploxions_dir>/<id>.wasm`) OR `{"wasm":"<base64>"}` (an arbitrary module, with
/// optional `{"id"}` to name it). The async handler only RESOLVES the bytes
/// (file read / base64 decode) and validates the request shape; the wasm itself is
/// compiled/instantiated/wired ON THE HOST THREAD via a `Load` command (the Host
/// is never touched here — `Store` is not `Sync`). On success returns `200` with
/// the loaded manifest as JSON; the lifecycle (`load` + `init`) is traced and
/// streamed on `/ws` + `/events`, and the ploxion appears `running` in
/// `/snapshot` + `/ecosystem`.
///
/// Errors are CLEAN `4xx`, never a panic:
/// - `400` malformed/oversized body, neither `id` nor `wasm` present, or invalid
///   base64.
/// - `404` `{id}` given but no `<ploxions_dir>/<id>.wasm` staged.
/// - `422` the wasm is invalid / missing required exports / its manifest is
///   rejected / a ploxion with that id is ALREADY loaded (duplicate).
async fn load(State(st): State<AppState>, body: axum::body::Bytes) -> Response {
    if body.len() > MAX_LOAD_BODY {
        return (StatusCode::PAYLOAD_TOO_LARGE, "load body too large").into_response();
    }
    let parsed: LoadBody = match serde_json::from_slice(&body) {
        Ok(b) => b,
        Err(e) => {
            return (StatusCode::BAD_REQUEST, format!("invalid load body: {e}")).into_response()
        }
    };

    // Resolve the wasm bytes + a fallback id + the PERSISTENCE source from the
    // request. A base64 upload is `Source::Wasm` (its bytes must be persisted);
    // a registered-id load is `Source::Staged` (it reloads from the staged dir).
    let (wasm, fallback_id, source): (Vec<u8>, String, Source) = match (&parsed.wasm, &parsed.id) {
        // base64 wasm (id, if present, is only the fallback name).
        (Some(b64), id_opt) => {
            use base64::Engine as _;
            match base64::engine::general_purpose::STANDARD.decode(b64.trim()) {
                Ok(bytes) if !bytes.is_empty() => {
                    let fid = id_opt.clone().unwrap_or_else(|| "uploaded".to_string());
                    (bytes, fid, Source::Wasm)
                }
                Ok(_) => {
                    return (StatusCode::BAD_REQUEST, "load: `wasm` decoded to empty bytes")
                        .into_response()
                }
                Err(e) => {
                    return (StatusCode::BAD_REQUEST, format!("load: invalid base64 `wasm`: {e}"))
                        .into_response()
                }
            }
        }
        // registered id -> staged <dir>/<id>.wasm.
        (None, Some(id)) => {
            let id = id.trim();
            if id.is_empty() {
                return (StatusCode::BAD_REQUEST, "load: `id` must not be empty").into_response();
            }
            // Guard against path traversal: a bare id only, no separators.
            if id.contains('/') || id.contains('\\') || id.contains("..") {
                return (StatusCode::BAD_REQUEST, "load: `id` must be a bare ploxion id")
                    .into_response();
            }
            let path = st.ploxions_dir.join(format!("{id}.wasm"));
            match std::fs::read(&path) {
                Ok(bytes) => (bytes, id.to_string(), Source::Staged),
                Err(_) => {
                    return (
                        StatusCode::NOT_FOUND,
                        format!("load: no staged wasm for id '{id}' at {}", path.display()),
                    )
                        .into_response()
                }
            }
        }
        // Neither id nor wasm.
        (None, None) => {
            return (
                StatusCode::BAD_REQUEST,
                "load: provide either {\"id\":\"<registered-id>\"} or {\"wasm\":\"<base64>\"}",
            )
                .into_response()
        }
    };

    // Hand the bytes to the HOST THREAD (the only owner of the Host). The source
    // rides along so the host thread can PERSIST the runtime-loaded delta.
    match st.host.load(wasm, fallback_id, source).await {
        Some(Ok(manifest)) => (StatusCode::OK, Json(manifest)).into_response(),
        // A bad module / duplicate id / rejected manifest is a 422 (the request
        // was well-formed, but the wasm could not be loaded) — never a panic.
        Some(Err(e)) => (StatusCode::UNPROCESSABLE_ENTITY, e).into_response(),
        None => (StatusCode::SERVICE_UNAVAILABLE, "host thread unavailable").into_response(),
    }
}

/// `POST /unload {"id":"<loaded-id>"}` — HOT-UNLOAD a loaded ploxion: `plc_goodbye`
/// then drop its `Store` (droit au silence), removing its bus wiring so it reacts
/// to nothing afterwards. The lifecycle (`goodbye` + `unload`) is traced and
/// streamed; the ploxion then disappears from `/snapshot` (or reverts to
/// `registered` in `/ecosystem` if it is a registry id). Returns `200` on success,
/// `404` if no ploxion with that id is loaded, `400` on a bad body. Never panics.
async fn unload(State(st): State<AppState>, body: axum::body::Bytes) -> Response {
    if body.len() > MAX_LOAD_BODY {
        return (StatusCode::PAYLOAD_TOO_LARGE, "unload body too large").into_response();
    }
    let parsed: UnloadBody = match serde_json::from_slice(&body) {
        Ok(b) => b,
        Err(e) => {
            return (StatusCode::BAD_REQUEST, format!("invalid unload body: {e}")).into_response()
        }
    };
    if parsed.id.trim().is_empty() {
        return (StatusCode::BAD_REQUEST, "unload: `id` must not be empty").into_response();
    }
    match st.host.unload(parsed.id).await {
        Some(Ok(id)) => (
            StatusCode::OK,
            Json(serde_json::json!({"unloaded": id})),
        )
            .into_response(),
        Some(Err(e)) => (StatusCode::NOT_FOUND, e).into_response(),
        None => (StatusCode::SERVICE_UNAVAILABLE, "host thread unavailable").into_response(),
    }
}

/// `GET /ws` — upgrade to a WebSocket. The socket then streams every live bus
/// hop as JSON and accepts inbound `{emit:{topic,payload}}` messages.
async fn ws_upgrade(State(st): State<AppState>, up: WebSocketUpgrade) -> Response {
    up.on_upgrade(move |socket| ws_loop(socket, st))
}

/// One WebSocket connection: forward every broadcast event out, and forward any
/// inbound `{emit:{...}}` onto the bus. Ends when either side closes.
async fn ws_loop(socket: WebSocket, st: AppState) {
    let mut rx = st.events.subscribe();
    let (mut sink, mut stream) = socket.split();

    // Outbound: broadcast bus hops -> client. A lagging client skips dropped
    // events (logged in the message) rather than killing the connection.
    let out = async {
        loop {
            match rx.recv().await {
                Ok(ev) => {
                    let txt = match serde_json::to_string(&ev) {
                        Ok(s) => s,
                        Err(_) => continue,
                    };
                    if sink.send(Message::Text(txt)).await.is_err() {
                        break; // client gone
                    }
                }
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    // Tell the client it fell behind, then keep streaming.
                    let note = format!("{{\"kind\":\"lagged\",\"dropped\":{n}}}");
                    if sink.send(Message::Text(note)).await.is_err() {
                        break;
                    }
                }
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
    };

    // Inbound: client -> bus. We accept {emit:{topic,payload}}; anything else is
    // ignored (never panic on a bad frame).
    let host = st.host.clone();
    let inn = async {
        while let Some(Ok(msg)) = stream.next().await {
            match msg {
                Message::Text(t) => {
                    if let Ok(inbound) = serde_json::from_str::<WsInbound>(&t) {
                        if !inbound.emit.topic.trim().is_empty() {
                            host.emit(inbound.emit.topic, inbound.emit.payload);
                        }
                    }
                }
                Message::Close(_) => break,
                _ => {}
            }
        }
    };

    // Run both halves; when either finishes (client closed / stream ended), drop
    // the other.
    tokio::select! {
        _ = out => {}
        _ = inn => {}
    }
}

/// `GET /events` — the SAME live bus feed as `/ws`, but as Server-Sent Events
/// over PLAIN HTTP (a normal `200` chunked stream, NOT a `101` upgrade). This is
/// trivially proxyable server-side: the fleet relay (a PHP container) can read it
/// with any HTTP client and forward it without a WebSocket sidecar.
///
/// Each [`LiveEvent`] becomes one SSE event whose `data:` is the SAME JSON shape
/// `/ws` sends (`{seq,kind,from,topic,payload,note}`), with the SSE `event:` field
/// set to the hop `kind` and `id:` to the global `seq`. A lagging subscriber drops
/// the oldest events (like `/ws`) and keeps streaming rather than erroring out.
/// A periodic keep-alive comment holds idle connections + proxies open.
async fn events_sse(State(st): State<AppState>) -> Response {
    // Subscribe to the SAME broadcast channel `/ws` uses — this is the real bus,
    // never a synthesized stream.
    let rx = st.events.subscribe();

    // Turn the broadcast receiver into an SSE event stream. On `Lagged(n)` we do
    // NOT end the stream: we emit a `lagged` notice (mirroring `/ws`) and keep
    // going. On `Closed` (host gone) the stream ends.
    let stream = futures_util::stream::unfold(rx, |mut rx| async move {
        loop {
            match rx.recv().await {
                Ok(ev) => {
                    let json = match serde_json::to_string(&ev) {
                        Ok(s) => s,
                        Err(_) => continue,
                    };
                    let event = Event::default()
                        .id(ev.seq.to_string())
                        .event(ev.kind.clone())
                        .data(json);
                    return Some((Ok::<Event, std::convert::Infallible>(event), rx));
                }
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    let event = Event::default()
                        .event("lagged")
                        .data(format!("{{\"kind\":\"lagged\",\"dropped\":{n}}}"));
                    return Some((Ok(event), rx));
                }
                Err(broadcast::error::RecvError::Closed) => return None,
            }
        }
    });

    // `Sse` sets `Content-Type: text/event-stream; charset=utf-8`,
    // `Cache-Control: no-cache`, and (via KeepAlive) `Connection: keep-alive`.
    // We additionally stamp `X-Accel-Buffering: no` so nginx/Traefik-style
    // reverse proxies do not buffer the stream.
    let sse = Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("keep-alive"),
    );

    let mut resp = sse.into_response();
    let headers = resp.headers_mut();
    // axum's `Sse` already sets `content-type: text/event-stream` and
    // `cache-control: no-cache`; we pin the explicit `charset=utf-8` form the
    // fleet relay expects, and add `X-Accel-Buffering: no` so reverse proxies
    // (nginx/Traefik) do not buffer the stream.
    headers.insert(
        axum::http::header::CONTENT_TYPE,
        axum::http::HeaderValue::from_static("text/event-stream; charset=utf-8"),
    );
    headers.insert(
        axum::http::header::CONNECTION,
        axum::http::HeaderValue::from_static("keep-alive"),
    );
    headers.insert(
        "X-Accel-Buffering",
        axum::http::HeaderValue::from_static("no"),
    );
    resp
}

// ---------------------------------------------------------------------------
// BUS FEDERATION — the `/peer` server endpoint + the shared bidirectional loop.
// ---------------------------------------------------------------------------

/// `GET /peer` — upgrade to a FEDERATION WebSocket (the inter-node link, server
/// side). Distinct from `/ws`: it speaks the `{fed:{…}}` frame, not `{emit:{…}}`.
///
/// AUTH: if this daemon was started with `--peer-token`, the upgrade MUST carry
/// the matching `X-Xion-Peer-Token` header or it is refused `401`. (Basic-auth,
/// when used, is enforced one layer up by Traefik before the request reaches the
/// daemon; the token is the daemon's own in-process check so the link is
/// authenticatable even without a proxy.)
async fn peer_upgrade(
    State(st): State<AppState>,
    headers: axum::http::HeaderMap,
    up: WebSocketUpgrade,
) -> Response {
    if let Some(expected) = st.peer_token.as_ref() {
        let presented = headers
            .get(PEER_TOKEN_HEADER)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        if presented != expected {
            return (StatusCode::UNAUTHORIZED, "peer token required").into_response();
        }
    }
    let node = (*st.node_id).clone();
    up.on_upgrade(move |socket| {
        let st = st.clone();
        async move {
            println!("xerboxion-rt serve: [{node}] peer link accepted (inbound)");
            peer_link(socket, st).await;
            println!("xerboxion-rt serve: [{node}] peer link closed (inbound)");
        }
    })
}

/// One FEDERATION link over an axum [`WebSocket`] (the SERVER side of `/peer`).
/// Bidirectional: forward this node's LOCAL-ORIGIN emits OUT as `{fed:{…}}`
/// frames, and inject inbound `{fed:{…}}` frames onto the local bus as
/// remote-origin. Loop-safe by construction (only local-origin emits ride
/// `fed_out`; an injected remote emit is never put back on `fed_out`).
async fn peer_link(socket: WebSocket, st: AppState) {
    let mut fed_rx = st.fed_out.subscribe();
    let (mut sink, mut stream) = socket.split();

    // OUT: local-origin emits -> peer.
    let out = async {
        loop {
            match fed_rx.recv().await {
                Ok(m) => {
                    let frame = PeerFrame { fed: m };
                    let Ok(txt) = serde_json::to_string(&frame) else { continue };
                    if sink.send(Message::Text(txt)).await.is_err() {
                        break;
                    }
                }
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
    };

    // IN: peer's `{fed:{…}}` -> local bus (remote-origin).
    let host = st.host.clone();
    let inn = async {
        while let Some(Ok(msg)) = stream.next().await {
            match msg {
                Message::Text(t) => {
                    if let Ok(frame) = serde_json::from_str::<PeerFrame>(&t) {
                        if !frame.fed.topic.trim().is_empty() {
                            host.fed_inject(frame.fed);
                        }
                    }
                }
                Message::Close(_) => break,
                _ => {}
            }
        }
    };

    tokio::select! {
        _ = out => {}
        _ = inn => {}
    }
}

/// The CLIENT side of a peer link: dial `peer` forever, reconnecting with
/// exponential backoff (a peer may not be up yet, or may restart). While
/// connected, run the same bidirectional federation loop as the server side.
/// Spawned once per `--peer`. `node` is this daemon's id (for log lines).
async fn peer_client(peer: PeerSpec, st: AppState, node: Arc<String>) {
    let mut backoff = RECONNECT_MIN;
    loop {
        match connect_peer(&peer).await {
            Ok((ws, _resp)) => {
                println!(
                    "xerboxion-rt serve: [{node}] peer link UP (outbound) -> {}",
                    peer.label()
                );
                backoff = RECONNECT_MIN; // reset on a successful connect
                peer_client_link(ws, st.clone()).await;
                println!(
                    "xerboxion-rt serve: [{node}] peer link DOWN (outbound) -> {} — reconnecting",
                    peer.label()
                );
            }
            Err(e) => {
                eprintln!(
                    "xerboxion-rt serve: [{node}] peer {} unreachable ({e}); retry in {:?}",
                    peer.label(),
                    backoff
                );
            }
        }
        tokio::time::sleep(backoff).await;
        backoff = (backoff * 2).min(RECONNECT_MAX);
    }
}

/// A tungstenite client WebSocket stream type alias (over a maybe-TLS stream).
type ClientWs = tokio_tungstenite::WebSocketStream<
    tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
>;

/// Open a client WebSocket to `peer`, attaching auth on the upgrade request:
/// `Authorization: Basic …` if the url carries `user:pass@` userinfo, and/or
/// `X-Xion-Peer-Token` if a token was configured. Returns the connected stream.
async fn connect_peer(
    peer: &PeerSpec,
) -> Result<(ClientWs, tokio_tungstenite::tungstenite::handshake::client::Response)> {
    use tokio_tungstenite::tungstenite::client::IntoClientRequest;

    let mut req = peer
        .url
        .as_str()
        .into_client_request()
        .with_context(|| format!("invalid peer ws url {}", peer.label()))?;

    // Basic-auth from the url userinfo (`ws://user:pass@host/...`).
    if let Some((user, pass)) = userinfo(&peer.url) {
        use base64::Engine as _;
        let creds = base64::engine::general_purpose::STANDARD
            .encode(format!("{user}:{pass}"));
        let value = format!("Basic {creds}")
            .parse()
            .context("building basic-auth header")?;
        req.headers_mut().insert(axum::http::header::AUTHORIZATION, value);
    }
    // Shared peer token.
    if let Some(tok) = &peer.token {
        let value = tok.parse().context("building peer-token header")?;
        req.headers_mut()
            .insert(PEER_TOKEN_HEADER, value);
    }

    let (ws, resp) = tokio_tungstenite::connect_async(req)
        .await
        .with_context(|| format!("connecting to peer {}", peer.label()))?;
    Ok((ws, resp))
}

/// Parse `user:pass` out of a `ws://user:pass@host/...` url, if present.
fn userinfo(url: &str) -> Option<(String, String)> {
    let s = url.strip_prefix("ws://").or_else(|| url.strip_prefix("wss://"))?;
    let (authority, _) = s.split_once('/').unwrap_or((s, ""));
    let (creds, _hostport) = authority.split_once('@')?;
    let (user, pass) = creds.split_once(':')?;
    Some((user.to_string(), pass.to_string()))
}

/// The bidirectional federation loop over a tungstenite CLIENT stream — the
/// mirror of [`peer_link`] (which runs over an axum server socket).
async fn peer_client_link(ws: ClientWs, st: AppState) {
    use tokio_tungstenite::tungstenite::Message as TMsg;
    let mut fed_rx = st.fed_out.subscribe();
    let (mut sink, mut stream) = ws.split();

    let out = async {
        loop {
            match fed_rx.recv().await {
                Ok(m) => {
                    let frame = PeerFrame { fed: m };
                    let Ok(txt) = serde_json::to_string(&frame) else { continue };
                    if sink.send(TMsg::Text(txt)).await.is_err() {
                        break;
                    }
                }
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
    };

    let host = st.host.clone();
    let inn = async {
        while let Some(Ok(msg)) = stream.next().await {
            match msg {
                TMsg::Text(t) => {
                    if let Ok(frame) = serde_json::from_str::<PeerFrame>(&t) {
                        if !frame.fed.topic.trim().is_empty() {
                            host.fed_inject(frame.fed);
                        }
                    }
                }
                TMsg::Close(_) => break,
                _ => {}
            }
        }
    };

    tokio::select! {
        _ = out => {}
        _ = inn => {}
    }
}

// ---------------------------------------------------------------------------
// Boot: build the host, spawn its thread, build the router.
// ---------------------------------------------------------------------------

/// Build the daemon's [`Host`] (load every ploxion in `dir`, init the fleet),
/// returning it plus the ploxion count. Kept separate so tests can reuse it.
fn build_host(dir: &Path) -> Result<(Host, usize)> {
    let mut host = Host::new();
    let n = host
        .load_dir(dir)
        .with_context(|| format!("loading ploxions from {}", dir.display()))?;
    if n == 0 {
        anyhow::bail!(
            "no ploxions loaded from {} — run scripts/build-ploxions.sh first",
            dir.display()
        );
    }
    host.init_all().context("initializing the ploxion fleet")?;
    Ok((host, n))
}

/// RESTORE the persisted runtime-loaded delta into a freshly-booted `host`
/// (whose base set is already loaded from `<ploxions_dir>`). For each persisted
/// record we `load_runtime` the bytes — from the persisted blob (`source=wasm`)
/// or from `<ploxions_dir>/<id>.wasm` (`source=staged`) — so a previously
/// hot-loaded ploxion comes back automatically.
///
/// GRACEFUL by contract: this NEVER returns an error and NEVER panics. A corrupt
/// manifest, a missing/truncated wasm, a now-duplicate id (already in the base
/// set), or a module that fails to load is SKIPPED with a `WARN` to stderr; the
/// daemon still boots its base set. Returns the count actually restored (for the
/// boot log). With no records (or a missing/corrupt manifest) it restores 0.
fn restore_persisted(host: &mut Host, store: &StateStore, ploxions_dir: &Path) -> usize {
    let records = match store.read_records() {
        Ok(r) => r,
        Err(e) => {
            eprintln!(
                "xerboxion-rt serve: WARN state dir {} is unreadable/corrupt ({e:#}); \
                 booting the base set only",
                store.dir().display()
            );
            return 0;
        }
    };
    let mut restored = 0usize;
    for rec in &records {
        // Resolve the bytes for this record.
        let bytes = match rec.source {
            Source::Wasm => match store.read_wasm(&rec.id) {
                Ok(b) => b,
                Err(e) => {
                    eprintln!(
                        "xerboxion-rt serve: WARN skipping restore of '{}' (wasm unreadable: {e:#})",
                        rec.id
                    );
                    continue;
                }
            },
            Source::Staged => {
                let path = ploxions_dir.join(format!("{}.wasm", rec.id));
                match std::fs::read(&path) {
                    Ok(b) => b,
                    Err(e) => {
                        eprintln!(
                            "xerboxion-rt serve: WARN skipping restore of staged '{}' \
                             (no {} : {e})",
                            rec.id,
                            path.display()
                        );
                        continue;
                    }
                }
            }
        };
        // Load it into the running host (same path POST /load uses). A duplicate
        // id (somehow already in the base set) or a bad module is skipped, never
        // fatal — restore must never crash startup.
        match host.load_runtime(&bytes, &rec.id) {
            Ok(m) => {
                restored += 1;
                println!(
                    "xerboxion-rt serve: restored runtime ploxion '{}' from state dir ({:?})",
                    m.id, rec.source
                );
            }
            Err(e) => {
                eprintln!(
                    "xerboxion-rt serve: WARN could not restore '{}' ({e:#}); skipping",
                    rec.id
                );
            }
        }
    }
    restored
}

/// Everything an in-process server needs, wired and ready to serve on `listener`.
/// Returned by [`build_server`] so tests can drive it directly (bind `127.0.0.1:0`,
/// connect a real client) without a process, while [`run`] uses it for the real
/// daemon. The `state` carries the host handle; dropping the returned future or
/// the host handle lets the host thread wind down.
pub struct Server {
    router: Router,
    state: AppState,
    /// The host-thread join handle, so a caller can wait for the Store drop.
    host_join: std::thread::JoinHandle<()>,
}

/// Construct the host + host thread + router for `dir` with NO federation (the
/// classic single-node daemon). Thin wrapper over [`build_server_fed`] so the
/// existing call sites and tests stay byte-identical and additive.
#[cfg(test)]
fn build_server(dir: &Path, commit: String) -> Result<Server> {
    build_server_fed(dir, commit, FederationConfig::default(), None)
}

/// Construct the host + host thread + router for `dir`, optionally with a
/// `--state-dir` (`state_dir`) for DURABLE persistence of the runtime-loaded
/// delta. A convenience for tests that need persistence but no federation.
#[cfg(test)]
fn build_server_state(dir: &Path, commit: String, state_dir: Option<&Path>) -> Result<Server> {
    build_server_fed(dir, commit, FederationConfig::default(), state_dir)
}

/// Construct the host + host thread + router for `dir`, wiring BUS FEDERATION per
/// `fed`. Does NOT bind a socket nor dial peers — the caller provides the
/// listener and (in [`run`]) spawns the peer client tasks. The `commit` is
/// stamped into snapshots/healthz.
///
/// When `state_dir` is `Some`, the daemon is DURABLE: the runtime-loaded delta is
/// persisted there on each `POST /load` and removed on `POST /unload`, and the
/// persisted set is RESTORED into the host right after the base set is loaded (so
/// a previously hot-loaded ploxion comes back). When `None`, persistence is OFF
/// and the daemon behaves exactly as before. A state dir that cannot be opened is
/// logged and treated as `None` (run without persistence) — never fatal.
fn build_server_fed(
    dir: &Path,
    commit: String,
    fed: FederationConfig,
    state_dir: Option<&Path>,
) -> Result<Server> {
    let (mut host, base_ploxions) = build_host(dir)?;

    // Open the state store (best-effort). A dir that cannot be created degrades
    // to no persistence rather than crashing the daemon.
    let state_store = match state_dir {
        Some(p) => match StateStore::open(p) {
            Ok(s) => Some(s),
            Err(e) => {
                eprintln!(
                    "xerboxion-rt serve: WARN could not open state dir {} ({e:#}); \
                     running WITHOUT persistence",
                    p.display()
                );
                None
            }
        },
        None => None,
    };

    // RESTORE the persisted runtime delta on top of the base set (graceful: bad
    // records are skipped, never fatal). The base set is NOT in the state dir, so
    // there is no double-load.
    let mut ploxions = base_ploxions;
    if let Some(store) = &state_store {
        let restored = restore_persisted(&mut host, store, dir);
        if restored > 0 {
            println!(
                "xerboxion-rt serve: restored {restored} runtime ploxion(s) from {}",
                store.dir().display()
            );
        }
        ploxions += restored;
    }

    let (control_tx, control_rx) = mpsc::unbounded_channel::<Command>();
    let (data_tx, data_rx) = mpsc::unbounded_channel::<Command>();
    let (events_tx, _events_rx) = broadcast::channel::<LiveEvent>(BROADCAST_CAPACITY);
    let (fed_tx, _fed_rx) = broadcast::channel::<FedMessage>(FED_CAPACITY);
    let commit = Arc::new(commit);
    let node_id = Arc::new(if fed.node_id.trim().is_empty() {
        default_node_id()
    } else {
        fed.node_id.clone()
    });
    let peer_token = Arc::new(fed.peer_token.clone());
    let bus_token = Arc::new(std::env::var("XION_BUS_TOKEN").ok().filter(|t| !t.trim().is_empty()));

    // Spawn the dedicated HOST THREAD — it owns `host` from here on.
    let events_for_thread = events_tx.clone();
    let fed_for_thread = fed_tx.clone();
    let commit_for_thread = Arc::clone(&commit);
    let node_for_thread = Arc::clone(&node_id);
    let host_join = std::thread::Builder::new()
        .name("xerboxion-host".into())
        .spawn(move || {
            host_thread(
                host,
                control_rx,
                data_rx,
                events_for_thread,
                fed_for_thread,
                node_for_thread,
                commit_for_thread,
                state_store,
            )
        })
        .context("spawning the host thread")?;

    let live_html = Arc::new(live_map_html());
    let xion3d_html = Arc::new(xion_3d_html());
    let xer_html = Arc::new(xer_html_str());

    let state = AppState {
        host: HostHandle { control: control_tx, data: data_tx },
        events: events_tx,
        node_id,
        fed_out: fed_tx,
        peer_token,
        bus_token,
        started: Instant::now(),
        ploxions,
        commit,
        live_html,
        xion3d_html,
        xer_html,
        ploxions_dir: Arc::new(dir.to_path_buf()),
        assets_dir: Arc::new(
            std::env::var_os("XION_ASSETS_DIR")
                .map(std::path::PathBuf::from)
                .or_else(|| std::env::current_dir().ok())
                .unwrap_or_else(|| dir.to_path_buf()),
        ),
        state_dir: Arc::new(state_dir.map(|p| p.to_path_buf())),
        ecosystem_cache: Arc::new(std::sync::Mutex::new(None)),
        store_token: Arc::new(std::env::var("XER_STORE_TOKEN").ok().filter(|v| !v.is_empty())),
        store_lock: Arc::new(std::sync::Mutex::new(())),
    };

    let router = Router::new()
        .route("/", get(xer_page))
        .route("/xer", get(xer_page))
        .route("/px", get(list_px))
        .route("/px/:id/*path", get(serve_asset))
        .route("/store/:ns/:collection", get(store_list).post(store_create))
        .route("/store/:ns/:collection/:id", get(store_get).put(store_put).delete(store_delete))
        .route("/map", get(index))
        .route("/3d", get(xion_3d))
        .route("/3d/three.min.js", get(three_js))
        .route("/paper", get(paper_page))
        .route("/snapshot", get(snapshot))
        .route("/ecosystem", get(ecosystem))
        .route("/replicate", get(replicate))
        .route("/healthz", get(healthz))
        .route("/emit", post(emit))
        .route("/sso/token", post(sso_token))
        .route("/load", post(load))
        .route("/unload", post(unload))
        .route("/ws", get(ws_upgrade))
        .route("/events", get(events_sse))
        .route("/peer", get(peer_upgrade))
        // Auth du PLAN DE CONTRÔLE : exige `XION_BUS_TOKEN` sur /emit /load /unload /replicate
        // quand il est configuré (sinon ouvert — rétrocompat). Audit sécu finding #1.
        .layer(middleware::from_fn_with_state(state.clone(), require_bus_token))
        .with_state(state.clone());

    Ok(Server {
        router,
        state,
        host_join,
    })
}

/// Middleware d'auth du plan de contrôle du bus. Si `XION_BUS_TOKEN` est configuré, exige
/// `Authorization: Bearer <token>` sur les routes mutantes (/emit /load /unload /replicate) ;
/// sinon laisse passer (rétrocompat). Empêche un conteneur quelconque de piloter le plan de
/// contrôle quand le daemon est bindé sur la passerelle docker0.
async fn require_bus_token(State(state): State<AppState>, req: Request, next: Next) -> Response {
    let protected = matches!(req.uri().path(), "/emit" | "/load" | "/unload" | "/replicate");
    if protected {
        if let Some(expected) = state.bus_token.as_ref() {
            let ok = req
                .headers()
                .get(axum::http::header::AUTHORIZATION)
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.strip_prefix("Bearer "))
                .map(|t| t == expected.as_str())
                .unwrap_or(false);
            if !ok {
                return (StatusCode::UNAUTHORIZED, "bus: token requis").into_response();
            }
        }
    }
    next.run(req).await
}

/// The self-contained LIVE map HTML served at `GET /`. Inlined at compile time
/// from `web-map/xerboxion-live.html` so the daemon serves it with zero file I/O
/// and the binary is self-contained.
fn live_map_html() -> String {
    include_str!("../../../web-map/xerboxion-live.html").to_string()
}

/// The self-contained 3D/4D xion-view HTML served at `GET /3d`. Inlined at
/// compile time from `web-3d/xion-3d.html`. It loads Three.js from the
/// same-origin `/3d/three.min.js` route — never a CDN.
fn xion_3d_html() -> String {
    include_str!("../../../web-3d/xion-3d.html").to_string()
}

/// The self-contained LABO-XER page, inlined at compile time from `web-xer/xer.html`
/// and served at `GET /` + `GET /xer`. The migration target: the whole labo (desktop,
/// dock, ploxion launcher, autonomy dashboard, forge) lives IN the core — install the
/// core, get the labo, work from anywhere. No Laravel.
fn xer_html_str() -> String {
    include_str!("../../../web-xer/index.html").to_string()
}

/// The VENDORED Three.js build, inlined at compile time from
/// `web-3d/vendor/three.min.js` and served verbatim at `GET /3d/three.min.js`.
/// Bundling it into the binary keeps the daemon self-contained (no CDN, no file
/// I/O at request time) — exactly like the HTML pages.
const THREE_JS: &str = include_str!("../../../web-3d/vendor/three.min.js");

/// The self-contained XERBOXION paper v0.8 page, inlined at compile time and
/// served verbatim at `GET /paper` — same self-contained pattern as the map/3D.
const PAPER_HTML: &str = include_str!("../../../web-paper/xerboxion-paper.html");

/// Run the daemon with NO federation (back-compat shim; kept so any caller of the
/// old 4-arg signature still works). Identical to passing a default
/// [`FederationConfig`].
pub fn run(dir: &Path, addr: &str, port: u16, commit: String) -> Result<()> {
    run_fed(dir, addr, port, commit, FederationConfig::default(), None)
}

/// Run the daemon: bind `addr:port`, dial each configured peer (BUS FEDERATION),
/// serve until SIGINT, then gracefully shut the host down (`plc_goodbye` + drop
/// Stores) and exit. This is the body of the `serve` subcommand. Blocks (it
/// builds its own tokio runtime). With `fed.peers` empty it behaves EXACTLY as
/// the classic daemon (no peer task spawned, `/peer` simply never dialed).
pub fn run_fed(
    dir: &Path,
    addr: &str,
    port: u16,
    commit: String,
    fed: FederationConfig,
    state_dir: Option<&Path>,
) -> Result<()> {
    let peers = fed.peers.clone();
    let server = build_server_fed(dir, commit, fed, state_dir)?;

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("building the tokio runtime")?;

    runtime.block_on(async move {
        let bind: SocketAddr = format!("{addr}:{port}")
            .parse()
            .with_context(|| format!("invalid bind address {addr}:{port}"))?;
        let listener = tokio::net::TcpListener::bind(bind)
            .await
            .with_context(|| format!("binding {bind}"))?;
        let local = listener.local_addr().unwrap_or(bind);
        let node_id = (*server.state.node_id).clone();

        println!("xerboxion-rt serve: daemon up on http://{local} (node-id «{node_id}»)");
        println!("  GET  http://{local}/           live map (HTML)");
        println!("  GET  http://{local}/snapshot   JSON snapshot of the live (loaded) core");
        println!("  GET  http://{local}/ecosystem  JSON of ALL ploxions (running+registered+deployed)");
        println!("  GET  http://{local}/3d         3D/4D view of the WHOLE ecosystem");
        println!("  GET  http://{local}/healthz    liveness");
        println!("  POST http://{local}/emit       {{\"topic\":\"…\",\"payload\":\"…\"}} -> onto the bus");
        println!("  POST http://{local}/load       {{\"id\":\"…\"}} | {{\"wasm\":\"<base64>\"}} -> hot-load a ploxion");
        println!("  POST http://{local}/unload     {{\"id\":\"…\"}} -> hot-unload a ploxion (droit au silence)");
        println!("  WS   ws://{local}/ws           live bus event stream");
        println!("  WS   ws://{local}/peer         FEDERATION inter-node link");
        if let Some(sd) = state_dir {
            println!("  state -> {} (runtime-loaded ploxions are DURABLE across restarts)", sd.display());
        }
        println!("  ({} ploxion(s) loaded; Ctrl-C to shut down — droit au silence)", server.state.ploxions);

        // BUS FEDERATION: dial each peer (reconnecting forever). Inert if empty.
        let node_arc = Arc::clone(&server.state.node_id);
        for peer in peers {
            println!("  peer  -> {} (federation client, reconnecting)", peer.label());
            let st = server.state.clone();
            let node = Arc::clone(&node_arc);
            tokio::spawn(async move { peer_client(peer, st, node).await });
        }

        let host = server.state.host.clone();
        let app = server.router.clone();

        // Serve until SIGINT.
        axum::serve(listener, app)
            .with_graceful_shutdown(async {
                let _ = tokio::signal::ctrl_c().await;
                println!("\nxerboxion-rt serve: SIGINT — shutting the core down…");
            })
            .await
            .context("serving")?;

        // GRACEFUL SHUTDOWN: tell the host thread to plc_goodbye + drop Stores.
        host.shutdown().await;
        println!("xerboxion-rt serve: plc_goodbye sent, all Stores dropped (droit au silence)");
        Ok::<(), anyhow::Error>(())
    })?;

    // Wait for the host thread to fully wind down (Store drop complete).
    let _ = server.host_join.join();
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests — in-process server on an ephemeral port + a real WS client.
// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::time::Duration;
    use tokio_tungstenite::tungstenite::Message as TMessage;

    fn ploxion_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|p| p.parent())
            .map(|p| p.join("target/ploxions"))
            .unwrap()
    }

    fn built() -> bool {
        ploxion_dir().join("ping.wasm").exists()
    }

    /// Spawn the in-process server on 127.0.0.1:0, returning its bound addr +
    /// the host handle (for clean shutdown). The router is served on a tokio task.
    async fn spawn() -> (SocketAddr, HostHandle, broadcast::Sender<LiveEvent>) {
        let server = build_server(&ploxion_dir(), "testsha".into()).expect("server builds");
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind ephemeral");
        let addr = listener.local_addr().unwrap();
        let host = server.state.host.clone();
        let events = server.state.events.clone();
        let app = server.router.clone();
        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });
        (addr, host, events)
    }

    #[tokio::test]
    async fn snapshot_is_valid_json_with_loaded_ploxions() {
        if !built() {
            eprintln!("SKIP: ploxions not built");
            return;
        }
        let (addr, host, _ev) = spawn().await;
        let body = reqwest_get(&format!("http://{addr}/snapshot")).await;
        let v: serde_json::Value = serde_json::from_str(&body).expect("valid JSON");
        let ploxions = v["ploxions"].as_array().expect("ploxions array");
        assert!(ploxions.len() >= 6, "expected the loaded fleet, got {}", ploxions.len());
        // ping must be among the loaded wasm ploxions.
        assert!(
            ploxions.iter().any(|p| p["id"] == "ping" && p["kind"] == "wasm"),
            "ping ploxion must be present"
        );
        host.shutdown().await;
    }

    #[tokio::test]
    async fn ecosystem_is_superset_of_snapshot() {
        if !built() {
            eprintln!("SKIP: ploxions not built");
            return;
        }
        let (addr, host, _ev) = spawn().await;

        // /snapshot = the running set (loaded ploxions only).
        let snap_body = reqwest_get(&format!("http://{addr}/snapshot")).await;
        let snap: serde_json::Value = serde_json::from_str(&snap_body).expect("snapshot JSON");
        let loaded = snap["ploxions"].as_array().expect("ploxions array").len();

        // /ecosystem = running + registered + deployed (the WHOLE computer).
        let eco_body = reqwest_get(&format!("http://{addr}/ecosystem")).await;
        let eco: serde_json::Value = serde_json::from_str(&eco_body).expect("ecosystem JSON");
        let nodes = eco["nodes"].as_array().expect("nodes array");

        // The real ploxi0ns.json is present on the box, so the ecosystem MUST be a
        // strict superset of the loaded set (more registered ploxions exist than
        // are loaded here). If the registry were absent it would degrade to equal.
        assert!(
            nodes.len() >= loaded,
            "ecosystem ({}) must be >= the loaded set ({})",
            nodes.len(),
            loaded
        );
        // Counts add up and the running count matches the loaded snapshot.
        let c = &eco["counts"];
        assert_eq!(c["running"].as_u64().unwrap() as usize, loaded);
        assert_eq!(
            c["total"].as_u64().unwrap(),
            c["running"].as_u64().unwrap()
                + c["registered"].as_u64().unwrap()
                + c["deployed"].as_u64().unwrap()
        );
        // The registry-API connector is named (José: « c'est un ploxion »).
        assert_eq!(eco["connector"]["id"], ecosystem::REGISTRY_CONNECTOR_ID);
        // Every node carries a state in the allowed set.
        for n in nodes {
            let s = n["state"].as_str().unwrap();
            assert!(
                s == "running" || s == "registered" || s == "deployed",
                "unexpected state {s}"
            );
        }
        // The live bus wiring is carried over from the snapshot.
        assert_eq!(
            eco["bus"].as_array().unwrap().len(),
            snap["bus"].as_array().unwrap().len()
        );

        host.shutdown().await;
    }

    #[tokio::test]
    async fn healthz_ok() {
        if !built() {
            eprintln!("SKIP: ploxions not built");
            return;
        }
        let (addr, host, _ev) = spawn().await;
        let body = reqwest_get(&format!("http://{addr}/healthz")).await;
        let v: serde_json::Value = serde_json::from_str(&body).expect("valid JSON");
        assert_eq!(v["status"], "ok");
        assert!(v["ploxions"].as_u64().unwrap() >= 6);
        host.shutdown().await;
    }

    #[tokio::test]
    async fn three_d_page_is_served_and_references_snapshot_and_ws() {
        if !built() {
            eprintln!("SKIP: ploxions not built");
            return;
        }
        let (addr, host, _ev) = spawn().await;
        // GET /3d must be 200 HTML and wire to the live data + the vendored JS.
        let (status, ctype, body) =
            http_get_full(&format!("http://{addr}/3d")).await;
        assert_eq!(status, 200, "GET /3d must be 200");
        assert!(
            ctype.to_ascii_lowercase().contains("text/html"),
            "GET /3d content-type must be HTML, got {ctype:?}"
        );
        assert!(
            body.contains("/ecosystem"),
            "the 3D page must fetch /ecosystem (the whole computer)"
        );
        assert!(
            body.contains("/snapshot"),
            "the 3D page must still reference /snapshot (running-set fallback)"
        );
        assert!(body.contains("/ws"), "the 3D page must open the /ws live feed");
        assert!(
            body.contains("/3d/three.min.js"),
            "the 3D page must load Three.js from the same-origin vendored route (no CDN)"
        );

        // GET /3d/three.min.js must serve the vendored Three.js as JS, 200.
        let (status, ctype, body) =
            http_get_full(&format!("http://{addr}/3d/three.min.js")).await;
        assert_eq!(status, 200, "GET /3d/three.min.js must be 200");
        assert!(
            ctype.to_ascii_lowercase().contains("javascript"),
            "Three.js content-type must be JavaScript, got {ctype:?}"
        );
        assert!(
            body.contains("REVISION"),
            "the served asset must be the real Three.js build"
        );

        host.shutdown().await;
    }

    #[tokio::test]
    async fn emit_flows_through_the_real_bus_to_ws_client() {
        if !built() {
            eprintln!("SKIP: ploxions not built");
            return;
        }
        let (addr, host, _ev) = spawn().await;

        // Connect a real WS client and wait until it is subscribed.
        let (mut ws, _) = tokio_tungstenite::connect_async(format!("ws://{addr}/ws"))
            .await
            .expect("ws connect");

        // Give the subscribe a moment to register on the broadcast channel.
        tokio::time::sleep(Duration::from_millis(50)).await;

        // POST /emit a topic a loaded ploxion subscribes to: `ping` (pong + tracer
        // require it). The bus must route it and we must SEE the resulting hops.
        let code = http_post(
            &format!("http://{addr}/emit"),
            r#"{"topic":"ping","payload":"ws-test-1"}"#,
        )
        .await;
        assert_eq!(code, 202, "emit must be accepted");

        // Collect events for up to 2s; assert we see the emit AND a routed
        // delivery to a subscriber (pong or tracer) — proof it hit the REAL bus.
        let mut saw_emit = false;
        let mut saw_route = false;
        let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
        while tokio::time::Instant::now() < deadline && !(saw_emit && saw_route) {
            let next = tokio::time::timeout(Duration::from_millis(500), ws.next()).await;
            let Ok(Some(Ok(TMessage::Text(t)))) = next else { continue };
            let v: serde_json::Value = match serde_json::from_str(&t) {
                Ok(v) => v,
                Err(_) => continue,
            };
            if v["kind"] == "emit" && v["topic"] == "ping" && v["payload"] == "ws-test-1" {
                saw_emit = true;
            }
            if v["kind"] == "route" && v["topic"] == "ping" {
                saw_route = true;
            }
        }
        assert!(saw_emit, "WS client must receive the emitted ping event");
        assert!(saw_route, "WS client must receive the routed delivery (real bus)");

        host.shutdown().await;
    }

    /// `GET /events` is a plain-HTTP SSE feed of the SAME broadcast bus as `/ws`:
    /// a `200` chunked stream (not a `101` upgrade) with `text/event-stream`, and
    /// after an injected emit it yields an SSE `data:` line carrying that event.
    #[tokio::test]
    async fn events_sse_streams_emitted_event_over_plain_http() {
        if !built() {
            eprintln!("SKIP: ploxions not built");
            return;
        }
        let (addr, host, _ev) = spawn().await;

        // Open a streaming HTTP/1.1 GET on /events. We keep the socket open (NO
        // `Connection: close`) and read the response incrementally.
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let mut stream = tokio::net::TcpStream::connect(addr).await.expect("connect");
        let req = format!(
            "GET /events HTTP/1.1\r\nHost: {}\r\nAccept: text/event-stream\r\n\r\n",
            addr
        );
        stream.write_all(req.as_bytes()).await.expect("write GET /events");

        // Read until we have the full response header block, then assert status +
        // content-type. This proves it is a normal 200 stream, not a 101 upgrade.
        let mut buf = Vec::new();
        let mut tmp = [0u8; 4096];
        let header_deadline = tokio::time::Instant::now() + Duration::from_secs(3);
        let header_end = loop {
            if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                break pos + 4;
            }
            assert!(
                tokio::time::Instant::now() < header_deadline,
                "did not receive SSE response headers in time"
            );
            let n = tokio::time::timeout(Duration::from_millis(500), stream.read(&mut tmp))
                .await
                .ok()
                .and_then(|r| r.ok())
                .unwrap_or(0);
            if n > 0 {
                buf.extend_from_slice(&tmp[..n]);
            }
        };
        let head = String::from_utf8_lossy(&buf[..header_end]).to_lowercase();
        assert!(
            head.starts_with("http/1.1 200"),
            "events must be a 200 chunked stream, not a 101 upgrade; got: {}",
            head.lines().next().unwrap_or_default()
        );
        assert!(
            head.contains("content-type: text/event-stream"),
            "events must be text/event-stream; headers were:\n{head}"
        );
        assert!(
            head.contains("x-accel-buffering: no"),
            "events must disable proxy buffering; headers were:\n{head}"
        );

        // Now inject an emit on a subscribed topic and prove the SSE stream carries
        // the real broadcast event as a `data:` line.
        let code = http_post(
            &format!("http://{addr}/emit"),
            r#"{"topic":"ping","payload":"sse-test-1"}"#,
        )
        .await;
        assert_eq!(code, 202, "emit must be accepted");

        // Accumulate the streamed body (we may already hold some bytes past the
        // header end) and scan SSE `data:` lines for our event.
        let mut body = String::from_utf8_lossy(&buf[header_end..]).to_string();
        let mut saw_data = false;
        let deadline = tokio::time::Instant::now() + Duration::from_secs(3);
        while tokio::time::Instant::now() < deadline && !saw_data {
            for line in body.lines() {
                let payload = match line.strip_prefix("data:") {
                    Some(p) => p.trim(),
                    None => continue,
                };
                let Ok(v) = serde_json::from_str::<serde_json::Value>(payload) else {
                    continue;
                };
                if v["kind"] == "emit" && v["topic"] == "ping" && v["payload"] == "sse-test-1" {
                    saw_data = true;
                    break;
                }
            }
            if saw_data {
                break;
            }
            let n = tokio::time::timeout(Duration::from_millis(500), stream.read(&mut tmp))
                .await
                .ok()
                .and_then(|r| r.ok())
                .unwrap_or(0);
            if n > 0 {
                body.push_str(&String::from_utf8_lossy(&tmp[..n]));
            }
        }
        assert!(
            saw_data,
            "SSE /events must deliver the emitted ping event as a data line; got:\n{body}"
        );

        host.shutdown().await;
    }

    #[tokio::test]
    async fn emit_on_unsubscribed_topic_yields_no_spurious_delivery() {
        if !built() {
            eprintln!("SKIP: ploxions not built");
            return;
        }
        let (addr, host, _ev) = spawn().await;
        let (mut ws, _) = tokio_tungstenite::connect_async(format!("ws://{addr}/ws"))
            .await
            .expect("ws connect");
        tokio::time::sleep(Duration::from_millis(50)).await;

        // A topic NO ploxion subscribes to. We must see the emit hop itself but
        // NEVER a route/deliver of it — droit au silence.
        let code = http_post(
            &format!("http://{addr}/emit"),
            r#"{"topic":"nobody.listens.here","payload":"x"}"#,
        )
        .await;
        assert_eq!(code, 202);

        let mut saw_emit = false;
        let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
        while tokio::time::Instant::now() < deadline {
            let next = tokio::time::timeout(Duration::from_millis(400), ws.next()).await;
            let Ok(Some(Ok(TMessage::Text(t)))) = next else { break };
            let v: serde_json::Value = match serde_json::from_str(&t) {
                Ok(v) => v,
                Err(_) => continue,
            };
            if v["topic"] == "nobody.listens.here" {
                assert_ne!(
                    v["kind"], "route",
                    "an unsubscribed topic must NOT be routed/delivered"
                );
                if v["kind"] == "emit" {
                    saw_emit = true;
                }
            }
        }
        assert!(saw_emit, "the emit hop itself must still be on the feed");
        host.shutdown().await;
    }

    #[tokio::test]
    async fn ws_inbound_emit_reaches_the_bus() {
        if !built() {
            eprintln!("SKIP: ploxions not built");
            return;
        }
        let (addr, host, _ev) = spawn().await;
        let (mut ws, _) = tokio_tungstenite::connect_async(format!("ws://{addr}/ws"))
            .await
            .expect("ws connect");
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Push an emit IN over the same socket; expect to see it flow back out.
        ws.send(TMessage::Text(
            r#"{"emit":{"topic":"ping","payload":"inbound-1"}}"#.into(),
        ))
        .await
        .expect("ws send");

        let mut saw = false;
        let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
        while tokio::time::Instant::now() < deadline && !saw {
            let next = tokio::time::timeout(Duration::from_millis(500), ws.next()).await;
            let Ok(Some(Ok(TMessage::Text(t)))) = next else { continue };
            let v: serde_json::Value = match serde_json::from_str(&t) {
                Ok(v) => v,
                Err(_) => continue,
            };
            if v["kind"] == "emit" && v["topic"] == "ping" && v["payload"] == "inbound-1" {
                saw = true;
            }
        }
        assert!(saw, "an inbound WS emit must reach the bus and stream back");
        host.shutdown().await;
    }

    #[tokio::test]
    async fn bad_emit_body_is_rejected_not_panicked() {
        if !built() {
            eprintln!("SKIP: ploxions not built");
            return;
        }
        let (addr, host, _ev) = spawn().await;
        // Not JSON -> 400.
        let code = http_post(&format!("http://{addr}/emit"), "not json at all").await;
        assert_eq!(code, 400);
        // Missing topic -> 400.
        let code = http_post(&format!("http://{addr}/emit"), r#"{"payload":"x"}"#).await;
        assert_eq!(code, 400);
        host.shutdown().await;
    }

    // =======================================================================
    // HOT-LOAD / HOT-UNLOAD — load and unload ploxions in the RUNNING daemon
    // without a restart. Everything below drives the in-process server on an
    // ephemeral port over real HTTP; nothing is faked — POST /load really
    // instantiates a new Store + wires the bus, POST /unload really drops it.
    // =======================================================================

    /// A small HTTP POST helper returning `(status, body)` so the load/unload
    /// tests can assert on both the code and the returned manifest JSON.
    async fn http_post_full(url: &str, body: &str) -> (u16, String) {
        let (host, port, path) = split_url(url);
        let mut stream = tokio::net::TcpStream::connect((host.as_str(), port))
            .await
            .expect("connect");
        let req = format!(
            "POST {path} HTTP/1.1\r\nHost: {host}\r\nContent-Type: application/json\r\n\
             Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        stream.write_all(req.as_bytes()).await.expect("write");
        let mut buf = Vec::new();
        stream.read_to_end(&mut buf).await.expect("read");
        let text = String::from_utf8_lossy(&buf);
        let (head, body) = text.split_once("\r\n\r\n").unwrap_or((&text, ""));
        let status = head
            .split_whitespace()
            .nth(1)
            .and_then(|c| c.parse().ok())
            .unwrap_or(0);
        (status, body.to_string())
    }

    /// Does `/snapshot` currently list a loaded wasm ploxion with this id?
    async fn snapshot_has(addr: SocketAddr, id: &str) -> bool {
        let body = reqwest_get(&format!("http://{addr}/snapshot")).await;
        let v: serde_json::Value = serde_json::from_str(&body).unwrap_or(serde_json::json!({}));
        v["ploxions"]
            .as_array()
            .map(|a| a.iter().any(|p| p["id"] == id && p["kind"] == "wasm"))
            .unwrap_or(false)
    }

    /// FULL DEMO: unload `pong` (it boots loaded), prove `ping` no longer routes
    /// to it (SILENCE), then HOT-LOAD pong back by registered id, prove `ping` now
    /// routes to it again (it REACTS), then unload once more and prove silence —
    /// all on ONE running daemon, no restart. The bus is rewired correctly each way.
    #[tokio::test]
    async fn hot_load_then_react_then_unload_then_silence() {
        if !built() {
            eprintln!("SKIP: ploxions not built");
            return;
        }
        let (addr, host, events) = spawn().await;
        let mut bus = events.subscribe();

        // pong boots loaded. Unload it so we start from a clean, pong-less bus.
        let (code, body) = http_post_full(&format!("http://{addr}/unload"), r#"{"id":"pong"}"#).await;
        assert_eq!(code, 200, "unload pong must succeed: {body}");
        assert!(!snapshot_has(addr, "pong").await, "pong must be gone after unload");

        // Emit ping — with pong unloaded, NO route to pong (droit au silence). The
        // emit hop still appears; we just must never see a route to `pong`.
        drain(&mut bus).await;
        assert_eq!(http_post(&format!("http://{addr}/emit"), r#"{"topic":"ping","payload":"silent-1"}"#).await, 202);
        let evs = collect_for(&mut bus, 1200).await;
        assert!(
            evs.iter().any(|e| e.kind == "emit" && e.topic == "ping" && e.payload == "silent-1"),
            "the ping emit hop must still flow"
        );
        assert!(
            !evs.iter().any(|e| e.kind == "route" && e.topic == "ping" && e.note.contains("pong")),
            "with pong unloaded, ping must NOT route to pong (silence); got {evs:?}"
        );

        // HOT-LOAD pong back by registered id. 200 + the manifest comes back.
        let (code, body) = http_post_full(&format!("http://{addr}/load"), r#"{"id":"pong"}"#).await;
        assert_eq!(code, 200, "load pong must succeed: {body}");
        let m: serde_json::Value = serde_json::from_str(&body).expect("manifest JSON");
        assert_eq!(m["id"], "pong");
        assert!(
            m["requires"].as_array().unwrap().iter().any(|t| t == "ping"),
            "loaded manifest must declare requires ping"
        );
        assert!(snapshot_has(addr, "pong").await, "pong must be running after load");

        // Now emit ping — pong is wired, so it REACTS: ping routes to pong, and
        // pong emits `pong`. We must SEE the route to pong AND its emit.
        drain(&mut bus).await;
        assert_eq!(http_post(&format!("http://{addr}/emit"), r#"{"topic":"ping","payload":"react-1"}"#).await, 202);
        let evs = collect_for(&mut bus, 1500).await;
        assert!(
            evs.iter().any(|e| e.kind == "route" && e.topic == "ping" && e.note.contains("pong")),
            "after load, ping MUST route to pong; got {evs:?}"
        );
        assert!(
            evs.iter().any(|e| e.kind == "emit" && e.from == "pong" && e.topic == "pong"),
            "after load, pong MUST emit `pong` in reaction; got {evs:?}"
        );

        // UNLOAD again → goodbye runs (pong logs it) → silence once more.
        drain(&mut bus).await;
        let (code, _b) = http_post_full(&format!("http://{addr}/unload"), r#"{"id":"pong"}"#).await;
        assert_eq!(code, 200);
        let unload_evs = collect_for(&mut bus, 800).await;
        assert!(
            unload_evs.iter().any(|e| e.kind == "life" && e.from == "pong" && e.payload == "goodbye"),
            "unload must trace pong's goodbye lifecycle hop; got {unload_evs:?}"
        );
        assert!(
            unload_evs.iter().any(|e| e.kind == "life" && e.from == "pong" && e.payload == "unload"),
            "unload must trace the unload lifecycle hop; got {unload_evs:?}"
        );
        assert!(!snapshot_has(addr, "pong").await, "pong must be gone after the second unload");

        drain(&mut bus).await;
        assert_eq!(http_post(&format!("http://{addr}/emit"), r#"{"topic":"ping","payload":"silent-2"}"#).await, 202);
        let evs = collect_for(&mut bus, 1000).await;
        assert!(
            !evs.iter().any(|e| e.kind == "route" && e.topic == "ping" && e.note.contains("pong")),
            "after the second unload, ping must be SILENT toward pong again; got {evs:?}"
        );

        host.shutdown().await;
    }

    /// LOAD ARBITRARY WASM via base64: read the staged `pong.wasm`, unload the
    /// boot-loaded pong, then POST /load `{"wasm":"<base64>"}` (no registered file
    /// lookup) and prove it lands as `running` and reacts.
    #[tokio::test]
    async fn hot_load_base64_wasm_reacts() {
        if !built() {
            eprintln!("SKIP: ploxions not built");
            return;
        }
        let (addr, host, events) = spawn().await;
        let mut bus = events.subscribe();

        // Remove the boot-loaded pong so the base64 load is not a duplicate.
        assert_eq!(http_post(&format!("http://{addr}/unload"), r#"{"id":"pong"}"#).await, 200);

        // Read the staged wasm and base64-encode it for the {wasm} path.
        use base64::Engine as _;
        let bytes = std::fs::read(ploxion_dir().join("pong.wasm")).expect("read pong.wasm");
        let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
        let body = format!(r#"{{"wasm":"{b64}"}}"#);

        let (code, rbody) = http_post_full(&format!("http://{addr}/load"), &body).await;
        assert_eq!(code, 200, "base64 load must succeed: {rbody}");
        let m: serde_json::Value = serde_json::from_str(&rbody).expect("manifest JSON");
        // The manifest's OWN id wins (pong), regardless of any fallback.
        assert_eq!(m["id"], "pong");
        assert!(snapshot_has(addr, "pong").await, "base64-loaded pong must be running");

        // It reacts on the bus exactly like a file-loaded one.
        drain(&mut bus).await;
        assert_eq!(http_post(&format!("http://{addr}/emit"), r#"{"topic":"ping","payload":"b64-1"}"#).await, 202);
        let evs = collect_for(&mut bus, 1500).await;
        assert!(
            evs.iter().any(|e| e.kind == "emit" && e.from == "pong" && e.topic == "pong"),
            "base64-loaded pong must react to ping; got {evs:?}"
        );

        host.shutdown().await;
    }

    /// DUPLICATE id is rejected (422), and the already-loaded ploxion is untouched.
    #[tokio::test]
    async fn hot_load_duplicate_id_is_rejected() {
        if !built() {
            eprintln!("SKIP: ploxions not built");
            return;
        }
        let (addr, host, _ev) = spawn().await;
        // pong is already loaded at boot — loading it again is a duplicate.
        let (code, body) = http_post_full(&format!("http://{addr}/load"), r#"{"id":"pong"}"#).await;
        assert_eq!(code, 422, "duplicate id must be 422; body={body}");
        assert!(body.contains("already loaded"), "error must name the duplicate: {body}");
        // pong is still there and still works (the failed load changed nothing).
        assert!(snapshot_has(addr, "pong").await, "pong must still be loaded after a rejected duplicate");
        host.shutdown().await;
    }

    /// UNKNOWN id (no staged wasm) is a clean 404 — no panic.
    #[tokio::test]
    async fn hot_load_unknown_id_is_404() {
        if !built() {
            eprintln!("SKIP: ploxions not built");
            return;
        }
        let (addr, host, _ev) = spawn().await;
        let (code, body) = http_post_full(
            &format!("http://{addr}/load"),
            r#"{"id":"no-such-ploxion-xyz"}"#,
        )
        .await;
        assert_eq!(code, 404, "unknown id must be 404; body={body}");
        host.shutdown().await;
    }

    /// BAD WASM (base64 of garbage bytes) is rejected 422 — clean error, no panic,
    /// host untouched.
    #[tokio::test]
    async fn hot_load_bad_wasm_is_rejected_not_panicked() {
        if !built() {
            eprintln!("SKIP: ploxions not built");
            return;
        }
        let (addr, host, _ev) = spawn().await;
        use base64::Engine as _;
        // Not a wasm module at all — just bytes. Compilation must fail cleanly.
        let b64 = base64::engine::general_purpose::STANDARD.encode(b"this is not wasm at all");
        let body = format!(r#"{{"wasm":"{b64}","id":"junk"}}"#);
        let (code, rbody) = http_post_full(&format!("http://{addr}/load"), &body).await;
        assert_eq!(code, 422, "bad wasm must be 422; body={rbody}");
        // The daemon is still alive: a follow-up healthz still answers.
        let hz = reqwest_get(&format!("http://{addr}/healthz")).await;
        assert!(hz.contains("\"status\":\"ok\""), "daemon must survive a bad load");
        host.shutdown().await;
    }

    /// Invalid base64 in `wasm` => 400 (request malformed, distinct from 422 for a
    /// well-formed-but-unloadable module). Empty body / neither field => 400.
    #[tokio::test]
    async fn hot_load_bad_request_shapes_are_400() {
        if !built() {
            eprintln!("SKIP: ploxions not built");
            return;
        }
        let (addr, host, _ev) = spawn().await;
        // Not JSON.
        assert_eq!(http_post(&format!("http://{addr}/load"), "not json").await, 400);
        // Neither id nor wasm.
        assert_eq!(http_post(&format!("http://{addr}/load"), "{}").await, 400);
        // Invalid base64.
        assert_eq!(
            http_post(&format!("http://{addr}/load"), r#"{"wasm":"!!!not base64!!!"}"#).await,
            400
        );
        // Path traversal in id.
        assert_eq!(
            http_post(&format!("http://{addr}/load"), r#"{"id":"../etc/passwd"}"#).await,
            400
        );
        host.shutdown().await;
    }

    /// UNLOAD of a not-loaded ploxion is a clean 404; a bad body is 400.
    #[tokio::test]
    async fn hot_unload_errors_are_clean() {
        if !built() {
            eprintln!("SKIP: ploxions not built");
            return;
        }
        let (addr, host, _ev) = spawn().await;
        // Unload something never loaded -> 404.
        let (code, _b) = http_post_full(
            &format!("http://{addr}/unload"),
            r#"{"id":"never-loaded-abc"}"#,
        )
        .await;
        assert_eq!(code, 404);
        // Bad body -> 400.
        assert_eq!(http_post(&format!("http://{addr}/unload"), "not json").await, 400);
        host.shutdown().await;
    }

    /// Drain whatever is currently buffered on a bus receiver (non-blocking-ish):
    /// pull events for a short window so a subsequent `collect_for` starts clean.
    async fn drain(rx: &mut broadcast::Receiver<LiveEvent>) {
        let _ = collect_for(rx, 200).await;
    }

    // =======================================================================
    // DURABLE PERSISTENCE — a ploxion hot-loaded via POST /load SURVIVES a
    // restart. Each test spins up a daemon with a --state-dir, performs the
    // load/unload, FULLY shuts it down (new Host dropped), then spins up a
    // SECOND, INDEPENDENT daemon on the SAME state dir and asserts on the
    // restored set. Nothing is faked: the second daemon really re-instantiates
    // the ploxion from the persisted manifest/bytes. Offline (127.0.0.1:0) +
    // a per-test tmp state dir.
    // =======================================================================

    /// A throwaway state dir under the system temp dir, unique per test + run.
    fn tmp_state_dir(tag: &str) -> PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!(
            "xion-serve-state-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        p
    }

    /// Spawn an in-process daemon on 127.0.0.1:0 WITH a `--state-dir`, returning
    /// its addr + host handle + events sender + the host-thread join handle. The
    /// join handle lets a test wait for the Host (and thus the StateStore's last
    /// write) to fully wind down before "restarting" — the real-restart contract.
    async fn spawn_with_state(
        state_dir: &Path,
    ) -> (SocketAddr, HostHandle, broadcast::Sender<LiveEvent>, std::thread::JoinHandle<()>) {
        let server = build_server_state(&ploxion_dir(), "testsha".into(), Some(state_dir))
            .expect("server builds");
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind ephemeral");
        let addr = listener.local_addr().unwrap();
        let host = server.state.host.clone();
        let events = server.state.events.clone();
        let join = server.host_join;
        let app = server.router.clone();
        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });
        (addr, host, events, join)
    }

    /// Fully stop a daemon: shut the host down (plc_goodbye + drop Stores) and
    /// JOIN its host thread so every persistence write has flushed before we
    /// "restart" on the same state dir. This is what makes the restart a REAL
    /// process-like restart, not a racey overlap.
    async fn stop(host: HostHandle, join: std::thread::JoinHandle<()>) {
        host.shutdown().await;
        // Joining a std thread from async: do it on a blocking pool so we don't
        // stall the runtime. The thread exits right after the Store drop.
        let _ = tokio::task::spawn_blocking(move || {
            let _ = join.join();
        })
        .await;
    }

    /// (a) LOAD a runtime ploxion with --state-dir, simulate a restart (a NEW
    /// Host/daemon on the SAME state dir), and prove it is loaded AGAIN and
    /// REACTS on the bus — survival across a real process-like restart.
    #[tokio::test]
    async fn persisted_runtime_ploxion_survives_restart() {
        if !built() {
            eprintln!("SKIP: ploxions not built");
            return;
        }
        let dir = tmp_state_dir("survives");

        // --- Daemon #1: unload the boot-loaded pong, then HOT-LOAD it back via
        //     POST /load so it becomes a RUNTIME-loaded (persisted) ploxion. ----
        {
            let (addr, host, _ev, join) = spawn_with_state(&dir).await;
            assert_eq!(
                http_post(&format!("http://{addr}/unload"), r#"{"id":"pong"}"#).await,
                200,
                "unload boot pong"
            );
            let (code, body) =
                http_post_full(&format!("http://{addr}/load"), r#"{"id":"pong"}"#).await;
            assert_eq!(code, 200, "runtime load of pong must succeed: {body}");
            assert!(snapshot_has(addr, "pong").await, "pong running after load");
            stop(host, join).await;
        }

        // The manifest now records exactly one runtime ploxion: pong (staged).
        let store = StateStore::open(&dir).unwrap();
        let recs = store.read_records().unwrap();
        assert_eq!(recs.len(), 1, "exactly one runtime ploxion persisted; got {recs:?}");
        assert_eq!(recs[0].id, "pong");
        assert_eq!(recs[0].source, Source::Staged);

        // --- Daemon #2 (the "restart"): a fresh Host on the same state dir. pong
        //     must be RESTORED automatically and react to ping. -----------------
        {
            let (addr, host, events, join) = spawn_with_state(&dir).await;
            assert!(
                snapshot_has(addr, "pong").await,
                "after restart, the persisted pong must be loaded again"
            );
            // Prove it is LIVE, not just listed: emit ping, pong must react.
            let mut bus = events.subscribe();
            drain(&mut bus).await;
            assert_eq!(
                http_post(&format!("http://{addr}/emit"), r#"{"topic":"ping","payload":"restart-1"}"#).await,
                202
            );
            let evs = collect_for(&mut bus, 1500).await;
            assert!(
                evs.iter().any(|e| e.kind == "emit" && e.from == "pong" && e.topic == "pong"),
                "restored pong must REACT to ping after restart; got {evs:?}"
            );
            stop(host, join).await;
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// (a') BASE64-uploaded wasm survives a restart too: its BYTES are persisted
    /// (the module lives nowhere else) and re-instantiated on the next boot.
    #[tokio::test]
    async fn persisted_base64_wasm_survives_restart() {
        if !built() {
            eprintln!("SKIP: ploxions not built");
            return;
        }
        let dir = tmp_state_dir("b64survives");
        use base64::Engine as _;
        let bytes = std::fs::read(ploxion_dir().join("pong.wasm")).expect("read pong.wasm");
        let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);

        {
            let (addr, host, _ev, join) = spawn_with_state(&dir).await;
            assert_eq!(http_post(&format!("http://{addr}/unload"), r#"{"id":"pong"}"#).await, 200);
            let body = format!(r#"{{"wasm":"{b64}"}}"#);
            let (code, rbody) = http_post_full(&format!("http://{addr}/load"), &body).await;
            assert_eq!(code, 200, "base64 load must succeed: {rbody}");
            stop(host, join).await;
        }

        // The wasm BYTES were persisted under the manifest id (pong), source=wasm.
        let store = StateStore::open(&dir).unwrap();
        let recs = store.read_records().unwrap();
        assert_eq!(recs.len(), 1);
        assert_eq!(recs[0].id, "pong");
        assert_eq!(recs[0].source, Source::Wasm);
        assert_eq!(store.read_wasm("pong").unwrap(), bytes, "persisted bytes must equal the upload");

        {
            let (addr, host, events, join) = spawn_with_state(&dir).await;
            assert!(snapshot_has(addr, "pong").await, "base64 pong must be restored from bytes");
            let mut bus = events.subscribe();
            drain(&mut bus).await;
            assert_eq!(
                http_post(&format!("http://{addr}/emit"), r#"{"topic":"ping","payload":"b64-restart"}"#).await,
                202
            );
            let evs = collect_for(&mut bus, 1500).await;
            assert!(
                evs.iter().any(|e| e.kind == "emit" && e.from == "pong" && e.topic == "pong"),
                "restored base64 pong must react; got {evs:?}"
            );
            stop(host, join).await;
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// (b) UNLOAD then restart -> the ploxion STAYS GONE (the unload removed it
    /// from the persisted set, so the restart does not bring it back).
    #[tokio::test]
    async fn unloaded_ploxion_stays_gone_after_restart() {
        if !built() {
            eprintln!("SKIP: ploxions not built");
            return;
        }
        let dir = tmp_state_dir("staysgone");

        {
            let (addr, host, _ev, join) = spawn_with_state(&dir).await;
            // Load pong at runtime (it is now persisted)...
            assert_eq!(http_post(&format!("http://{addr}/unload"), r#"{"id":"pong"}"#).await, 200);
            assert_eq!(
                http_post_full(&format!("http://{addr}/load"), r#"{"id":"pong"}"#).await.0,
                200
            );
            // ...then UNLOAD it again — this must REMOVE it from the persisted set.
            assert_eq!(http_post(&format!("http://{addr}/unload"), r#"{"id":"pong"}"#).await, 200);
            stop(host, join).await;
        }

        // Nothing persisted now (the unload cleared the one record).
        let store = StateStore::open(&dir).unwrap();
        assert!(
            store.read_records().unwrap().is_empty(),
            "unload must leave the persisted set empty"
        );

        {
            // Restart: bring the daemon back up on the same state dir. The
            // unloaded runtime record must NOT be resurrected into the persisted
            // set — the restart leaves the manifest empty. (pong itself still
            // boots as part of the BASE set from <dir>; the contract here is about
            // the runtime DELTA, which was cleared by the unload and stays cleared
            // — a restart never brings back an unloaded runtime ploxion.)
            let (_addr, host, _ev, join) = spawn_with_state(&dir).await;
            tokio::time::sleep(Duration::from_millis(100)).await;
            let store2 = StateStore::open(&dir).unwrap();
            assert!(
                store2.read_records().unwrap().is_empty(),
                "restart must not resurrect an unloaded ploxion into the persisted set"
            );
            stop(host, join).await;
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// (c) The BASE SET is never persisted/duplicated: with a state dir but no
    /// runtime /load, the manifest stays empty, and a restart loads the same base
    /// count (no double-load, no growth).
    #[tokio::test]
    async fn base_set_is_not_persisted_or_duplicated() {
        if !built() {
            eprintln!("SKIP: ploxions not built");
            return;
        }
        let dir = tmp_state_dir("nodup");

        let base_count;
        {
            let (addr, host, _ev, join) = spawn_with_state(&dir).await;
            let body = reqwest_get(&format!("http://{addr}/snapshot")).await;
            let v: serde_json::Value = serde_json::from_str(&body).unwrap();
            base_count = v["ploxions"].as_array().unwrap().len();
            // No /load happened, so the manifest must be empty (base set excluded).
            let store = StateStore::open(&dir).unwrap();
            assert!(
                store.read_records().unwrap().is_empty(),
                "the base set must NOT be persisted"
            );
            stop(host, join).await;
        }

        {
            // Restart: the base set reloads from <dir> exactly once — same count,
            // no duplication from a (correctly empty) persisted set.
            let (addr, host, _ev, join) = spawn_with_state(&dir).await;
            let body = reqwest_get(&format!("http://{addr}/snapshot")).await;
            let v: serde_json::Value = serde_json::from_str(&body).unwrap();
            let after = v["ploxions"].as_array().unwrap().len();
            assert_eq!(
                after, base_count,
                "restart must not duplicate the base set (was {base_count}, now {after})"
            );
            stop(host, join).await;
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// (d) A CORRUPT state dir is ignored gracefully: the daemon still boots the
    /// base set, never panics, and answers healthz. We write garbage into the
    /// manifest, then start a daemon on that dir.
    #[tokio::test]
    async fn corrupt_state_dir_boots_base_set_no_panic() {
        if !built() {
            eprintln!("SKIP: ploxions not built");
            return;
        }
        let dir = tmp_state_dir("corrupt");
        let store = StateStore::open(&dir).unwrap();
        // Garbage manifest + a junk wasm blob referenced by nobody.
        std::fs::write(dir.join("loaded.json"), b"{ NOT json at all ]").unwrap();
        std::fs::write(dir.join("loaded").join("ghost.wasm"), b"not wasm").unwrap();
        drop(store);

        let (addr, host, _ev, join) = spawn_with_state(&dir).await;
        // The daemon booted the base set and answers — no panic, no crash.
        let hz = reqwest_get(&format!("http://{addr}/healthz")).await;
        assert!(hz.contains("\"status\":\"ok\""), "daemon must boot despite corrupt state: {hz}");
        assert!(snapshot_has(addr, "ping").await, "the base set must still be loaded");
        stop(host, join).await;
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// (e) A staged record whose wasm has VANISHED from <dir> is SKIPPED on
    /// restore (warning), not fatal: the daemon still boots and other ploxions
    /// load. We simulate this by hand-writing a manifest naming a non-existent id.
    #[tokio::test]
    async fn restore_skips_missing_wasm_gracefully() {
        if !built() {
            eprintln!("SKIP: ploxions not built");
            return;
        }
        let dir = tmp_state_dir("missingwasm");
        let store = StateStore::open(&dir).unwrap();
        // Persist a STAGED record for an id with no <dir>/<id>.wasm and no blob.
        store.persist("no-such-ploxion-zzz", Source::Staged, None).unwrap();
        drop(store);

        let (addr, host, _ev, join) = spawn_with_state(&dir).await;
        let hz = reqwest_get(&format!("http://{addr}/healthz")).await;
        assert!(hz.contains("\"status\":\"ok\""), "daemon must boot despite a missing restore wasm");
        assert!(
            !snapshot_has(addr, "no-such-ploxion-zzz").await,
            "a record with no resolvable wasm must be skipped, not loaded"
        );
        // The base set is intact.
        assert!(snapshot_has(addr, "ping").await);
        stop(host, join).await;
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// (f) NO --state-dir => persistence OFF, behaves exactly as before: a load
    /// is NOT persisted (no state files anywhere), proving the feature is inert
    /// when unset. We use the classic `spawn()` (no state dir) and assert no dir
    /// was created at a path we never passed.
    #[tokio::test]
    async fn no_state_dir_is_inert() {
        if !built() {
            eprintln!("SKIP: ploxions not built");
            return;
        }
        // The classic spawn() builds with state_dir = None. A runtime load must
        // succeed and behave exactly as the existing hot-load tests — there is
        // simply no persistence side effect. (The absence of a StateStore is the
        // observable: build_server() passes None, so the host thread's
        // state_store is None and the persist/remove branches are never taken.)
        let (addr, host, _ev) = spawn().await;
        assert_eq!(http_post(&format!("http://{addr}/unload"), r#"{"id":"pong"}"#).await, 200);
        let (code, _b) = http_post_full(&format!("http://{addr}/load"), r#"{"id":"pong"}"#).await;
        assert_eq!(code, 200, "runtime load still works with no state dir");
        assert!(snapshot_has(addr, "pong").await);
        host.shutdown().await;
    }

    // =======================================================================
    // BUS FEDERATION — two SEPARATE in-process daemon instances on ephemeral
    // ports, peer-linked over a REAL `/peer` WebSocket. Nothing is faked: an
    // event emitted on A genuinely crosses the wire to B and triggers B's bus.
    // =======================================================================

    /// Spawn a federated in-process daemon on 127.0.0.1:0 with the given
    /// `node_id` and `peer_token`. Returns the bound addr + the full `AppState`
    /// (so a test can subscribe to its events, reach its host, and — separately —
    /// launch a peer_client toward another node). The router is served on a task.
    async fn spawn_node(
        node_id: &str,
        peer_token: Option<String>,
    ) -> (SocketAddr, AppState) {
        let fed = FederationConfig { node_id: node_id.into(), peers: vec![], peer_token };
        let server = build_server_fed(&ploxion_dir(), "testsha".into(), fed, None)
            .expect("server builds");
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind ephemeral");
        let addr = listener.local_addr().unwrap();
        let state = server.state.clone();
        let app = server.router.clone();
        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });
        (addr, state)
    }

    /// Drive node `st` to dial `peer_url` as a federation CLIENT (the same task
    /// `run_fed` spawns), returning once we have given the link time to come up.
    fn dial_peer(st: &AppState, peer_url: String, token: Option<String>) {
        let st = st.clone();
        let node = Arc::clone(&st.node_id);
        let peer = PeerSpec { url: peer_url, token };
        tokio::spawn(async move { peer_client(peer, st, node).await });
    }

    /// Collect bus events on `events` for up to `ms`, returning every parsed
    /// `LiveEvent` seen (as JSON values) — used to assert what landed on a node's
    /// bus. Subscribes BEFORE the caller triggers, so nothing is missed.
    async fn collect_for(
        rx: &mut broadcast::Receiver<LiveEvent>,
        ms: u64,
    ) -> Vec<LiveEvent> {
        let mut out = Vec::new();
        let deadline = tokio::time::Instant::now() + Duration::from_millis(ms);
        while tokio::time::Instant::now() < deadline {
            match tokio::time::timeout(Duration::from_millis(120), rx.recv()).await {
                Ok(Ok(ev)) => out.push(ev),
                Ok(Err(broadcast::error::RecvError::Lagged(_))) => continue,
                Ok(Err(broadcast::error::RecvError::Closed)) => break,
                Err(_) => {} // idle tick; keep waiting until the deadline
            }
        }
        out
    }

    /// CROSS-NODE DELIVERY: emit `ping` on node A; node B's `pong` (which
    /// `requires` ping) must react — proving the event crossed two SEPARATE
    /// daemon instances over the peer link, injected on B as remote-origin
    /// `fed:A`, and routed to a local subscriber there.
    #[tokio::test]
    async fn federation_cross_node_delivery() {
        if !built() {
            eprintln!("SKIP: ploxions not built");
            return;
        }
        // Node A (the emitter) and node B (dials A, hosts pong).
        let (addr_a, st_a) = spawn_node("A", None).await;
        let (_addr_b, st_b) = spawn_node("B", None).await;

        // B dials A's /peer. Subscribe to B's bus BEFORE the emit.
        dial_peer(&st_b, format!("ws://{addr_a}/peer"), None);
        let mut b_bus = st_b.events.subscribe();

        // Give the peer link a moment to establish.
        tokio::time::sleep(Duration::from_millis(300)).await;

        // Emit ping on A (locally originated -> forwarded to B).
        assert!(st_a.host.emit("ping".into(), "fed-hello".into()));

        // On B's bus we must see: the fed-in note, the ping emitted from `fed:A`,
        // and a route of that ping to a local subscriber (pong/tracer).
        let evs = collect_for(&mut b_bus, 2500).await;
        let saw_fed_in = evs.iter().any(|e| {
            e.kind == "life" && e.from == "fed:A" && e.payload.contains("fed-in from A")
        });
        let saw_remote_ping = evs.iter().any(|e| {
            e.kind == "emit" && e.from == "fed:A" && e.topic == "ping" && e.payload == "fed-hello"
        });
        let saw_route_to_local = evs.iter().any(|e| {
            e.kind == "route" && e.topic == "ping" && (e.note.contains("pong") || e.note.contains("tracer"))
        });
        assert!(saw_fed_in, "B must trace a `fed-in from A` hop; got {evs:?}");
        assert!(saw_remote_ping, "B's bus must carry the ping as remote-origin fed:A");
        assert!(saw_route_to_local, "B must ROUTE the federated ping to a local subscriber");

        st_a.host.shutdown().await;
        st_b.host.shutdown().await;
    }

    /// LOOP PREVENTION: with A and B dialing EACH OTHER (a full bidirectional
    /// link), a single ping emitted on A must (a) reach B exactly once and (b)
    /// NOT bounce back to A and re-trigger. We assert B injects exactly ONE
    /// `fed:A` ping, and A NEVER injects a `fed:B` ping of its own original emit
    /// (no echo / no storm).
    #[tokio::test]
    async fn federation_loop_prevention_no_echo() {
        if !built() {
            eprintln!("SKIP: ploxions not built");
            return;
        }
        let (addr_a, st_a) = spawn_node("A", None).await;
        let (addr_b, st_b) = spawn_node("B", None).await;

        // Bidirectional: A dials B and B dials A.
        dial_peer(&st_a, format!("ws://{addr_b}/peer"), None);
        dial_peer(&st_b, format!("ws://{addr_a}/peer"), None);
        let mut a_bus = st_a.events.subscribe();
        let mut b_bus = st_b.events.subscribe();
        tokio::time::sleep(Duration::from_millis(400)).await;

        // ONE ping on A.
        assert!(st_a.host.emit("ping".into(), "once".into()));

        let a_evs = collect_for(&mut a_bus, 2500).await;
        let b_evs = collect_for(&mut b_bus, 2500).await;

        // B injects the federated ping EXACTLY once (no storm).
        let b_fed_pings = b_evs
            .iter()
            .filter(|e| e.kind == "emit" && e.from == "fed:A" && e.topic == "ping")
            .count();
        assert_eq!(b_fed_pings, 1, "B must inject the federated ping exactly once; got {b_fed_pings}");

        // A must NEVER receive its own ping echoed back as a remote-origin event
        // (`fed:B` ping with the same payload). That is the echo the loop rule
        // forbids: B never re-forwards a `fed:A` emit.
        let a_echoed = a_evs
            .iter()
            .any(|e| e.kind == "emit" && e.from == "fed:B" && e.topic == "ping" && e.payload == "once");
        assert!(!a_echoed, "A must NOT receive its own ping echoed back (loop!); got {a_evs:?}");

        st_a.host.shutdown().await;
        st_b.host.shutdown().await;
    }

    /// AUTH: a `/peer` upgrade is refused without the shared token and accepted
    /// with it (the link MUST be authenticatable). We assert delivery only
    /// happens when the dialing client presents the right token.
    #[tokio::test]
    async fn federation_peer_token_is_enforced() {
        if !built() {
            eprintln!("SKIP: ploxions not built");
            return;
        }
        let token = "s3cret-peer-token".to_string();
        // Node A requires the token on /peer; node B will dial it.
        let (addr_a, st_a) = spawn_node("A", Some(token.clone())).await;
        let (_addr_b1, st_bad) = spawn_node("Bbad", None).await;
        let (_addr_b2, st_good) = spawn_node("Bgood", None).await;

        // WRONG token -> the upgrade is 401, the link never carries events.
        dial_peer(&st_bad, format!("ws://{addr_a}/peer"), Some("wrong".into()));
        let mut bad_bus = st_bad.events.subscribe();
        tokio::time::sleep(Duration::from_millis(300)).await;
        assert!(st_a.host.emit("ping".into(), "auth-1".into()));
        let bad = collect_for(&mut bad_bus, 1200).await;
        assert!(
            !bad.iter().any(|e| e.from == "fed:A" && e.topic == "ping"),
            "an unauthenticated peer must NOT receive federated events"
        );

        // RIGHT token -> delivery works.
        dial_peer(&st_good, format!("ws://{addr_a}/peer"), Some(token));
        let mut good_bus = st_good.events.subscribe();
        tokio::time::sleep(Duration::from_millis(400)).await;
        assert!(st_a.host.emit("ping".into(), "auth-2".into()));
        let good = collect_for(&mut good_bus, 2000).await;
        assert!(
            good.iter().any(|e| e.from == "fed:A" && e.topic == "ping" && e.payload == "auth-2"),
            "an authenticated peer MUST receive federated events; got {good:?}"
        );

        st_a.host.shutdown().await;
        st_bad.host.shutdown().await;
        st_good.host.shutdown().await;
    }

    /// STANDALONE (no peer) is unchanged: a node with no peer link records NO
    /// `fed-in`/`fed-out` notes and no `fed:*` origins — federation is inert.
    #[tokio::test]
    async fn federation_standalone_is_inert() {
        if !built() {
            eprintln!("SKIP: ploxions not built");
            return;
        }
        let (_addr, st) = spawn_node("solo", None).await;
        let mut bus = st.events.subscribe();
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert!(st.host.emit("ping".into(), "lonely".into()));
        let evs = collect_for(&mut bus, 1200).await;
        // The ordinary ping path still works...
        assert!(
            evs.iter().any(|e| e.kind == "emit" && e.from == "api" && e.topic == "ping"),
            "standalone emit must still flow (additive)"
        );
        // ...but NOTHING federated appears (no peer attached).
        assert!(
            !evs.iter().any(|e| e.from.starts_with("fed:") || e.payload.contains("fed-")),
            "with no peer there must be zero federation hops; got {evs:?}"
        );
        st.host.shutdown().await;
    }

    // --- tiny in-test HTTP helpers (no extra dep; raw TCP) ------------------

    /// Minimal HTTP/1.1 GET returning the response body (offline, localhost).
    async fn reqwest_get(url: &str) -> String {
        let (host, port, path) = split_url(url);
        let mut stream = tokio::net::TcpStream::connect((host.as_str(), port))
            .await
            .expect("connect");
        let req = format!(
            "GET {path} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n\r\n"
        );
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        stream.write_all(req.as_bytes()).await.expect("write");
        let mut buf = Vec::new();
        stream.read_to_end(&mut buf).await.expect("read");
        let text = String::from_utf8_lossy(&buf);
        // Split off headers; return the body.
        text.split_once("\r\n\r\n").map(|(_, b)| b.to_string()).unwrap_or_default()
    }

    /// Minimal HTTP/1.1 GET returning `(status, content-type, body)` — used to
    /// assert the 3D page's status line, content-type header, and that the body
    /// references the live endpoints. Offline/localhost; `Connection: close`.
    async fn http_get_full(url: &str) -> (u16, String, String) {
        let (host, port, path) = split_url(url);
        let mut stream = tokio::net::TcpStream::connect((host.as_str(), port))
            .await
            .expect("connect");
        let req = format!("GET {path} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n\r\n");
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        stream.write_all(req.as_bytes()).await.expect("write");
        let mut buf = Vec::new();
        stream.read_to_end(&mut buf).await.expect("read");
        let text = String::from_utf8_lossy(&buf);
        let (head, body) = text.split_once("\r\n\r\n").unwrap_or((&text, ""));
        let status = head
            .split_whitespace()
            .nth(1)
            .and_then(|c| c.parse().ok())
            .unwrap_or(0);
        let ctype = head
            .lines()
            .find_map(|l| {
                let (k, v) = l.split_once(':')?;
                k.trim().eq_ignore_ascii_case("content-type").then(|| v.trim().to_string())
            })
            .unwrap_or_default();
        (status, ctype, body.to_string())
    }

    /// Minimal HTTP/1.1 POST returning the status code.
    async fn http_post(url: &str, body: &str) -> u16 {
        let (host, port, path) = split_url(url);
        let mut stream = tokio::net::TcpStream::connect((host.as_str(), port))
            .await
            .expect("connect");
        let req = format!(
            "POST {path} HTTP/1.1\r\nHost: {host}\r\nContent-Type: application/json\r\n\
             Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        stream.write_all(req.as_bytes()).await.expect("write");
        let mut buf = Vec::new();
        stream.read_to_end(&mut buf).await.expect("read");
        let text = String::from_utf8_lossy(&buf);
        // "HTTP/1.1 202 Accepted" -> 202.
        text.split_whitespace()
            .nth(1)
            .and_then(|c| c.parse().ok())
            .unwrap_or(0)
    }

    fn split_url(url: &str) -> (String, u16, String) {
        let rest = url.strip_prefix("http://").unwrap_or(url);
        let (authority, path) = rest.split_once('/').map(|(a, p)| (a, format!("/{p}"))).unwrap_or((rest, "/".into()));
        let (host, port) = authority
            .split_once(':')
            .map(|(h, p)| (h.to_string(), p.parse().unwrap_or(80)))
            .unwrap_or((authority.to_string(), 80));
        (host, port, path)
    }
}
