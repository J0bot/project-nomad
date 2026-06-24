//! # xerboxion-host — the operational xerboxion-core
//!
//! A [`wasmtime`]-based host that loads N `.wasm` ploxion modules, isolates each
//! in its **own** `Store`/`Instance`, reads its [`Manifest`], runs its lifecycle
//! ([`plc_init`] / health sweep / [`plc_goodbye`]), and wires them together over
//! the **XERB0XI0N bus**.
//!
//! The bus ROUTES and TRACES but never FILTERS: when a ploxion calls the
//! imported `plc_emit(topic, payload)`, the host delivers that event to every
//! *other* ploxion subscribed to `topic` by invoking its `plc_on_event`, and
//! appends the hop to a journal.
//!
//! ## Isolation
//! Each ploxion gets a separate `Store<PloxionState>`. WASM linear memory is
//! per-instance, so one ploxion cannot read another's memory — the host never
//! shares a memory or a `Store` between ploxions.
//!
//! ## Re-entrancy
//! `plc_emit` is called *inside* a running ploxion (inside its `Store`). We
//! cannot re-enter another `Store` from there safely, so emits are **queued**
//! into the calling ploxion's [`PloxionState::outbox`]; the host drains the
//! outbox after the call returns and dispatches each event. Dispatch may cause
//! more emits, which queue again — the host loops until the bus is quiescent.

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use wasmtime::{Caller, Engine, Extern, Instance, Linker, Memory, Module, Store};
use xerboxion_plc::{exports, imports, Manifest};

pub mod connector;
pub mod ecosystem;
pub mod federation;
pub mod fetch;
pub mod map;
pub mod mock_osiris;
pub mod recipe;
pub mod recorder;
pub mod registry;
pub mod replicate;
pub mod serve;
pub mod state;
pub use connector::{HealthCheck, ServiceConnector, SweepRow};
pub use ecosystem::Ecosystem;
pub use fetch::{http_fetch, FetchLimits, FetchResult};
pub use recipe::{CompiledRecipe, Recipe};
pub use recorder::{record_trace, replay, BusTsoin, RecordedEvent, TsoinStats};
pub use registry::RegistryEntry;

/// The host-side `plc_fetch` engine a ploxion's brokered fetch is dispatched
/// through. Signature mirrors [`fetch::http_fetch`]: `(method, url, body,
/// limits) -> FetchResult`. Injected into the [`Host`] so tests can substitute a
/// deterministic offline stub (the live network belongs only in the demo, never
/// in `cargo test`) — exactly how [`HealthCheck`] keeps the connector tests
/// offline.
pub type FetchFn = dyn Fn(&str, &str, &[u8], FetchLimits) -> FetchResult + Send + Sync;

/// One event in flight on the bus: who emitted it, the topic, the payload.
#[derive(Debug, Clone)]
pub struct Event {
    /// Ploxion id of the emitter.
    pub from: String,
    /// Bus topic.
    pub topic: String,
    /// Opaque payload bytes (the host never inspects these).
    pub payload: Vec<u8>,
}

/// A single traced hop in the journal: an emit, or a delivery to a subscriber.
#[derive(Debug, Clone)]
pub enum Trace {
    /// A ploxion emitted on a topic.
    Emit { from: String, topic: String, payload: String },
    /// The bus delivered an event to a subscriber.
    Deliver { from: String, to: String, topic: String },
    /// A ploxion wrote a log line via `plc_log`.
    Log { from: String, line: String },
    /// A lifecycle milestone (init, goodbye, health).
    Lifecycle { who: String, what: String },
    /// A ploxion performed a **capability-gated** brokered HTTP fetch via the
    /// host import `plc_fetch` (PLC v1.1). Traced like an emit — the host did
    /// real I/O on the sandbox's behalf and recorded the hop.
    Fetch { from: String, method: String, url: String, status: u16 },
    /// A native **recipe** participant made a reactive decision: a trigger
    /// received, a condition evaluated true/false, an emit fired, or the rule
    /// stayed silent. `who` is the recipe's bus id (`recipe:<name>`); `step` is
    /// the one-line rendering of the step. The bus routes the recipe's emits
    /// through the very same cascade as any ploxion — these hops just make the
    /// rule's *decision* visible alongside the routing.
    Recipe { who: String, step: String },
}

impl Trace {
    /// One-line human rendering for the trace journal.
    pub fn render(&self) -> String {
        match self {
            Trace::Emit { from, topic, payload } => {
                format!("  emit   {from} -> [{topic}] {payload}")
            }
            Trace::Deliver { from, to, topic } => {
                format!("  route  [{topic}] {from} => {to}")
            }
            Trace::Log { from, line } => format!("  log    {from}: {line}"),
            Trace::Lifecycle { who, what } => format!("  life   {who}: {what}"),
            Trace::Fetch { from, method, url, status } => {
                format!("  fetch  {from} -> {method} {url} => {status}")
            }
            Trace::Recipe { who, step } => format!("  recipe {who}: {step}"),
        }
    }
}

/// Per-`Store` state the host attaches to each ploxion. Holds the ploxion's id
/// (for tracing), the outbox of pending emits, and a sink for log lines. This
/// is the ONLY thing the import functions can touch; it is private to one
/// ploxion's `Store`, never shared.
pub struct PloxionState {
    id: String,
    /// Emits this ploxion produced during the current call, awaiting dispatch.
    outbox: VecDeque<Event>,
    /// Log lines this ploxion produced during the current call.
    logs: VecDeque<String>,
    /// Brokered `plc_fetch` calls this ploxion made during the current call,
    /// awaiting trace (method, url, resulting status). Recorded inside the gated
    /// import body; flushed into the journal right after the call, in order, so
    /// a fetch precedes the emit it triggers.
    fetches: VecDeque<(String, String, u16)>,
}

impl PloxionState {
    fn new(id: String) -> Self {
        PloxionState {
            id,
            outbox: VecDeque::new(),
            logs: VecDeque::new(),
            fetches: VecDeque::new(),
        }
    }
}

/// A loaded, isolated ploxion: its own `Store` + `Instance` + declared manifest.
pub struct Ploxion {
    pub manifest: Manifest,
    pub path: PathBuf,
    store: Store<PloxionState>,
    instance: Instance,
    /// Set once `plc_init` has run, so the host can assert init-once.
    initialized: bool,
    /// Set once `plc_goodbye` has run, so the host won't double-shutdown.
    saidgoodbye: bool,
}

impl Ploxion {
    /// The ploxion's stable id (from its manifest).
    pub fn id(&self) -> &str {
        &self.manifest.id
    }

    // --- low-level memory helpers (operate on THIS ploxion's linear memory) --

    fn memory(&mut self) -> Result<Memory> {
        match self.instance.get_export(&mut self.store, "memory") {
            Some(Extern::Memory(m)) => Ok(m),
            _ => Err(anyhow!("ploxion '{}' exports no linear memory", self.manifest.id)),
        }
    }

    /// Copy `bytes` into the ploxion's memory using its `alloc` export, return
    /// the `(ptr, len)` the host can pass into a call.
    fn write_bytes(&mut self, bytes: &[u8]) -> Result<(u32, u32)> {
        let alloc = self
            .instance
            .get_typed_func::<i32, i32>(&mut self.store, exports::ALLOC)
            .with_context(|| format!("ploxion '{}' missing export `{}`", self.manifest.id, exports::ALLOC))?;
        let ptr = alloc.call(&mut self.store, bytes.len() as i32)? as u32;
        let mem = self.memory()?;
        mem.write(&mut self.store, ptr as usize, bytes)
            .with_context(|| format!("writing into ploxion '{}' memory", self.manifest.id))?;
        Ok((ptr, bytes.len() as u32))
    }

    /// Read `len` bytes at `ptr` out of the ploxion's memory.
    fn read_bytes(&mut self, ptr: u32, len: u32) -> Result<Vec<u8>> {
        let mem = self.memory()?;
        let mut buf = vec![0u8; len as usize];
        mem.read(&self.store, ptr as usize, &mut buf)
            .with_context(|| format!("reading from ploxion '{}' memory", self.manifest.id))?;
        Ok(buf)
    }

    // --- PLC export invocations ---------------------------------------------

    /// Call `plc_init` exactly once.
    fn call_init(&mut self) -> Result<()> {
        if self.initialized {
            return Err(anyhow!("plc_init called twice on '{}'", self.manifest.id));
        }
        let f = self
            .instance
            .get_typed_func::<(), ()>(&mut self.store, exports::INIT)
            .with_context(|| format!("ploxion '{}' missing `{}`", self.manifest.id, exports::INIT))?;
        f.call(&mut self.store, ())?;
        self.initialized = true;
        Ok(())
    }

    /// Call `plc_health`, returning the i32 status (0 = ok).
    fn call_health(&mut self) -> Result<i32> {
        let f = self
            .instance
            .get_typed_func::<(), i32>(&mut self.store, exports::HEALTH)
            .with_context(|| format!("ploxion '{}' missing `{}`", self.manifest.id, exports::HEALTH))?;
        f.call(&mut self.store, ())
    }

    /// Deliver an event by copying topic+payload into memory and calling
    /// `plc_on_event`. Returns whether the export existed.
    fn call_on_event(&mut self, topic: &str, payload: &[u8]) -> Result<()> {
        let (tptr, tlen) = self.write_bytes(topic.as_bytes())?;
        let (pptr, plen) = self.write_bytes(payload)?;
        let f = self
            .instance
            .get_typed_func::<(i32, i32, i32, i32), ()>(&mut self.store, exports::ON_EVENT)
            .with_context(|| format!("ploxion '{}' missing `{}`", self.manifest.id, exports::ON_EVENT))?;
        f.call(&mut self.store, (tptr as i32, tlen as i32, pptr as i32, plen as i32))?;
        Ok(())
    }

    /// Call `plc_goodbye` (idempotent at host level).
    fn call_goodbye(&mut self) -> Result<()> {
        if self.saidgoodbye {
            return Ok(());
        }
        let f = self
            .instance
            .get_typed_func::<(), ()>(&mut self.store, exports::GOODBYE)
            .with_context(|| format!("ploxion '{}' missing `{}`", self.manifest.id, exports::GOODBYE))?;
        f.call(&mut self.store, ())?;
        self.saidgoodbye = true;
        Ok(())
    }

    /// Drain whatever this ploxion queued (fetches + logs + emits) during the
    /// last call. Returned in the causal order the host journals them: a fetch
    /// (host did I/O), then logs, then the emits they produced.
    #[allow(clippy::type_complexity)]
    fn drain_outbox(&mut self) -> (Vec<Event>, Vec<String>, Vec<(String, String, u16)>) {
        let st = self.store.data_mut();
        let events: Vec<Event> = st.outbox.drain(..).collect();
        let logs: Vec<String> = st.logs.drain(..).collect();
        let fetches: Vec<(String, String, u16)> = st.fetches.drain(..).collect();
        (events, logs, fetches)
    }
}

/// The bus wiring: topic -> set of subscriber ploxion ids.
#[derive(Debug, Default, Clone)]
pub struct Bus {
    subs: HashMap<String, Vec<String>>,
}

impl Bus {
    /// Subscribe a ploxion id to a topic (idempotent).
    pub fn subscribe(&mut self, topic: &str, who: &str) {
        let v = self.subs.entry(topic.to_string()).or_default();
        if !v.iter().any(|x| x == who) {
            v.push(who.to_string());
        }
    }

    /// Subscribers of a topic (empty slice if none).
    pub fn subscribers(&self, topic: &str) -> &[String] {
        self.subs.get(topic).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// All (topic, subscribers) pairs, for `ls`/status rendering.
    pub fn wiring(&self) -> impl Iterator<Item = (&String, &Vec<String>)> {
        self.subs.iter()
    }

    /// Remove a subscriber id from EVERY topic it is wired to (the inverse of the
    /// per-topic [`Bus::subscribe`] calls a ploxion's `requires` produce). Used by
    /// runtime UNLOAD so a dropped ploxion receives nothing afterwards. Any topic
    /// left with no subscribers is pruned so the wiring stays minimal/consistent.
    pub fn unsubscribe_all(&mut self, who: &str) {
        for subs in self.subs.values_mut() {
            subs.retain(|x| x != who);
        }
        self.subs.retain(|_, subs| !subs.is_empty());
    }
}

/// The host: owns the engine, the loaded ploxions, the bus, and the trace.
pub struct Host {
    engine: Engine,
    ploxions: Vec<Ploxion>,
    bus: Bus,
    trace: Vec<Trace>,
    /// Native (non-WASM) participants and the topics they provide. These are
    /// host-side adapters/connectors (real I/O bridged to the bus) — e.g. the
    /// [`ServiceConnector`] under id `"service-connector"`. Tracked separately
    /// from WASM ploxions so the wiring display can attribute a topic's provider
    /// to a native source. Ordered (`BTreeMap`) for deterministic rendering.
    native_participants: BTreeMap<String, Vec<String>>,
    /// Compiled designer recipes registered as native, reactive bus participants.
    /// Each is a host-side rule: it `requires` its trigger topics and `provides`
    /// its action/sortie topics (registered in `native_participants`), and the
    /// cascade dispatches a matching event into [`CompiledRecipe::react`] exactly
    /// as it delivers to a WASM subscriber — closing the designer→host→bus
    /// triangle. They never touch a `Store`: a recipe is pure host logic.
    recipes: Vec<CompiledRecipe>,
    /// The engine behind the **capability-gated** `plc_fetch` import (PLC v1.1).
    /// Defaults to the real network ([`fetch::http_fetch`]); tests inject an
    /// offline stub via [`Host::with_fetch_fn`]. Wrapped in an `Arc` so each
    /// gated ploxion's import closure can share it.
    fetch_fn: Arc<FetchFn>,
    /// Limits the host applies to every brokered fetch (timeout, max body).
    fetch_limits: FetchLimits,
}

impl Default for Host {
    fn default() -> Self {
        Self::new()
    }
}

impl Host {
    /// Create an empty host. The gated `plc_fetch` (PLC v1.1) defaults to the
    /// REAL network ([`fetch::http_fetch`]); use [`Host::with_fetch_fn`] to
    /// substitute a deterministic offline stub in tests.
    pub fn new() -> Self {
        Host {
            engine: Engine::default(),
            ploxions: Vec::new(),
            bus: Bus::default(),
            trace: Vec::new(),
            native_participants: BTreeMap::new(),
            recipes: Vec::new(),
            fetch_fn: Arc::new(|method, url, body, limits| {
                fetch::http_fetch(method, url, body, limits)
            }),
            fetch_limits: FetchLimits::default(),
        }
    }

    /// Replace the `plc_fetch` engine — for OFFLINE/deterministic tests. The
    /// stub receives `(method, url, body, limits)` and returns a
    /// [`FetchResult`], exactly like the real [`fetch::http_fetch`], so no
    /// ploxion can tell the difference. Must be set before any capability-gated
    /// ploxion is loaded (the closure is captured at link time).
    pub fn with_fetch_fn(
        mut self,
        f: impl Fn(&str, &str, &[u8], FetchLimits) -> FetchResult + Send + Sync + 'static,
    ) -> Self {
        self.fetch_fn = Arc::new(f);
        self
    }

    /// Override the brokered-fetch limits (timeout / max body).
    pub fn with_fetch_limits(mut self, limits: FetchLimits) -> Self {
        self.fetch_limits = limits;
        self
    }

    /// Loaded ploxions (read-only).
    pub fn ploxions(&self) -> &[Ploxion] {
        &self.ploxions
    }

    /// The bus wiring (read-only).
    pub fn bus(&self) -> &Bus {
        &self.bus
    }

    /// The trace journal so far.
    pub fn trace(&self) -> &[Trace] {
        &self.trace
    }

    /// Append a host-level annotation hop to the trace journal as a
    /// [`Trace::Lifecycle`]. This is the seam BUS FEDERATION uses to make a
    /// cross-node hop VISIBLE in the snapshot/trace (`fed-out to <node> [topic]`
    /// / `fed-in from <node> [topic]`) without inventing a new `Trace` variant.
    /// Purely additive: nothing in the host calls it unless federation is on.
    pub fn note(&mut self, who: &str, what: &str) {
        self.trace.push(Trace::Lifecycle {
            who: who.to_string(),
            what: what.to_string(),
        });
    }

    /// The native (non-WASM) participants and the topics they provide
    /// (read-only). These are host-side adapters/connectors bridging real I/O
    /// to the bus — they have no `Store`, no sandbox; they are trusted host code.
    pub fn native_participants(&self) -> impl Iterator<Item = (&String, &Vec<String>)> {
        self.native_participants.iter()
    }

    /// Register a native (host-side) participant on the bus under `id`,
    /// declaring the topics it `provides`. Unlike a WASM ploxion it is not
    /// sandboxed and not health-swept — it is the host doing real I/O on the
    /// bus's behalf. Idempotent.
    pub fn register_native(&mut self, id: &str, provides: &[&str]) {
        let entry = self.native_participants.entry(id.to_string()).or_default();
        for t in provides {
            if !entry.iter().any(|x| x == t) {
                entry.push((*t).to_string());
            }
        }
    }

    /// Compile a designer [`Recipe`] and **register it as a native, reactive bus
    /// participant** — the same mechanism as the service connector, but instead
    /// of doing I/O it executes a rule. This is the host EXECUTING a recipe: the
    /// designer produces it, the host wires it onto the bus, the bus connects the
    /// ploxions.
    ///
    /// Wiring (provides/requires, mirroring a ploxion's manifest):
    /// - `requires` = the `quand` trigger topics → the rule is **subscribed** to
    ///   them, so the cascade routes those events into it.
    /// - `provides` = the `action`/`sortie` topics → its **consent surface**,
    ///   registered as a native participant so the wiring display attributes the
    ///   topic to the recipe.
    ///
    /// At runtime, when a trigger event reaches the rule the host evaluates its
    /// conditions and — only if they all hold — queues its emits onto the bus.
    /// The rule NEVER fires on a non-trigger topic or a false condition (*droit
    /// au silence*). Returns the compiled rule (a clone) so callers can show what
    /// was wired.
    pub fn register_recipe(&mut self, recipe: &Recipe) -> CompiledRecipe {
        let compiled = CompiledRecipe::compile(recipe);
        // requires: subscribe the rule to each trigger topic.
        for topic in compiled.requires() {
            self.bus.subscribe(topic, &compiled.id);
        }
        // provides: announce the rule as a native provider of its emit topics.
        let provides = compiled.provides();
        if !provides.is_empty() {
            self.register_native(&compiled.id, &provides);
        }
        self.recipes.push(compiled.clone());
        compiled
    }

    /// The compiled recipes registered as native participants (read-only).
    pub fn recipes(&self) -> &[CompiledRecipe] {
        &self.recipes
    }

    /// Build the `Linker` providing the host imports for a single ploxion's
    /// `Store`, **gated by the ploxion's declared capabilities** (PLC v1.1).
    ///
    /// The ungated imports (`plc_emit`, `plc_log`) are ALWAYS linked — every
    /// ploxion has them. The gated import `plc_fetch` is linked **only if**
    /// `caps` contains [`capabilities::NET_FETCH`] — least authority, consent by
    /// manifest. A ploxion that does not declare the capability gets NO
    /// `plc_fetch` symbol in its import namespace, so a wasm that references it
    /// fails to instantiate (an unresolved import) — the gate is load-bearing.
    fn make_linker_for(&self, caps: &[String]) -> Result<Linker<PloxionState>> {
        let mut linker: Linker<PloxionState> = Linker::new(&self.engine);

        // plc_emit(topic_ptr, topic_len, payload_ptr, payload_len)
        linker.func_wrap(
            imports::MODULE,
            imports::EMIT,
            |mut caller: Caller<'_, PloxionState>,
             tptr: i32,
             tlen: i32,
             pptr: i32,
             plen: i32| {
                let mem = match caller.get_export("memory") {
                    Some(Extern::Memory(m)) => m,
                    _ => return, // no memory => nothing we can read; drop quietly
                };
                let topic = read_str(&caller, &mem, tptr, tlen);
                let payload = read_vec(&caller, &mem, pptr, plen);
                let st = caller.data_mut();
                st.outbox.push_back(Event {
                    from: st.id.clone(),
                    topic,
                    payload,
                });
            },
        )?;

        // plc_log(ptr, len)
        linker.func_wrap(
            imports::MODULE,
            imports::LOG,
            |mut caller: Caller<'_, PloxionState>, ptr: i32, len: i32| {
                let mem = match caller.get_export("memory") {
                    Some(Extern::Memory(m)) => m,
                    _ => return,
                };
                let line = read_str(&caller, &mem, ptr, len);
                let st = caller.data_mut();
                st.logs.push_back(line);
            },
        )?;

        // plc_fetch — GATED. Linked ONLY when the manifest declared net.fetch.
        // A ploxion without the capability never gets this symbol, so if its
        // wasm imports `xerboxion.plc_fetch` it cannot instantiate.
        if caps.iter().any(|c| c == xerboxion_plc::capabilities::NET_FETCH) {
            let fetch_fn = Arc::clone(&self.fetch_fn);
            let limits = self.fetch_limits;
            // plc_fetch(method_ptr,method_len, url_ptr,url_len, body_ptr,body_len) -> i64 packed(ptr,len)
            linker.func_wrap(
                imports::MODULE,
                imports::FETCH,
                move |mut caller: Caller<'_, PloxionState>,
                      mptr: i32,
                      mlen: i32,
                      uptr: i32,
                      ulen: i32,
                      bptr: i32,
                      blen: i32|
                      -> i64 {
                    let mem = match caller.get_export("memory") {
                        Some(Extern::Memory(m)) => m,
                        _ => return 0, // no memory => can't return anything
                    };
                    let method = read_str(&caller, &mem, mptr, mlen);
                    let url = read_str(&caller, &mem, uptr, ulen);
                    let body = read_vec(&caller, &mem, bptr, blen);

                    // The host does the real (or stubbed) I/O. NEVER panics:
                    // unreachable => status 0 inside the FetchResult.
                    let result = fetch_fn(&method, &url, &body, limits);
                    let status = result.status;
                    let json = result.to_json();

                    // Record for the trace journal (flushed after the call).
                    {
                        let st = caller.data_mut();
                        st.fetches.push_back((method.clone(), url.clone(), status));
                    }

                    // Allocate space in the ploxion's OWN memory and write the
                    // JSON result there, returning packed (ptr,len). Re-entrant
                    // alloc into the same Store is fine here.
                    match write_into_caller(&mut caller, json.as_bytes()) {
                        Some((ptr, len)) => xerboxion_plc::pack_ptr_len(ptr, len),
                        None => 0,
                    }
                },
            )?;
        }

        Ok(linker)
    }

    /// Load a ploxion from raw wasm bytes with an explicit fallback id (used if
    /// the manifest cannot be read, e.g. when rejecting a bad module). The
    /// module is validated against the PLC required exports BEFORE init.
    pub fn load_bytes(&mut self, wasm: &[u8], fallback_id: &str) -> Result<()> {
        let ploxion = self.build_ploxion(wasm, fallback_id)?;
        self.register(ploxion);
        Ok(())
    }

    /// Compile + validate + capability-gate + instantiate a ploxion from wasm
    /// bytes, returning the fully-built (but NOT yet registered, NOT yet
    /// initialized) [`Ploxion`]. This is the shared core of [`Host::load_bytes`]
    /// (boot-time) and [`Host::load_runtime`] (hot-load into a running daemon): a
    /// bad module is rejected here with a CLEAR error and NO panic, before any bus
    /// wiring or `plc_init` happens, so a failed load leaves the host untouched.
    fn build_ploxion(&mut self, wasm: &[u8], fallback_id: &str) -> Result<Ploxion> {
        let module = Module::new(&self.engine, wasm)
            .with_context(|| format!("compiling wasm for '{fallback_id}'"))?;

        // Validate required exports up front so a bad module is rejected
        // cleanly (no panic, no half-loaded ploxion).
        let exported: std::collections::HashSet<String> =
            module.exports().map(|e| e.name().to_string()).collect();
        for req in exports::REQUIRED {
            if !exported.contains(*req) {
                return Err(anyhow!(
                    "ploxion '{fallback_id}' rejected: missing required export `{req}`"
                ));
            }
        }
        if !exported.contains("memory") {
            return Err(anyhow!(
                "ploxion '{fallback_id}' rejected: missing exported linear `memory`"
            ));
        }

        // What gated host imports does this wasm actually reference? (Static —
        // read off the module, before we instantiate.) Used to enforce the
        // capability gate with a CLEAR error: a module that imports
        // `xerboxion.plc_fetch` but does not declare `net.fetch` is rejected
        // here, rather than producing an opaque "unknown import" later.
        let imports_fetch = module.imports().any(|i| {
            i.module() == imports::MODULE && i.name() == imports::FETCH
        });

        // --- Phase 1: PROBE the manifest. -----------------------------------
        // We need the manifest (which declares capabilities) BEFORE we can build
        // the gated linker. Instantiate a throwaway probe with a permissive
        // linker that provides every host import (incl. a never-called plc_fetch
        // stub) so the manifest read succeeds regardless of what the module
        // imports. The probe instance is dropped immediately after.
        let manifest = {
            let mut probe_store =
                Store::new(&self.engine, PloxionState::new(fallback_id.to_string()));
            let probe_linker = self.make_linker_for(&[
                xerboxion_plc::capabilities::NET_FETCH.to_string(),
            ])?;
            let probe_instance = probe_linker
                .instantiate(&mut probe_store, &module)
                .with_context(|| format!("probing manifest of '{fallback_id}'"))?;
            let mut probe = Ploxion {
                manifest: Manifest::new(fallback_id, "0.0.0", vec![], vec![]),
                path: PathBuf::from(fallback_id),
                store: probe_store,
                instance: probe_instance,
                initialized: false,
                saidgoodbye: false,
            };
            read_manifest(&mut probe)?
            // probe Store dropped here — nothing of it survives.
        };

        // --- Capability validation (fail closed). ---------------------------
        if let Some(unknown) = manifest.unknown_capability() {
            return Err(anyhow!(
                "ploxion '{}' rejected: declares unknown capability `{unknown}` (known: {:?})",
                manifest.id,
                xerboxion_plc::capabilities::KNOWN
            ));
        }
        // The gate, made explicit + load-bearing: a wasm that imports plc_fetch
        // MUST declare net.fetch. (If it does declare it, the gated linker below
        // links plc_fetch; if it imports it without declaring it, reject now.)
        if imports_fetch && !manifest.has_capability(xerboxion_plc::capabilities::NET_FETCH) {
            return Err(anyhow!(
                "ploxion '{}' rejected: imports `{}.{}` without declaring capability `{}` \
                 (least authority — declare it in the manifest to be granted the power)",
                manifest.id,
                imports::MODULE,
                imports::FETCH,
                xerboxion_plc::capabilities::NET_FETCH
            ));
        }

        // --- Phase 2: build the REAL, capability-GATED instance. ------------
        // The linker links plc_fetch ONLY if the manifest declared net.fetch.
        let mut store = Store::new(&self.engine, PloxionState::new(manifest.id.clone()));
        let linker = self.make_linker_for(&manifest.capabilities)?;
        let instance = linker
            .instantiate(&mut store, &module)
            .with_context(|| format!("instantiating '{}'", manifest.id))?;

        let ploxion = Ploxion {
            manifest,
            path: PathBuf::from(fallback_id),
            store,
            instance,
            initialized: false,
            saidgoodbye: false,
        };

        Ok(ploxion)
    }

    /// **Hot-load** a ploxion from wasm bytes into a RUNNING host: build it
    /// (compile + validate + capability-gate + instantiate — same path as boot),
    /// reject it if a ploxion with the same id is ALREADY loaded, then WIRE it to
    /// the bus (its `requires` → subscribe so it receives those topics; its
    /// `provides` are now routable because the wiring/snapshot derive providers
    /// from its manifest), run `plc_init`, dispatch whatever init emitted through
    /// the cascade, and trace a `load` [`Trace::Lifecycle`] hop. Returns the loaded
    /// [`Manifest`] (a clone) so the caller can echo it back.
    ///
    /// Errors (all CLEAN, no panic, host left unchanged on failure): a bad/invalid
    /// wasm, a module missing required exports, a rejected/over-capable manifest,
    /// or a DUPLICATE id already loaded. The duplicate check runs AFTER the
    /// manifest is read (we need the real id) but BEFORE any wiring/init, so a
    /// rejected duplicate never touches the live bus.
    pub fn load_runtime(&mut self, wasm: &[u8], fallback_id: &str) -> Result<Manifest> {
        let ploxion = self.build_ploxion(wasm, fallback_id)?;
        let id = ploxion.manifest.id.clone();
        if self.ploxions.iter().any(|p| p.id() == id) {
            return Err(anyhow!(
                "ploxion '{id}' is already loaded — unload it first (duplicate id)"
            ));
        }
        let manifest = ploxion.manifest.clone();
        // WIRE: subscribe its requires + add to the live set (register does both).
        self.register(ploxion);
        self.trace.push(Trace::Lifecycle {
            who: id.clone(),
            what: "load".into(),
        });
        // INIT: run plc_init, then cascade whatever it emitted onto the bus so a
        // freshly-loaded ploxion is fully live (its init emits route to peers).
        self.with_ploxion(&id, |p| p.call_init())?;
        self.trace.push(Trace::Lifecycle {
            who: id.clone(),
            what: "init".into(),
        });
        let mut queue = VecDeque::new();
        self.drain_into(&id, &mut queue)?;
        self.cascade(queue)?;
        Ok(manifest)
    }

    /// **Hot-unload** a loaded ploxion by id: run `plc_goodbye` on it, trace a
    /// `goodbye` [`Trace::Lifecycle`] hop, REMOVE its bus wiring (unsubscribe every
    /// topic it required, so it receives nothing afterwards), and DROP its `Store`
    /// — *droit au silence*; nothing of it survives. Its `provides` cease to be
    /// routable because the manifest is gone with the dropped ploxion.
    ///
    /// Returns the unloaded id on success, or an error if NO ploxion with that id
    /// is loaded (the daemon maps that to a `404`). Never panics.
    pub fn unload(&mut self, id: &str) -> Result<String> {
        let pos = self
            .ploxions
            .iter()
            .position(|p| p.id() == id)
            .ok_or_else(|| anyhow!("no loaded ploxion '{id}'"))?;
        // GOODBYE first (best-effort: a panicking goodbye must not block the drop).
        let _ = self.with_ploxion(id, |p| p.call_goodbye());
        self.trace.push(Trace::Lifecycle {
            who: id.to_string(),
            what: "goodbye".into(),
        });
        // UNWIRE: remove it from every topic it subscribed to.
        self.bus.unsubscribe_all(id);
        // DROP its Store (removing it from the Vec drops the wasm memory/state).
        let removed = self.ploxions.remove(pos);
        let id = removed.manifest.id.clone();
        drop(removed); // explicit: the Store (and all linear memory) is freed here.
        self.trace.push(Trace::Lifecycle {
            who: id.clone(),
            what: "unload".into(),
        });
        Ok(id)
    }

    /// Load a ploxion from a `.wasm` file path.
    pub fn load_file(&mut self, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();
        let wasm = std::fs::read(path)
            .with_context(|| format!("reading wasm file {}", path.display()))?;
        let fallback = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();
        let before = self.ploxions.len();
        self.load_bytes(&wasm, &fallback)?;
        // Record the real path on the just-loaded ploxion.
        if self.ploxions.len() > before {
            if let Some(p) = self.ploxions.last_mut() {
                p.path = path.to_path_buf();
            }
        }
        Ok(())
    }

    /// Load every `*.wasm` in a directory (sorted for determinism).
    pub fn load_dir(&mut self, dir: impl AsRef<Path>) -> Result<usize> {
        let dir = dir.as_ref();
        let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
            .with_context(|| format!("reading ploxion dir {}", dir.display()))?
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("wasm"))
            .collect();
        files.sort();
        let mut n = 0;
        for f in files {
            self.load_file(&f)?;
            n += 1;
        }
        Ok(n)
    }

    /// Register a loaded ploxion: subscribe it to its `requires` topics and add
    /// it to the live set.
    fn register(&mut self, ploxion: Ploxion) {
        for topic in &ploxion.manifest.requires {
            self.bus.subscribe(topic, &ploxion.manifest.id);
        }
        self.ploxions.push(ploxion);
    }

    /// Explicitly subscribe a ploxion (by id) to a topic beyond its manifest.
    pub fn subscribe(&mut self, who: &str, topic: &str) {
        self.bus.subscribe(topic, who);
    }

    /// Run `plc_init` on every ploxion (in load order). Any emits produced
    /// during init are dispatched on the bus afterwards, including the cascade.
    pub fn init_all(&mut self) -> Result<()> {
        let ids: Vec<String> = self.ploxions.iter().map(|p| p.id().to_string()).collect();
        for id in ids {
            self.with_ploxion(&id, |p| p.call_init())?;
            self.trace.push(Trace::Lifecycle { who: id.clone(), what: "init".into() });
            // Flush whatever init produced, then run the cascade to quiescence.
            let mut queue = VecDeque::new();
            self.drain_into(&id, &mut queue)?;
            self.cascade(queue)?;
        }
        Ok(())
    }

    /// Health sweep: returns `(id, status)` for every ploxion (0 = ok).
    pub fn health_sweep(&mut self) -> Result<Vec<(String, i32)>> {
        let ids: Vec<String> = self.ploxions.iter().map(|p| p.id().to_string()).collect();
        let mut out = Vec::with_capacity(ids.len());
        for id in ids {
            let status = self.with_ploxion(&id, |p| p.call_health())?;
            self.trace.push(Trace::Lifecycle {
                who: id.clone(),
                what: format!("health={status}"),
            });
            out.push((id, status));
        }
        Ok(out)
    }

    /// Inject an event into the bus as if from an external source named `from`
    /// (e.g. a host "tick"). Routes it and runs the full cascade to quiescence.
    pub fn inject(&mut self, from: &str, topic: &str, payload: &[u8]) -> Result<()> {
        let ev = Event {
            from: from.to_string(),
            topic: topic.to_string(),
            payload: payload.to_vec(),
        };
        let mut queue = VecDeque::new();
        queue.push_back(ev);
        self.cascade(queue)
    }

    /// Run the native [`ServiceConnector`] against the live (or stubbed)
    /// services and BRIDGE the results onto the bus.
    ///
    /// This is the host's native I/O adapter in action: the connector does real
    /// HTTP (via `check`) — something a sandboxed WASM ploxion cannot do — then
    /// the host EMITS one `service.health` event per service from the native
    /// participant `"service-connector"`, routing each to its WASM subscribers
    /// (e.g. the `watcher`) through the very same cascade ordinary ploxions use.
    ///
    /// `check` is injected so unit tests stay offline/deterministic; the demo
    /// passes [`connector::http_get_health`]. Returns the sweep rows (id, url,
    /// result, payload) so callers can also show the raw health table.
    ///
    /// The connector is auto-registered as a native participant providing
    /// `["service.health"]` (so the wiring display attributes the topic to it).
    pub fn connector_sweep(
        &mut self,
        connector: &ServiceConnector,
        check: &HealthCheck<'_>,
    ) -> Result<Vec<SweepRow>> {
        self.register_native(connector::CONNECTOR_ID, &[connector::HEALTH_TOPIC]);
        let rows = connector.sweep(check);
        // One bus event per service, emitted FROM the native connector. Each
        // emit cascades to the watcher (and any other subscriber) before the
        // next, so the trace reads service-by-service in order.
        for row in &rows {
            self.inject(
                connector::CONNECTOR_ID,
                connector::HEALTH_TOPIC,
                row.payload.as_bytes(),
            )?;
        }
        Ok(rows)
    }

    /// Process an event queue: route each event to its subscribers, then drain
    /// each subscriber's outbox/logs (so cause precedes effect in the trace).
    /// New emits are appended, so the loop runs until the bus is quiescent.
    fn cascade(&mut self, mut queue: VecDeque<Event>) -> Result<()> {
        while let Some(ev) = queue.pop_front() {
            // TRACE the emit (the host never inspects/filters the payload).
            self.trace.push(Trace::Emit {
                from: ev.from.clone(),
                topic: ev.topic.clone(),
                payload: String::from_utf8_lossy(&ev.payload).to_string(),
            });
            // ROUTE to every OTHER subscriber, draining each one right after.
            let targets: Vec<String> = self
                .bus
                .subscribers(&ev.topic)
                .iter()
                .filter(|t| **t != ev.from) // never deliver back to the emitter
                .cloned()
                .collect();
            for to in targets {
                self.trace.push(Trace::Deliver {
                    from: ev.from.clone(),
                    to: to.clone(),
                    topic: ev.topic.clone(),
                });
                // A subscriber is either a WASM ploxion or a native recipe rule.
                // Recipes have no `Store`; dispatch them as pure host logic.
                if self.ploxions.iter().any(|p| p.id() == to) {
                    self.with_ploxion(&to, |p| p.call_on_event(&ev.topic, &ev.payload))?;
                    // Whatever this delivery produced (logs + emits) is causally
                    // after the delivery, so flush it now.
                    self.drain_into(&to, &mut queue)?;
                } else {
                    self.dispatch_recipe(&to, &ev, &mut queue)?;
                }
            }
        }
        Ok(())
    }

    /// Deliver an event to a registered recipe rule (a native participant, NOT a
    /// WASM `Store`). The rule reacts purely: it evaluates its conditions against
    /// the payload and, only if they hold, produces its actions. Every step
    /// (trigger, each condition's truth value, each emit/delivery, or staying
    /// silent) is traced as a [`Trace::Recipe`] hop.
    ///
    /// Two kinds of action come back from [`CompiledRecipe::react`]:
    /// - **emits** (`action`/`sortie`) are queued FROM the recipe's id so the
    ///   cascade routes them to their bus subscribers — exactly like a ploxion's.
    /// - **deliveries** (RECIPE v1's `ploxion` kind) are TARGETED: the host calls
    ///   `plc_on_event` on the ONE ploxion whose manifest id == the delivery's
    ///   `ploxion_id`, point-to-point — NO bus route is created, no other ploxion
    ///   sees it. A delivery naming no loaded ploxion is traced as a warning and
    ///   skipped (never a panic). Whatever the delivered ploxion then emits is
    ///   drained into the cascade, exactly as for a normal bus delivery.
    fn dispatch_recipe(&mut self, to: &str, ev: &Event, queue: &mut VecDeque<Event>) -> Result<()> {
        // Find the rule by id (cloned out so we don't hold a borrow on self).
        let Some(rule) = self.recipes.iter().find(|r| r.id == to).cloned() else {
            return Ok(());
        };
        let payload = String::from_utf8_lossy(&ev.payload);
        let reaction = rule.react(&ev.topic, &payload);
        for step in &reaction.steps {
            self.trace.push(Trace::Recipe {
                who: to.to_string(),
                step: render_step(step),
            });
        }
        // (1) bus emits — routed through the cascade like any ploxion's emit.
        for (topic, payload) in reaction.emits {
            queue.push_back(Event {
                from: to.to_string(),
                topic,
                payload: payload.into_bytes(),
            });
        }
        // (2) targeted deliveries — point-to-point to ONE ploxion's plc_on_event.
        for d in reaction.deliveries {
            if self.ploxions.iter().any(|p| p.id() == d.ploxion_id) {
                // A distinct deliver hop: recipe -> ploxion <id> (point-to-point,
                // NOT a bus route — no Bus::subscribe was ever issued for this).
                self.trace.push(Trace::Deliver {
                    from: to.to_string(),
                    to: d.ploxion_id.clone(),
                    topic: d.topic.clone(),
                });
                self.with_ploxion(&d.ploxion_id, |p| p.call_on_event(&d.topic, d.payload.as_bytes()))?;
                // Whatever the delivered ploxion produced cascades, as usual.
                self.drain_into(&d.ploxion_id, queue)?;
            } else {
                // No such loaded ploxion — warn + skip, never panic.
                self.trace.push(Trace::Recipe {
                    who: to.to_string(),
                    step: format!(
                        "WARN deliver -> ploxion '{}' skipped (no loaded ploxion with that id)",
                        d.ploxion_id
                    ),
                });
            }
        }
        Ok(())
    }

    /// Drain one ploxion's pending fetches (into the trace as `Fetch` hops),
    /// logs (into the trace), and emits (appended to `queue` for the cascade to
    /// route). Order: fetch (host I/O happened first), then logs, then emits —
    /// so the journal reads cause-before-effect (fetch -> log -> emit).
    fn drain_into(&mut self, id: &str, queue: &mut VecDeque<Event>) -> Result<()> {
        let (events, logs, fetches) = self.with_ploxion(id, |p| Ok(p.drain_outbox()))?;
        for (method, url, status) in fetches {
            self.trace.push(Trace::Fetch {
                from: id.to_string(),
                method,
                url,
                status,
            });
        }
        for line in logs {
            self.trace.push(Trace::Log { from: id.to_string(), line });
        }
        for ev in events {
            queue.push_back(ev);
        }
        Ok(())
    }

    /// Run a closure against the ploxion with the given id.
    fn with_ploxion<T>(
        &mut self,
        id: &str,
        f: impl FnOnce(&mut Ploxion) -> Result<T>,
    ) -> Result<T> {
        let p = self
            .ploxions
            .iter_mut()
            .find(|p| p.id() == id)
            .ok_or_else(|| anyhow!("no loaded ploxion '{id}'"))?;
        f(p)
    }

    /// Clean shutdown: call `plc_goodbye` on each ploxion, trace it, then DROP
    /// every `Store` (and thus all linear memory / state). *Droit au silence* —
    /// nothing of any ploxion survives.
    pub fn shutdown(&mut self) -> Result<()> {
        let ids: Vec<String> = self.ploxions.iter().map(|p| p.id().to_string()).collect();
        for id in &ids {
            // Best-effort goodbye; one bad ploxion must not block the rest.
            let _ = self.with_ploxion(id, |p| p.call_goodbye());
            self.trace.push(Trace::Lifecycle { who: id.clone(), what: "goodbye".into() });
        }
        // Dropping the Vec drops every Store -> all wasm memory freed.
        self.ploxions.clear();
        self.bus = Bus::default();
        self.native_participants.clear();
        self.recipes.clear();
        Ok(())
    }
}

/// One-line rendering of a recipe [`Step`](recipe::Step) for the trace journal.
fn render_step(step: &recipe::Step) -> String {
    match step {
        recipe::Step::Triggered { topic } => format!("triggered on [{topic}]"),
        recipe::Step::Condition { expr, holds } => {
            format!("condition '{expr}' => {holds}")
        }
        recipe::Step::Emitted { topic, payload } => {
            format!("FIRE -> emit [{topic}] {payload}")
        }
        recipe::Step::Delivered { ploxion, topic, payload } => {
            format!("FIRE -> deliver ploxion '{ploxion}' [{topic}] {payload}")
        }
        recipe::Step::Silent { reason } => format!("silent ({reason})"),
    }
}

// --- free helpers -----------------------------------------------------------

/// Read the manifest from a freshly instantiated ploxion via `plc_manifest`.
fn read_manifest(p: &mut Ploxion) -> Result<Manifest> {
    let f = p
        .instance
        .get_typed_func::<(), i64>(&mut p.store, exports::MANIFEST)
        .with_context(|| format!("ploxion missing `{}`", exports::MANIFEST))?;
    let packed = f.call(&mut p.store, ())?;
    let (ptr, len) = xerboxion_plc::unpack_ptr_len(packed);
    let bytes = p.read_bytes(ptr, len)?;
    Manifest::from_json_bytes(&bytes).context("parsing ploxion manifest JSON")
}

/// Allocate `bytes.len()` in the *caller's own* linear memory via its `alloc`
/// export and copy `bytes` in, returning the `(ptr, len)` — used by `plc_fetch`
/// to hand a result buffer back to the ploxion. Re-entrant into the same Store.
/// Returns `None` (the host then returns a 0 packed value) on any failure rather
/// than panicking inside the sandbox boundary.
fn write_into_caller(caller: &mut Caller<'_, PloxionState>, bytes: &[u8]) -> Option<(u32, u32)> {
    let alloc = caller
        .get_export(exports::ALLOC)
        .and_then(|e| e.into_func())?
        .typed::<i32, i32>(&caller)
        .ok()?;
    let ptr = alloc.call(&mut *caller, bytes.len() as i32).ok()? as u32;
    let mem = match caller.get_export("memory") {
        Some(Extern::Memory(m)) => m,
        _ => return None,
    };
    mem.write(&mut *caller, ptr as usize, bytes).ok()?;
    Some((ptr, bytes.len() as u32))
}

/// Read a UTF-8 string out of a caller's memory (lossy; the host never panics
/// on a ploxion's bytes).
fn read_str(caller: &Caller<'_, PloxionState>, mem: &Memory, ptr: i32, len: i32) -> String {
    String::from_utf8_lossy(&read_vec(caller, mem, ptr, len)).to_string()
}

/// Read raw bytes out of a caller's memory, clamped to memory bounds.
fn read_vec(caller: &Caller<'_, PloxionState>, mem: &Memory, ptr: i32, len: i32) -> Vec<u8> {
    let data = mem.data(caller);
    let start = ptr.max(0) as usize;
    let len = len.max(0) as usize;
    let end = start.saturating_add(len).min(data.len());
    if start >= data.len() {
        return Vec::new();
    }
    data[start..end].to_vec()
}
