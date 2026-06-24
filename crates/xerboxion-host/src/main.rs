//! `xerboxion-rt` — the operational xerboxion-core CLI.
//!
//! Loads WASM ploxions into the host, wires the bus from their manifests, and
//! lets you SEE them connect.
//!
//! Subcommands:
//!   xerboxion-rt ls       [dir]   load ploxions, print registry + manifests + bus wiring
//!   xerboxion-rt status   [dir]   same as ls, plus a health sweep
//!   xerboxion-rt demo     [dir]   load ping+pong, drive ticks, print the ping->pong trace
//!   xerboxion-rt tsoin    [dir]   load tsoin+state-client+tracer, record 3 states,
//!                                 replay an old one, prove the bytes come back BIT-EXACT
//!   xerboxion-rt services [dir]   load the watcher (+tracer), run the NATIVE service
//!                                 connector's sweep against the LIVE deployed services
//!                                 from runtime.json, and SEE them connect to the bus
//!   xerboxion-rt capdemo  [dir]   load the PURE-WASM health-adapter (PLC v1.1 capability
//!                                 net.fetch) + watcher; the SANDBOXED adapter does a REAL
//!                                 plc_fetch of live health URLs, emits service.health, and
//!                                 the watcher reacts — a service bridged to the bus by a
//!                                 ploxion, not native code (trace: fetch->emit->route->log)
//!   xerboxion-rt adapters [dir]   the GENERIC adapter driver: load every ploxion in dir,
//!                                 DISCOVER all that declare capability net.fetch BY CAPABILITY
//!                                 (not by id), drive each (plc_init + inject its 'requires'
//!                                 trigger) so it does its REAL plc_fetch against its live
//!                                 deployed service and emits its topic, route those topics to
//!                                 a tracer observer, and print a per-adapter trace + VERDICT
//!   xerboxion-rt recipedemo [dir] [recipe.json]
//!                                 EXECUTE a designer recipe on the bus: compile it to a
//!                                 reactive rule registered as a native bus participant
//!                                 (requires=quand topics, provides=action topics), inject a
//!                                 MATCHING and a NON-MATCHING event, and SEE the rule fire
//!                                 only on the match (condition true -> emit -> routed) and
//!                                 stay silent on the non-match — closing designer->host->bus
//!   xerboxion-rt record   [dir]   the THESIS, reflexive: run the representative scene, then
//!                                 RECORD the core's OWN ordered bus trace as a tsoin timeline
//!                                 (the real tsoin engine), print engine-derived stats
//!                                 (frames, collapse Nx, Merkle root), REPLAY the timeline and
//!                                 assert the frames come back BIT-EXACT, then FORK the
//!                                 timeline and show the branch diverge while the original is
//!                                 intact — 'tout est un tsoin' applied to the runtime itself
//!   xerboxion-rt serve    [dir] [--port N] [--addr A] [--state-dir PATH]
//!                                 [--node-id NAME] [--peer ws://host/peer ...] [--peer-token T]
//!                                 the PERSISTENT DAEMON (Phase A): load dir, init the fleet,
//!                                 and expose a LIVE bus API — GET / (live map), /snapshot,
//!                                 /healthz, POST /emit (onto the REAL bus), WS /ws (live event
//!                                 stream). Host on a dedicated thread (Stores not Sync), async
//!                                 axum server reaches it via channels. Runs until SIGINT, then
//!                                 plc_goodbye + drops Stores (droit au silence). 127.0.0.1:8730
//!
//! `dir` defaults to ./target/ploxions (where scripts/build-ploxions.sh writes
//! the compiled example ploxions). The known-ploxion catalogue is read
//! read-only from the runtime session's ploxi0ns.json.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Context, Result};
use xerboxion_host::connector::{self, ServiceConnector, DEFAULT_RUNTIME};
use xerboxion_host::map;
use xerboxion_host::recorder::{self, RecordedEvent};
use xerboxion_host::registry::{self, DEFAULT_REGISTRY};
use xerboxion_host::{Host, Recipe, Trace};

fn default_ploxion_dir() -> PathBuf {
    // Relative to the workspace root when run via cargo/scripts.
    PathBuf::from("target/ploxions")
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let cmd = args.get(1).map(String::as_str).unwrap_or("help");
    // The ploxion dir is the first POSITIONAL argument after the command (so a
    // leading flag like `--json` is never mistaken for the dir). Flags that TAKE
    // a value (`--port`, `--addr`) must not have that value mistaken for the dir,
    // so we skip the token following any such flag.
    const VALUED_FLAGS: &[&str] = &[
        "--port", "--addr", "--node-id", "--peer", "--peer-token", "--state-dir",
        "--reconstruct-from",
    ];
    let dir = {
        let mut skip_next = false;
        let mut found = None;
        for a in args.iter().skip(2) {
            if skip_next {
                skip_next = false;
                continue;
            }
            if a.starts_with('-') {
                if VALUED_FLAGS.contains(&a.as_str()) {
                    skip_next = true;
                }
                continue;
            }
            found = Some(PathBuf::from(a));
            break;
        }
        found.unwrap_or_else(default_ploxion_dir)
    };

    let result = match cmd {
        "ls" => cmd_ls(&dir, false),
        "status" => cmd_ls(&dir, true),
        "demo" => cmd_demo(&dir),
        "tsoin" => cmd_tsoin(&dir),
        "services" => cmd_services(&dir),
        "capdemo" => cmd_capdemo(&dir),
        "adapters" => cmd_adapters(&dir),
        "osiris" => cmd_osiris(&dir),
        "recipedemo" => cmd_recipedemo(&dir, args.get(3).map(String::as_str)),
        "map" => cmd_map(&dir, args.iter().any(|a| a == "--json")),
        "record" => cmd_record(&dir),
        "serve" => cmd_serve(&dir, &args),
        "help" | "-h" | "--help" => {
            print_help();
            Ok(())
        }
        other => {
            eprintln!("xerboxion-rt: unknown command '{other}' (try: help)");
            return ExitCode::from(2);
        }
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("xerboxion-rt: error: {e:#}");
            ExitCode::FAILURE
        }
    }
}

fn print_help() {
    println!(
        "xerboxion-rt — operational xerboxion-core (loads + connects WASM ploxions)\n\n\
         USAGE:\n\
         \x20 xerboxion-rt ls     [dir]   load ploxions; print registry + manifests + bus wiring\n\
         \x20 xerboxion-rt status [dir]   same as ls + a live health sweep\n\
         \x20 xerboxion-rt demo   [dir]   load ping+pong, drive ticks, show ping->pong trace\n\
         \x20 xerboxion-rt tsoin  [dir]   load tsoin+state-client, record 3 states, replay an\n\
         \x20                             old one, prove the bytes come back BIT-EXACT\n\
         \x20 xerboxion-rt services [dir] run the NATIVE service connector against the LIVE\n\
         \x20                             deployed services (runtime.json) -> bus -> watcher\n\
         \x20 xerboxion-rt capdemo [dir]  load the PURE-WASM health-adapter (capability\n\
         \x20                             net.fetch) + watcher; the adapter does a REAL\n\
         \x20                             plc_fetch -> emits service.health -> watcher reacts\n\
         \x20 xerboxion-rt adapters [dir] GENERIC driver: load dir, DISCOVER every net.fetch\n\
         \x20                             adapter BY CAPABILITY (not id), drive each (init +\n\
         \x20                             inject its trigger) -> REAL plc_fetch -> emit ->\n\
         \x20                             routed to a tracer observer; per-adapter trace +\n\
         \x20                             VERDICT (discovered / fetched / emitted / UP-DOWN)\n\
         \x20 xerboxion-rt osiris [dir]   CONVERGENCE: start a LOCAL MOCK OSIRIS (machine à\n\
         \x20                             veille), load osiris-adapter + the tsoin ploxion +\n\
         \x20                             a tracer; the adapter REAL-plc_fetches the mock's\n\
         \x20                             GET routes, republishes each world event as osiris.*\n\
         \x20                             on the bus AND records it via tsoin.record -> the\n\
         \x20                             tsoin engine stores each as an instant de réel; then\n\
         \x20                             REPLAY one bit-exact (OSIRIS sees / tsoin remembers)\n\
         \x20 xerboxion-rt recipedemo [dir] [recipe.json]  EXECUTE a designer recipe on the\n\
         \x20                             bus: compile it to a reactive rule (native\n\
         \x20                             participant), inject a matching + a non-matching\n\
         \x20                             event, show it FIRES only on a match (droit au silence)\n\
         \x20 xerboxion-rt map    [dir] [--json]  load the FULL example scene (every ploxion +\n\
         \x20                             a sample recipe), run a representative scenario, and\n\
         \x20                             emit a JSON SNAPSHOT (--json) or a human summary of\n\
         \x20                             the live core: ploxions, bus wiring, trace, services\n\
         \x20 xerboxion-rt record [dir]   run the representative scene, then RECORD the core's\n\
         \x20                             OWN ordered bus trace as a tsoin timeline (real\n\
         \x20                             engine), REPLAY it bit-exact, and FORK it — 'tout est\n\
         \x20                             un tsoin' applied to the runtime itself\n\
         \x20 xerboxion-rt serve [dir] [--port N] [--addr A]  start the PERSISTENT DAEMON: load\n\
         \x20                             dir, init the fleet, and expose a LIVE bus API —\n\
         \x20                             GET / (live map) /snapshot /healthz, POST /emit (onto\n\
         \x20                             the REAL bus), WS /ws (live event stream). Runs until\n\
         \x20                             SIGINT, then plc_goodbye + drops Stores. Default\n\
         \x20                             127.0.0.1:8730\n\
         \x20                             DURABLE (opt-in): --state-dir PATH persists ploxions\n\
         \x20                             hot-loaded via POST /load and RESTORES them on the next\n\
         \x20                             startup, so they survive a deploy/crash (base set\n\
         \x20                             always reloads from dir; only the runtime delta is saved)\n\
         \x20                             BUS FEDERATION (opt-in): --node-id NAME (origin tag),\n\
         \x20                             --peer ws://host/peer (repeatable; peer-link the bus so\n\
         \x20                             a ploxion here reacts to events emitted on the peer),\n\
         \x20                             --peer-token TOK (authenticate the link). See\n\
         \x20                             docs/FEDERATION-v0.md\n\n\
         dir defaults to ./target/ploxions. Catalogue read read-only from\n\
         {DEFAULT_REGISTRY}."
    );
}

/// Print the catalogue (known ploxions) read read-only from ploxi0ns.json.
fn print_catalogue() {
    match registry::read_registry(DEFAULT_REGISTRY) {
        Ok(entries) if !entries.is_empty() => {
            println!("== KNOWN PLOXIONS (catalogue, read-only {DEFAULT_REGISTRY}) ==");
            for e in &entries {
                println!("  - {:<28} [{}] {}", e.id, e.category, e.status);
            }
            println!("  ({} known)\n", entries.len());
        }
        Ok(_) => {
            println!("== KNOWN PLOXIONS == (catalogue empty or not found)\n");
        }
        Err(e) => {
            println!("== KNOWN PLOXIONS == (catalogue unreadable: {e})\n");
        }
    }
}

/// Load ploxions and print the live registry + manifests + bus wiring.
fn load_and_describe(dir: &PathBuf) -> Result<Host> {
    let mut host = Host::new();
    let n = host.load_dir(dir).map_err(|e| {
        anyhow::anyhow!(
            "no ploxions loaded from {} ({e}). Run scripts/build-ploxions.sh first.",
            dir.display()
        )
    })?;
    println!("== LOADED PLOXIONS (live, isolated WASM) ==");
    println!("  loaded {n} ploxion(s) from {}\n", dir.display());
    for p in host.ploxions() {
        let m = &p.manifest;
        println!(
            "  * {} v{}  (own wasmtime Store/Instance)",
            m.id, m.version
        );
        println!("      provides: {:?}", m.provides);
        println!("      requires: {:?}", m.requires);
    }
    println!();

    println!("== BUS WIRING (topic -> subscribers, built from manifests) ==");
    let mut topics: Vec<(&String, &Vec<String>)> = host.bus().wiring().collect();
    topics.sort_by(|a, b| a.0.cmp(b.0));
    if topics.is_empty() {
        println!("  (no subscriptions)");
    }
    for (topic, subs) in topics {
        // Who provides this topic?
        let providers: Vec<&str> = host
            .ploxions()
            .iter()
            .filter(|p| p.manifest.provides.iter().any(|t| t == topic))
            .map(|p| p.id())
            .collect();
        println!(
            "  [{topic}]  provided by {:?}  ->  delivered to {:?}",
            providers, subs
        );
    }
    println!();
    Ok(host)
}

fn cmd_ls(dir: &PathBuf, with_health: bool) -> Result<()> {
    print_catalogue();
    let mut host = load_and_describe(dir)?;
    host.init_all()?;
    if with_health {
        println!("== HEALTH SWEEP ==");
        for (id, status) in host.health_sweep()? {
            let label = if status == 0 { "ok" } else { "DEGRADED" };
            println!("  {id:<20} health={status} ({label})");
        }
        println!();
    }
    host.shutdown()?;
    Ok(())
}

fn cmd_demo(dir: &PathBuf) -> Result<()> {
    println!("================ XERB0XI0N-RT DEMO ================\n");
    print_catalogue();
    let mut host = load_and_describe(dir)?;

    // Lifecycle: init every ploxion.
    println!("== LIFECYCLE: plc_init on each ploxion ==");
    host.init_all()?;
    println!("  all ploxions initialized\n");

    // Drive a few host "ticks": inject a tick event that ping subscribes to,
    // OR if ping emits on init we still drive ticks to show repeated flow.
    println!("== DRIVING THE BUS: 3 host ticks (inject 'tick') ==");
    for i in 1..=3 {
        host.inject("host", "tick", format!("tick#{i}").as_bytes())?;
    }
    println!("  injected 3 ticks\n");

    // Health sweep.
    println!("== HEALTH SWEEP ==");
    for (id, status) in host.health_sweep()? {
        let label = if status == 0 { "ok" } else { "DEGRADED" };
        println!("  {id:<20} health={status} ({label})");
    }
    println!();

    // The trace journal — the proof the bus routed + traced.
    println!("== EVENT TRACE (the bus ROUTES + TRACES, never filters) ==");
    let mut ping_emits = 0usize;
    let mut pong_emits = 0usize;
    let mut pong_deliveries = 0usize;
    for t in host.trace() {
        println!("{}", t.render());
        match t {
            Trace::Emit { from, topic, .. } if from == "ping" && topic == "ping" => ping_emits += 1,
            Trace::Emit { from, topic, .. } if from == "pong" && topic == "pong" => pong_emits += 1,
            Trace::Deliver { to, topic, .. } if to == "pong" && topic == "ping" => pong_deliveries += 1,
            _ => {}
        }
    }
    println!();

    // Verdict.
    println!("== VERDICT ==");
    println!("  ping emitted 'ping'      : {ping_emits}");
    println!("  pong received 'ping'     : {pong_deliveries}");
    println!("  pong emitted 'pong'      : {pong_emits}");

    host.shutdown()?;
    println!("  shutdown: plc_goodbye called, all Stores dropped (droit au silence)\n");

    if ping_emits > 0 && pong_deliveries > 0 && pong_emits > 0 {
        println!("RESULT: OK — ping -> pong flowed across the WASM sandbox boundary.");
        Ok(())
    } else {
        Err(anyhow::anyhow!(
            "events did not flow (ping_emits={ping_emits}, pong_deliveries={pong_deliveries}, pong_emits={pong_emits})"
        ))
    }
}

/// The tsoin state demo: load the **tsoin** ploxion (the versioning/state layer,
/// wrapping the real tsoin record/replay engine) + the **state-client** that
/// drives it + a passive **tracer**. The client records 3 evolving states across
/// the bus, the tsoin ploxion stores each as a delta in its OWN sandbox, then the
/// client replays an OLD state and checks the reconstructed bytes are bit-exact.
///
/// Exits non-zero on any mismatch (or if the round trip never completes).
fn cmd_tsoin(dir: &Path) -> Result<()> {
    println!("============== XERB0XI0N-RT TSOIN STATE DEMO ==============\n");
    print_catalogue();

    // Load ONLY the ploxions this demo needs (the dir may also hold ping/pong).
    let mut host = Host::new();
    for name in ["tsoin", "state-client", "tracer"] {
        let path = dir.join(format!("{name}.wasm"));
        host.load_file(&path).map_err(|e| {
            anyhow::anyhow!(
                "could not load {} ({e}). Run scripts/build-ploxions.sh first.",
                path.display()
            )
        })?;
    }

    println!("== LOADED PLOXIONS (live, isolated WASM — each its own Store) ==");
    for p in host.ploxions() {
        let m = &p.manifest;
        println!("  * {} v{}", m.id, m.version);
        println!("      provides: {:?}", m.provides);
        println!("      requires: {:?}", m.requires);
    }
    println!();

    println!("== BUS WIRING (topic -> subscribers, from manifests) ==");
    let mut topics: Vec<(&String, &Vec<String>)> = host.bus().wiring().collect();
    topics.sort_by(|a, b| a.0.cmp(b.0));
    for (topic, subs) in topics {
        println!("  [{topic}]  ->  {subs:?}");
    }
    println!();

    // init_all drives the whole cascade: state-client emits 3 records on init,
    // tsoin stores them and replies, the client then replays an early id, tsoin
    // reconstructs it bit-exact and replies, the client verifies.
    println!("== DRIVE: init state-client (it records 3 states, then replays id=0) ==\n");
    host.init_all()?;

    println!("== EVENT TRACE (the bus ROUTES + TRACES every hop, never filters) ==");
    let mut records = 0usize;
    let mut recorded = 0usize;
    let mut replays = 0usize;
    let mut replayed = 0usize;
    let mut tracer_saw = 0usize;
    let mut ok = false;
    let mut mismatch = false;
    for t in host.trace() {
        println!("{}", t.render());
        match t {
            Trace::Emit { topic, .. } if topic == "tsoin.record" => records += 1,
            Trace::Emit { topic, .. } if topic == "tsoin.recorded" => recorded += 1,
            Trace::Emit { topic, .. } if topic == "tsoin.replay" => replays += 1,
            Trace::Emit { topic, .. } if topic == "tsoin.replayed" => replayed += 1,
            Trace::Deliver { to, .. } if to == "tracer" => tracer_saw += 1,
            Trace::Log { from, line } if from == "state-client" && line.contains("OK") => ok = true,
            Trace::Log { from, line } if from == "state-client" && line.contains("MISMATCH") => {
                mismatch = true
            }
            _ => {}
        }
    }
    println!();

    println!("== HEALTH SWEEP ==");
    for (id, status) in host.health_sweep()? {
        let label = if status == 0 { "ok" } else { "DEGRADED" };
        println!("  {id:<16} health={status} ({label})");
    }
    println!();

    println!("== VERDICT ==");
    println!("  client -> tsoin  records emitted     : {records}");
    println!("  tsoin  stored & replied 'recorded'   : {recorded}");
    println!("  client -> tsoin  replay  emitted     : {replays}");
    println!("  tsoin  reconstructed & replied bytes : {replayed}");
    println!("  tracer (passive) observed hops       : {tracer_saw}");
    println!("  client bit-exact check               : {}", if ok { "OK" } else { "—" });

    host.shutdown()?;
    println!("\n  shutdown: plc_goodbye on each, all Stores dropped (droit au silence)\n");

    if mismatch {
        return Err(anyhow::anyhow!(
            "BIT-EXACT CHECK FAILED: state-client reported a MISMATCH"
        ));
    }
    if records == 3 && recorded == 3 && replays == 1 && replayed == 1 && ok {
        println!(
            "RESULT: OK — 3 states recorded via the bus -> stored as deltas in the tsoin\n\
             \x20       ploxion's own sandbox -> an OLD state replayed BIT-EXACT back across\n\
             \x20       the bus + WASM boundary. The versioning/state layer works."
        );
        Ok(())
    } else {
        Err(anyhow::anyhow!(
            "round trip incomplete (records={records}, recorded={recorded}, replays={replays}, replayed={replayed}, ok={ok})"
        ))
    }
}

/// The **services** demo: connect the LIVE deployed services to the core bus.
///
/// 1. The host's NATIVE `service-connector` reads `runtime.json` (read-only) for
///    the DEPLOYED services + their health URLs.
/// 2. It performs a REAL HTTP GET against each (native I/O — a sandboxed WASM
///    ploxion could not), and EMITS one `service.health` `{id,url,code,up}`
///    event per service onto the bus.
/// 3. The WASM `watcher` ploxion (sandboxed, isolated `Store`) `requires`
///    `service.health`, so the host routes each event into its `plc_on_event`.
///    The watcher logs UP/DOWN, counts, and emits `alert` for down services.
/// 4. A passive `tracer` watches nothing here (it doesn't require service.health)
///    — included only to demonstrate *droit au silence*: it gets NONE of them.
///
/// Prints: the deployed-service registry, the real health table, the bus wiring
/// (service.health -> watcher), and the watcher's reaction log. Exits 0 iff the
/// sweep ran and every event was delivered to the watcher.
fn cmd_services(dir: &Path) -> Result<()> {
    println!("============ XERB0XI0N-RT SERVICE CONNECTOR DEMO ============\n");

    // --- 1. The NATIVE connector reads the deployed-service registry. --------
    let conn = ServiceConnector::from_runtime(DEFAULT_RUNTIME).map_err(|e| {
        anyhow::anyhow!("could not read runtime overlay {DEFAULT_RUNTIME}: {e}")
    })?;
    println!("== DEPLOYED SERVICES (runtime.json, read-only {DEFAULT_RUNTIME}) ==");
    if conn.services().is_empty() {
        println!("  (no deployed services with a health URL)");
    }
    for s in conn.services() {
        println!("  - {:<14} health {}", s.id, s.url);
    }
    println!(
        "  ({} deployed service(s) to connect to the bus)\n",
        conn.services().len()
    );

    // --- 2. Load the WASM watcher (+ a passive tracer to prove isolation). ----
    let mut host = Host::new();
    for name in ["watcher", "tracer"] {
        let path = dir.join(format!("{name}.wasm"));
        host.load_file(&path).map_err(|e| {
            anyhow::anyhow!(
                "could not load {} ({e}). Run scripts/build-ploxions.sh first.",
                path.display()
            )
        })?;
    }
    println!("== LOADED PLOXIONS (live, isolated WASM — each its own Store) ==");
    for p in host.ploxions() {
        let m = &p.manifest;
        println!("  * {} v{}", m.id, m.version);
        println!("      provides: {:?}", m.provides);
        println!("      requires: {:?}", m.requires);
    }
    println!(
        "  (+ native participant '{}' provides [\"{}\"] — host-side I/O, NOT sandboxed)\n",
        connector::CONNECTOR_ID,
        connector::HEALTH_TOPIC
    );

    host.init_all()?;

    // --- 3. Run the LIVE sweep: real HTTP -> bus -> watcher. ------------------
    println!("== LIVE HEALTH SWEEP (real HTTP GET via the native connector) ==");
    let rows = host.connector_sweep(&conn, &connector::http_get_health)?;
    for r in &rows {
        let label = if r.result.up { "UP " } else { "DOWN" };
        println!(
            "  {:<14} {} code={:03}  ({})",
            r.id, label, r.result.code, r.url
        );
    }
    println!();

    // --- Bus wiring (who provides service.health -> who it's delivered to). ---
    println!("== BUS WIRING (service.health -> subscribers) ==");
    let providers: Vec<&String> = host
        .native_participants()
        .filter(|(_, topics)| topics.iter().any(|t| t == connector::HEALTH_TOPIC))
        .map(|(id, _)| id)
        .collect();
    let subs = host.bus().subscribers(connector::HEALTH_TOPIC);
    println!(
        "  [{}]  provided by {:?} (native)  ->  delivered to {:?} (WASM)",
        connector::HEALTH_TOPIC,
        providers,
        subs
    );
    println!();

    // --- 4. The trace journal: the bus routing each health event + the
    //         watcher's reaction (its plc_log lines) + any alert it raised. ----
    println!("== EVENT TRACE (bus ROUTES + TRACES; native connector -> WASM watcher) ==");
    let mut delivered_to_watcher = 0usize;
    let mut tracer_got_health = 0usize;
    let mut alerts = 0usize;
    for t in host.trace() {
        // Only show the service-connector flow (skip init/observe noise).
        let relevant = matches!(t,
            Trace::Emit { topic, .. } if topic == connector::HEALTH_TOPIC || topic == "alert")
            || matches!(t, Trace::Deliver { topic, .. } if topic == connector::HEALTH_TOPIC || topic == "alert")
            || matches!(t, Trace::Log { from, .. } if from == "watcher");
        if relevant {
            println!("{}", t.render());
        }
        match t {
            Trace::Deliver { to, topic, .. }
                if to == "watcher" && topic == connector::HEALTH_TOPIC =>
            {
                delivered_to_watcher += 1
            }
            Trace::Deliver { to, topic, .. }
                if to == "tracer" && topic == connector::HEALTH_TOPIC =>
            {
                tracer_got_health += 1
            }
            Trace::Emit { from, topic, .. } if from == "watcher" && topic == "alert" => {
                alerts += 1
            }
            _ => {}
        }
    }
    println!();

    // --- Health sweep of the ploxions themselves (watcher mirrors the fleet). -
    println!("== PLOXION HEALTH SWEEP ==");
    for (id, status) in host.health_sweep()? {
        let label = if status == 0 { "ok" } else { "DEGRADED" };
        println!("  {id:<16} health={status} ({label})");
    }
    println!();

    let up = rows.iter().filter(|r| r.result.up).count();
    let down = rows.len() - up;

    println!("== VERDICT ==");
    println!("  deployed services swept (real HTTP)  : {}", rows.len());
    println!("  service.health events on the bus     : {}", rows.len());
    println!("  delivered into the WASM watcher      : {delivered_to_watcher}");
    println!("  services UP / DOWN                    : {up} / {down}");
    println!("  watcher 'alert' emits (down services): {alerts}");
    println!("  tracer (non-subscriber) got health   : {tracer_got_health}  (must be 0 — droit au silence)");

    host.shutdown()?;
    println!("\n  shutdown: plc_goodbye on each, all Stores dropped (droit au silence)\n");

    // Exit 0 iff the sweep ran AND every event reached the watcher AND the
    // non-subscriber got nothing.
    if !rows.is_empty() && delivered_to_watcher == rows.len() && tracer_got_health == 0 {
        println!(
            "RESULT: OK — {} LIVE deployed service(s) health-checked by the host's native\n\
             \x20       connector and bridged onto the bus; the WASM watcher reacted to every\n\
             \x20       one. The deployed services are connected to the xerboxion-core.",
            rows.len()
        );
        Ok(())
    } else {
        Err(anyhow::anyhow!(
            "service connect incomplete (swept={}, delivered_to_watcher={delivered_to_watcher}, tracer_got_health={tracer_got_health})",
            rows.len()
        ))
    }
}

/// The **capability** demo (PLC v1.1): a service bridged to the bus by a
/// **pure-WASM ploxion** instead of native host code.
///
/// 1. Load the sandboxed `health-adapter` ploxion. Its manifest declares
///    `capabilities:["net.fetch"]`, so the host — and ONLY for this ploxion —
///    links the gated `plc_fetch` host import into its `Store`. The `watcher`
///    (declares no capability) loads alongside it WITHOUT `plc_fetch`.
/// 2. `plc_init` drives the adapter: it performs a REAL HTTP GET of its default
///    health URLs *through the host* (`plc_fetch`) — it never opens a socket
///    itself — and emits one `service.health` `{id,url,code,up}` per URL.
/// 3. We then inject a `health.check` for a live HTTPS URL, driving one more
///    brokered fetch + emit at runtime.
/// 4. The host routes each `service.health` into the watcher's `plc_on_event`;
///    the watcher logs UP/DOWN and alerts on DOWN — reacting to the SANDBOXED
///    adapter exactly as it reacts to the native connector (same topic/shape).
///
/// The trace shows the full chain: fetch -> emit -> route -> watcher log. Exits
/// 0 iff the adapter performed >=1 brokered fetch, emitted service.health, and
/// every event reached the watcher.
fn cmd_capdemo(dir: &Path) -> Result<()> {
    println!("============ XERB0XI0N-RT CAPABILITY DEMO (PLC v1.1) ============\n");
    println!(
        "  A service bridged to the bus by a PURE-WASM ploxion (capability net.fetch),\n\
         \x20 not by native host code. The host links plc_fetch ONLY for the ploxion that\n\
         \x20 declared the capability — least authority / consent.\n"
    );

    // --- Load the sandboxed adapter + the watcher. ----------------------------
    let mut host = Host::new();
    for name in ["health-adapter", "watcher"] {
        let path = dir.join(format!("{name}.wasm"));
        host.load_file(&path).map_err(|e| {
            anyhow::anyhow!(
                "could not load {} ({e}). Run scripts/build-ploxions.sh first.",
                path.display()
            )
        })?;
    }

    println!("== LOADED PLOXIONS (live, isolated WASM — each its own Store) ==");
    for p in host.ploxions() {
        let m = &p.manifest;
        println!("  * {} v{}", m.id, m.version);
        println!("      capabilities: {:?}", m.capabilities);
        println!("      provides:     {:?}", m.provides);
        println!("      requires:     {:?}", m.requires);
    }
    println!(
        "  (host linked plc_fetch ONLY into health-adapter's Store — watcher has no such import)\n"
    );

    // The same GENERIC discovery the `adapters` driver uses: find the net.fetch
    // adapter(s) BY CAPABILITY, never by hardcoded id. Here it is exactly the
    // health-adapter, but capdemo and the generic driver agree on the mechanism.
    let discovered = discover_net_fetch_adapters(&host);
    println!(
        "== DISCOVERED net.fetch ADAPTER(S) (by capability) : {:?} ==\n",
        discovered.iter().map(|a| a.id.as_str()).collect::<Vec<_>>()
    );

    println!("== BUS WIRING (service.health -> subscribers) ==");
    let subs = host.bus().subscribers("service.health");
    println!("  [service.health]  provided by [\"health-adapter\"] (WASM, via plc_fetch)  ->  delivered to {subs:?}\n");

    // --- init: the adapter does REAL fetches of its default targets + emits. ---
    println!("== DRIVE: plc_init (adapter plc_fetches its default health URLs) ==");
    host.init_all()?;

    // --- a runtime trigger: probe a live HTTPS URL via health.check. ----------
    println!("== TRIGGER: inject health.check for https://j0bot.ch (runtime fetch) ==");
    host.inject(
        "host",
        "health.check",
        br#"{"id":"j0bot","url":"https://j0bot.ch"}"#,
    )?;
    println!();

    // --- the trace: fetch -> emit -> route -> watcher log. --------------------
    println!("== EVENT TRACE (fetch -> emit -> route -> watcher reaction) ==");
    let mut fetches = 0usize;
    let mut health_emits = 0usize;
    let mut delivered_to_watcher = 0usize;
    let mut alerts = 0usize;
    for t in host.trace() {
        let relevant = matches!(t, Trace::Fetch { from, .. } if from == "health-adapter")
            || matches!(t, Trace::Emit { topic, .. } if topic == "service.health" || topic == "alert")
            || matches!(t, Trace::Deliver { topic, .. } if topic == "service.health" || topic == "alert")
            || matches!(t, Trace::Log { from, .. } if from == "health-adapter" || from == "watcher");
        if relevant {
            println!("{}", t.render());
        }
        match t {
            Trace::Fetch { from, .. } if from == "health-adapter" => fetches += 1,
            Trace::Emit { from, topic, .. }
                if from == "health-adapter" && topic == "service.health" =>
            {
                health_emits += 1
            }
            Trace::Deliver { to, topic, .. }
                if to == "watcher" && topic == "service.health" =>
            {
                delivered_to_watcher += 1
            }
            Trace::Emit { from, topic, .. } if from == "watcher" && topic == "alert" => {
                alerts += 1
            }
            _ => {}
        }
    }
    println!();

    println!("== VERDICT ==");
    println!("  brokered plc_fetch calls (host I/O)  : {fetches}");
    println!("  service.health emitted by the ADAPTER: {health_emits}");
    println!("  delivered into the WASM watcher      : {delivered_to_watcher}");
    println!("  watcher 'alert' emits (down services): {alerts}");

    host.shutdown()?;
    println!("\n  shutdown: plc_goodbye on each, all Stores dropped (droit au silence)\n");

    if fetches >= 1 && health_emits >= 1 && delivered_to_watcher == health_emits {
        println!(
            "RESULT: OK — the SANDBOXED health-adapter (capability net.fetch) did {fetches} REAL\n\
             \x20       plc_fetch(es), emitted {health_emits} service.health onto the bus, and the\n\
             \x20       watcher reacted to every one. A service bridged to the core by a WASM\n\
             \x20       ploxion, not native code — option (A), least authority, consent honoured."
        );
        Ok(())
    } else {
        Err(anyhow::anyhow!(
            "capability demo incomplete (fetches={fetches}, health_emits={health_emits}, delivered_to_watcher={delivered_to_watcher})"
        ))
    }
}

/// The capability token a ploxion must declare to be a `net.fetch` service
/// adapter (and thus a candidate for the generic driver). Defined here so the
/// discovery is by the SAME token the host gates `plc_fetch` on — never a
/// hardcoded ploxion id.
const NET_FETCH: &str = xerboxion_plc::capabilities::NET_FETCH;

/// One discovered adapter and the topics the driver derives from its manifest.
struct DiscoveredAdapter {
    /// The ploxion id (for the trace + verdict). NOT used to decide it is an
    /// adapter — that is by capability — only to address it on the bus.
    id: String,
    /// Its `provides` topics (e.g. `service.health` / `pin.list` / `repo.list`).
    /// We route these to the observer so the emits are SEEN landing on the bus.
    provides: Vec<String>,
    /// Its `requires` trigger topics (e.g. `health.check` / `pin.refresh` /
    /// `repo.refresh`). Injecting one forces a live re-probe at runtime.
    requires: Vec<String>,
}

/// Discover every loaded ploxion that DECLARES capability `net.fetch` — purely
/// by the manifest capability, never by a hardcoded id. health-adapter,
/// ideas-map-adapter, repoverse-adapter (and any future Nth adapter) are all
/// found here automatically; a ploxion without the capability is skipped.
fn discover_net_fetch_adapters(host: &Host) -> Vec<DiscoveredAdapter> {
    host.ploxions()
        .iter()
        .filter(|p| p.manifest.has_capability(NET_FETCH))
        .map(|p| DiscoveredAdapter {
            id: p.id().to_string(),
            provides: p.manifest.provides.clone(),
            requires: p.manifest.requires.clone(),
        })
        .collect()
}

/// The **adapters** subcommand: the GENERIC pure-WASM service-adapter driver.
///
/// Where `capdemo` is hardcoded to load+drive only the `health-adapter`, this
/// drives EVERY `net.fetch` adapter in `dir` — discovered by capability, not by
/// id — so the oracle's `ideas-map-adapter` and `repoverse-adapter` (and any
/// future adapter) are driven automatically with no code change.
///
/// 1. Load every ploxion in `dir`, plus a generic `tracer` observer (it logs
///    whatever it is delivered, so routed emits are SEEN, not just counted).
/// 2. DISCOVER the net.fetch adapters by [`Manifest::has_capability`].
/// 3. Subscribe the observer to each discovered adapter's `provides` topics, so
///    the host routes those emits to a real ploxion (proof they land on the bus).
/// 4. DRIVE each adapter: `plc_init` (adapters fetch their baked defaults on
///    init) PLUS inject each adapter's declared `requires` trigger topic to
///    force a live re-probe. Both paths do a REAL brokered `plc_fetch` against
///    the adapter's live deployed service and emit its topic.
/// 5. Print a per-adapter trace (GET url => code, emit topic payload) and a
///    VERDICT: discovered / did-a-real-fetch / emits-on-bus / services UP/DOWN.
///
/// A DOWN/unreachable service is a VALID outcome: the adapter emits code 0 /
/// up:false and the driver records it as DOWN — never a crash. Exits non-zero
/// only if NO adapter was discovered or NONE emitted.
fn cmd_adapters(dir: &Path) -> Result<()> {
    println!("============ XERB0XI0N-RT GENERIC ADAPTER DRIVER ============\n");
    println!(
        "  Discover every pure-WASM service adapter BY CAPABILITY (net.fetch), not by id,\n\
         \x20 and drive each so it does its REAL plc_fetch against its live deployed service\n\
         \x20 and emits its topic onto the bus — a tracer observer SEES every emit routed.\n"
    );

    // --- 1. Load every ploxion in the dir + a generic observer. ---------------
    let mut host = Host::new();
    let n = host.load_dir(dir).map_err(|e| {
        anyhow::anyhow!(
            "no ploxions loaded from {} ({e}). Run scripts/build-ploxions.sh first.",
            dir.display()
        )
    })?;
    // The observer: load the tracer from the dir if it is not already there
    // (load_dir already loaded it when present). The tracer logs every delivery,
    // so we SEE the adapter emits get routed.
    let have_tracer = host.ploxions().iter().any(|p| p.id() == "tracer");
    if !have_tracer {
        let tracer_path = dir.join("tracer.wasm");
        host.load_file(&tracer_path).map_err(|e| {
            anyhow::anyhow!(
                "could not load observer {} ({e}). Run scripts/build-ploxions.sh first.",
                tracer_path.display()
            )
        })?;
    }
    println!("== LOADED PLOXIONS ({n} from {}) ==", dir.display());
    for p in host.ploxions() {
        let m = &p.manifest;
        let caps = if m.capabilities.is_empty() {
            String::new()
        } else {
            format!("  caps={:?}", m.capabilities)
        };
        println!("  * {:<20} v{}{}", m.id, m.version, caps);
    }
    println!();

    // --- 2. DISCOVER the net.fetch adapters BY CAPABILITY. --------------------
    let adapters = discover_net_fetch_adapters(&host);
    println!("== DISCOVERED net.fetch ADAPTERS (by capability, not by id) ==");
    if adapters.is_empty() {
        println!("  (none — no loaded ploxion declares capability '{NET_FETCH}')");
    }
    for a in &adapters {
        println!(
            "  + {:<20} provides {:?}  requires {:?}",
            a.id, a.provides, a.requires
        );
    }
    println!(
        "  ({} adapter(s) discovered — the host linked plc_fetch ONLY into these Stores)\n",
        adapters.len()
    );

    // --- 3. Route every adapter's provided topic(s) to the observer. ----------
    // The observer is the generic `tracer` ploxion (loaded above, guaranteed
    // present): it logs whatever it is delivered, so a routed emit is SEEN. The
    // emitter of a topic is never delivered its own event, so subscribing an
    // adapter that also provides a routed topic is harmless — but the tracer is
    // never an adapter, so this never collides.
    let observer = "tracer";
    let mut routed_topics: Vec<String> = Vec::new();
    for a in &adapters {
        for topic in &a.provides {
            host.subscribe(observer, topic);
            if !routed_topics.contains(topic) {
                routed_topics.push(topic.clone());
            }
        }
    }
    println!("== BUS WIRING (adapter topics -> observer '{observer}') ==");
    for topic in &routed_topics {
        let subs = host.bus().subscribers(topic);
        let providers: Vec<&str> = adapters
            .iter()
            .filter(|a| a.provides.iter().any(|t| t == topic))
            .map(|a| a.id.as_str())
            .collect();
        println!("  [{topic}]  provided by {providers:?} (WASM, via plc_fetch)  ->  delivered to {subs:?}");
    }
    println!();

    // --- 4a. DRIVE init: each adapter fetches its baked defaults + emits. ------
    println!("== DRIVE: plc_init on each ploxion (adapters fetch their default URLs) ==");
    host.init_all()?;

    // --- 4b. DRIVE the trigger: inject each adapter's `requires` topic to force
    //         a live re-probe at runtime. An empty `{}` payload makes the adapter
    //         re-probe its baked default URL (every adapter accepts that). -------
    println!("== TRIGGER: inject each adapter's 'requires' topic (force a live re-probe) ==");
    for a in &adapters {
        for trigger in &a.requires {
            println!("  inject [{trigger}] -> {} (re-probe its live service)", a.id);
            host.inject("host", trigger, b"{}")?;
        }
    }
    println!();

    // --- 5. The trace + per-adapter tally. ------------------------------------
    // Per adapter: count brokered fetches, emits, and remember the last code/up.
    use std::collections::BTreeMap;
    #[derive(Default)]
    struct Tally {
        fetches: usize,
        emits: usize,
        last_code: u16,
        up: bool,
    }
    let mut per: BTreeMap<String, Tally> = BTreeMap::new();
    for a in &adapters {
        per.entry(a.id.clone()).or_default();
    }
    let adapter_ids: Vec<&str> = adapters.iter().map(|a| a.id.as_str()).collect();
    let adapter_topics: Vec<&str> = adapters
        .iter()
        .flat_map(|a| a.provides.iter().map(|s| s.as_str()))
        .collect();

    println!("== EVENT TRACE (per adapter: fetch -> emit -> route -> observer) ==");
    let mut delivered_to_observer = 0usize;
    for t in host.trace() {
        let relevant = matches!(t, Trace::Fetch { from, .. } if adapter_ids.contains(&from.as_str()))
            || matches!(t, Trace::Emit { topic, .. } if adapter_topics.contains(&topic.as_str()))
            || matches!(t, Trace::Deliver { topic, .. } if adapter_topics.contains(&topic.as_str()))
            || matches!(t, Trace::Log { from, .. }
                if adapter_ids.contains(&from.as_str()) || from == observer);
        if relevant {
            println!("{}", t.render());
        }
        match t {
            Trace::Fetch { from, status, .. } if adapter_ids.contains(&from.as_str()) => {
                if let Some(tl) = per.get_mut(from) {
                    tl.fetches += 1;
                    tl.last_code = *status;
                    tl.up = (200..400).contains(status);
                }
            }
            Trace::Emit { from, topic, .. }
                if adapter_ids.contains(&from.as_str())
                    && adapter_topics.contains(&topic.as_str()) =>
            {
                if let Some(tl) = per.get_mut(from) {
                    tl.emits += 1;
                }
            }
            Trace::Deliver { to, topic, .. }
                if to == observer && adapter_topics.contains(&topic.as_str()) =>
            {
                delivered_to_observer += 1
            }
            _ => {}
        }
    }
    println!();

    // --- VERDICT. -------------------------------------------------------------
    let discovered = adapters.len();
    let did_fetch = per.values().filter(|t| t.fetches >= 1).count();
    let total_emits: usize = per.values().map(|t| t.emits).sum();
    let with_emit = per.values().filter(|t| t.emits >= 1).count();
    let up = per.values().filter(|t| t.fetches >= 1 && t.up).count();
    let down = per.values().filter(|t| t.fetches >= 1 && !t.up).count();

    println!("== PER-ADAPTER ==");
    for a in &adapters {
        let tl = &per[&a.id];
        let state = if tl.fetches == 0 {
            "no-fetch".to_string()
        } else if tl.up {
            format!("UP   (code {:03})", tl.last_code)
        } else {
            format!("DOWN (code {:03})", tl.last_code)
        };
        println!(
            "  {:<20} fetches={} emits={} -> {}",
            a.id, tl.fetches, tl.emits, state
        );
    }
    println!();

    println!("== VERDICT ==");
    println!("  net.fetch adapters DISCOVERED (by cap) : {discovered}");
    println!("  adapters that did a REAL plc_fetch      : {did_fetch}");
    println!("  topic emits on the bus (total)          : {total_emits}  (from {with_emit} adapter(s))");
    println!("  emits routed to the observer '{observer}'    : {delivered_to_observer}");
    println!("  services UP / DOWN                       : {up} / {down}");

    host.shutdown()?;
    println!("\n  shutdown: plc_goodbye on each, all Stores dropped (droit au silence)\n");

    // Exit non-zero ONLY if discovery found nothing or nothing emitted. A DOWN
    // service (fetch code 0 / up:false) is a VALID outcome, not a failure.
    if discovered == 0 {
        return Err(anyhow::anyhow!(
            "no net.fetch adapter discovered in {} — nothing to drive",
            dir.display()
        ));
    }
    if total_emits == 0 {
        return Err(anyhow::anyhow!(
            "discovered {discovered} adapter(s) but NONE emitted onto the bus"
        ));
    }
    println!(
        "RESULT: OK — {discovered} net.fetch adapter(s) discovered BY CAPABILITY and driven; {did_fetch}\n\
         \x20       did a REAL plc_fetch against their live service, emitting {total_emits} event(s) onto\n\
         \x20       the bus ({up} UP / {down} DOWN), all routed to the observer. A future Nth adapter\n\
         \x20       would be discovered + driven the same way — generic, not hardcoded."
    );
    Ok(())
}

/// The **osiris** subcommand: the CONVERGENCE of the two machines.
///
/// OSIRIS (José's *machine à veille*, an OSINT dashboard) SEES the world; the
/// tsoin engine (the *machine à tsoins*, record/replay over a content-addressed
/// timeline) REMEMBERS it. This demo wires them together end to end, hermetic:
///
/// 1. start a LOCAL MOCK OSIRIS — a throwaway localhost HTTP server serving the
///    committed OSIRIS-shaped fixtures (`ploxions/osiris-adapter/fixtures/*`) on
///    its real GET routes (`/api/earthquakes`, `/api/flights`, …);
/// 2. load the PURE-WASM `osiris-adapter` (capability `net.fetch`), the `tsoin`
///    ploxion (the real engine), and a `tracer` observer; auto-wire the bus from
///    their manifests (`tsoin` auto-subscribes to `tsoin.record`/`tsoin.replay`);
/// 3. subscribe the tracer to the adapter's `osiris.*` topics so each republished
///    world event is SEEN landing on the bus;
/// 4. DRIVE the adapter at the mock: inject `osiris.refresh {"base":..}`. The
///    adapter does a REAL brokered `plc_fetch` (over loopback) of each route,
///    normalises every event, EMITS `osiris.<domain>`, AND emits `tsoin.record`
///    {name:"osiris:<domain>:<id>", bytes:<event-json hex>} for a bounded sample;
/// 5. the host routes each `tsoin.record` into the `tsoin` ploxion, which stores
///    it as one frame of reel and replies `tsoin.recorded {id,...}`;
/// 6. REPLAY one recorded instant: inject `tsoin.replay {"id":..}`; the engine
///    reconstructs the bytes BIT-EXACT and we assert they equal the exact osiris
///    event JSON the adapter emitted — proof the reel is faithful.
///
/// Exits 0 iff: ≥1 real plc_fetch happened, osiris.* events landed on the bus,
/// ≥1 tsoin.record was recorded by the tsoin ploxion, and the replay came back
/// bit-exact equal to the originally-emitted event.
fn cmd_osiris(dir: &Path) -> Result<()> {
    use xerboxion_host::mock_osiris::MockOsiris;

    println!("============ XERB0XI0N-RT OSIRIS x MACHINE À TSOINS ============\n");
    println!(
        "  OSIRIS (machine à veille) SEES the world; the tsoin engine REMEMBERS it.\n\
         \x20 The pure-WASM osiris-adapter polls OSIRIS GET routes (REAL plc_fetch),\n\
         \x20 republishes each world event as osiris.* on the bus, AND records each as\n\
         \x20 a tsoin (un instant de réel) via tsoin.record -> the tsoin ploxion stores it,\n\
         \x20 replayable BIT-EXACT. This is the convergence of the two machines.\n"
    );

    // --- 1. The local MOCK OSIRIS (deterministic upstream, real HTTP). --------
    let mock = MockOsiris::start()
        .map_err(|e| anyhow::anyhow!("could not start mock OSIRIS: {e}"))?;
    let base = mock.base_url().to_string();
    println!("== MOCK OSIRIS (machine à veille) ==");
    println!("  serving committed OSIRIS-shaped fixtures at {base}");
    println!("  routes: /api/earthquakes /api/flights /api/frontlines /api/cyber-threats\n");

    // --- 2. Load the adapter + the tsoin engine ploxion + a tracer observer. --
    let mut host = Host::new();
    // Load tsoin BEFORE the adapter: plc_init runs in load order, so the tsoin
    // engine is built before the adapter's init poll could emit any tsoin.record
    // (a record to a not-yet-initialised tsoin is dropped — load order is the
    // contract the convergence relies on).
    for name in ["tsoin", "osiris-adapter", "tracer"] {
        let path = dir.join(format!("{name}.wasm"));
        host.load_file(&path).map_err(|e| {
            anyhow::anyhow!(
                "could not load {} ({e}). Run scripts/build-ploxions.sh first.",
                path.display()
            )
        })?;
    }

    println!("== LOADED PLOXIONS (live, isolated WASM — each its own Store) ==");
    for p in host.ploxions() {
        let m = &p.manifest;
        let caps = if m.capabilities.is_empty() { String::new() } else { format!("  caps={:?}", m.capabilities) };
        println!("  * {:<16} v{}{}", m.id, m.version, caps);
        println!("      provides: {:?}", m.provides);
        println!("      requires: {:?}", m.requires);
    }
    println!();

    // --- 3. Route the adapter's osiris.* topics to the tracer observer. -------
    // (tsoin already auto-subscribed to tsoin.record/tsoin.replay from its
    //  manifest `requires` — that is the machine-à-tsoins connection, wired by
    //  the manifests, not hardcoded here.)
    const OSIRIS_TOPICS: &[&str] = &[
        "osiris.air.track",
        "osiris.land.quake",
        "osiris.land.fire",
        "osiris.conflict.zone",
        "osiris.cyber.cve",
        "osiris.alert",
    ];
    for t in OSIRIS_TOPICS {
        host.subscribe("tracer", t);
    }
    println!("== BUS WIRING (the convergence) ==");
    println!("  osiris-adapter --[osiris.*]--> tracer            (the veille, on the bus)");
    println!(
        "  osiris-adapter --[tsoin.record]--> {:?}  (the connection: each event -> a tsoin)",
        host.bus().subscribers("tsoin.record")
    );
    println!();

    // --- 4. INIT each ploxion (the tsoin engine builds its Recorder+Timeline on
    //         plc_init; without it a tsoin.record would be dropped). The adapter's
    //         own plc_init polls its baked default (127.0.0.1:3000), which is DOWN
    //         here — a valid no-op, skipped without panic; the real poll is the
    //         osiris.refresh below, pointed at the mock. -------------------------
    println!("== INIT: plc_init on each ploxion (tsoin builds its engine; adapter probes default) ==");
    host.init_all()?;
    println!();

    // --- 4b. DRIVE: point the adapter at the mock and poll. -------------------
    println!("== DRIVE: inject osiris.refresh {{\"base\":\"{base}\"}} (REAL plc_fetch the mock) ==");
    let refresh = format!("{{\"base\":\"{base}\"}}");
    host.inject("host", "osiris.refresh", refresh.as_bytes())?;
    println!();

    // --- 5. The trace: fetch -> osiris.* emit -> tsoin.record -> recorded. -----
    println!("== EVENT TRACE (poll -> osiris.* + tsoin.record -> recorded) ==");
    let mut fetches = 0usize;
    let mut osiris_emits = 0usize;
    let mut tsoin_records = 0usize;
    let mut recorded_ok = 0usize;
    // Remember, per recorded id, the osiris event bytes the adapter emitted so we
    // can later compare the replay. The adapter emits tsoin.record with the SAME
    // bytes (hex) it published on the osiris.* topic, in order.
    let mut recorded: Vec<(u64, Vec<u8>)> = Vec::new();
    let mut pending_record_bytes: std::collections::VecDeque<Vec<u8>> = Default::default();

    for t in host.trace() {
        let show = matches!(t, Trace::Fetch { from, .. } if from == "osiris-adapter")
            || matches!(t, Trace::Emit { topic, .. }
                if OSIRIS_TOPICS.contains(&topic.as_str()) || topic == "tsoin.record" || topic == "tsoin.recorded")
            || matches!(t, Trace::Deliver { to, topic, .. }
                if (to == "tracer" && OSIRIS_TOPICS.contains(&topic.as_str())) || (to == "tsoin" && topic == "tsoin.record"))
            || matches!(t, Trace::Log { from, .. } if from == "tsoin");
        if show {
            println!("{}", t.render());
        }
        match t {
            Trace::Fetch { from, .. } if from == "osiris-adapter" => fetches += 1,
            Trace::Emit { from, topic, payload } if from == "osiris-adapter" => {
                if OSIRIS_TOPICS.contains(&topic.as_str()) {
                    osiris_emits += 1;
                } else if topic == "tsoin.record" {
                    tsoin_records += 1;
                    // Decode the bytes the adapter recorded (hex inside the payload).
                    if let Some(bytes) = json_hex_field(payload, "bytes") {
                        pending_record_bytes.push_back(bytes);
                    }
                }
            }
            Trace::Emit { from, topic, payload } if from == "tsoin" && topic == "tsoin.recorded" => {
                recorded_ok += 1;
                if let (Some(id), Some(orig)) =
                    (json_u64_field(payload, "id"), pending_record_bytes.pop_front())
                {
                    recorded.push((id, orig));
                }
            }
            _ => {}
        }
    }
    println!();

    // --- 6. REPLAY one recorded instant of reel — assert bit-exact. -----------
    println!("== REPLAY: reconstruct one recorded instant de réel (assert BIT-EXACT) ==");
    let mut replay_ok: Option<bool> = None;
    let mut replayed_id = 0u64;
    if let Some((id, orig)) = recorded.first().cloned() {
        replayed_id = id;
        let payload = format!("{{\"id\":{id}}}");
        println!("  inject tsoin.replay {{\"id\":{id}}}  (the FIRST recorded osiris event)");
        let before = host.trace().len();
        host.inject("host", "tsoin.replay", payload.as_bytes())?;
        for t in &host.trace()[before..] {
            if matches!(t, Trace::Log { from, .. } if from == "tsoin") {
                println!("{}", t.render());
            }
            if let Trace::Emit { from, topic, payload } = t {
                if from == "tsoin" && topic == "tsoin.replayed" {
                    if let Some(got) = json_hex_field(payload, "bytes") {
                        let ok = got == orig;
                        replay_ok = Some(ok);
                        println!(
                            "  tsoin.replayed id={id}: {} bytes reconstructed -> {}",
                            got.len(),
                            if ok { "BIT-EXACT match" } else { "MISMATCH" }
                        );
                        if ok {
                            println!(
                                "  reel = {}",
                                String::from_utf8_lossy(&got)
                            );
                        }
                    }
                }
            }
        }
    } else {
        println!("  (no recorded instant to replay)");
    }
    println!();

    // --- VERDICT. -------------------------------------------------------------
    println!("== VERDICT ==");
    println!("  REAL plc_fetch of OSIRIS routes        : {fetches}");
    println!("  osiris.* world events on the bus        : {osiris_emits}");
    println!("  tsoin.record emitted by the adapter     : {tsoin_records}");
    println!("  recorded by the tsoin ploxion (reel)    : {recorded_ok}");
    match replay_ok {
        Some(true) => println!("  replay of id={replayed_id}                       : BIT-EXACT ✓"),
        Some(false) => println!("  replay of id={replayed_id}                       : MISMATCH ✗"),
        None => println!("  replay                                  : not performed"),
    }

    host.shutdown()?;
    println!("\n  shutdown: plc_goodbye on each, all Stores dropped (droit au silence)");
    drop(mock);
    println!("  mock OSIRIS stopped\n");

    let ok = fetches >= 1
        && osiris_emits >= 1
        && tsoin_records >= 1
        && recorded_ok >= 1
        && replay_ok == Some(true);
    if ok {
        println!(
            "RESULT: OK — the osiris-adapter did {fetches} REAL plc_fetch(es) of OSIRIS, put\n\
             \x20       {osiris_emits} world event(s) on the bus as osiris.*, and recorded {recorded_ok} of them as\n\
             \x20       tsoins (instants de réel) in the tsoin engine — one replayed BIT-EXACT.\n\
             \x20       OSIRIS sees the world; the machine à tsoins remembers it. Convergence."
        );
        Ok(())
    } else {
        Err(anyhow::anyhow!(
            "osiris convergence incomplete (fetches={fetches}, osiris_emits={osiris_emits}, tsoin_records={tsoin_records}, recorded_ok={recorded_ok}, replay_ok={replay_ok:?})"
        ))
    }
}

/// Extract an unsigned integer `"key"` from a flat JSON payload string.
fn json_u64_field(json: &str, key: &str) -> Option<u64> {
    let needle = format!("\"{key}\"");
    let start = json.find(&needle)? + needle.len();
    let rest = &json[start..];
    let colon = rest.find(':')?;
    let after = rest[colon + 1..].trim_start();
    let digits: String = after.chars().take_while(|c| c.is_ascii_digit()).collect();
    digits.parse().ok()
}

/// Extract a lowercase-hex `"key":"..."` string value and decode it to bytes.
fn json_hex_field(json: &str, key: &str) -> Option<Vec<u8>> {
    let needle = format!("\"{key}\"");
    let start = json.find(&needle)? + needle.len();
    let rest = &json[start..];
    let colon = rest.find(':')?;
    let after = rest[colon + 1..].trim_start().strip_prefix('"')?;
    let end = after.find('"')?;
    let hex = &after[..end];
    if !hex.len().is_multiple_of(2) {
        return None;
    }
    let mut out = Vec::with_capacity(hex.len() / 2);
    let bytes = hex.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let hi = (bytes[i] as char).to_digit(16)?;
        let lo = (bytes[i + 1] as char).to_digit(16)?;
        out.push(((hi << 4) | lo) as u8);
        i += 2;
    }
    Some(out)
}

/// The default sample recipe, shipped inline so the demo runs with no extra
/// file. RECIPE v1: an `alerter` that, ONLY for a service whose `up == false`,
/// runs BOTH paths in one rule —
/// - an `action` block EMITS `alert.notify` on the bus (broadcast to its topic
///   subscribers), interpolating the id and code; and
/// - a `ploxion` block DELIVERS DIRECTLY (targeted, point-to-point) to the
///   loaded `tracer` ploxion's `plc_on_event` — proving the new v1 kind reaches
///   exactly that ploxion WITHOUT a bus route (the tracer is NOT subscribed to
///   `service.health`, so it could only have learned of it via the targeted
///   delivery).
const SAMPLE_RECIPE: &str = r#"{
  "ploxion": "alerter",
  "blocks": [
    {"kind": "quand",   "ref": "service.health"},
    {"kind": "si",      "param": "up == false"},
    {"kind": "action",  "ref": "alert.notify", "param": "{id} is DOWN (code {code})"},
    {"kind": "ploxion", "ref": "tracer",       "param": "targeted: {id} DOWN (code {code})"}
  ]
}"#;

/// The **recipe** demo: the host EXECUTES a designer recipe on the bus.
///
/// Closes the triangle *designer (ploxion5) produces a recipe -> host executes it
/// -> bus connects ploxions*. We:
///
/// 1. parse a designer-shaped recipe JSON (inline sample, or a file passed as the
///    3rd arg) and COMPILE it into a reactive rule;
/// 2. register the rule as a **native bus participant** — `requires` its `quand`
///    trigger topic(s), `provides` its `action`/`sortie` topic(s), exactly like a
///    ploxion's manifest wires it. A WASM `tracer` is subscribed to the rule's
///    output topic so we SEE the emit get routed to a real ploxion;
/// 3. inject a MATCHING event (a DOWN service) and a NON-MATCHING event (an UP
///    service);
/// 4. show: matching -> condition true -> action emitted -> routed to the tracer;
///    non-matching -> condition false -> NOTHING emitted (droit au silence).
///
/// Exits 0 iff the rule fired exactly once (on the match), stayed silent on the
/// non-match, and its emit was routed to the subscriber.
fn cmd_recipedemo(dir: &Path, recipe_arg: Option<&str>) -> Result<()> {
    println!("============ XERB0XI0N-RT RECIPE RUNNER DEMO ============\n");
    println!(
        "  A designer recipe (ploxion5) COMPILED to a reactive rule and EXECUTED on the\n\
         \x20 bus as a native participant. It fires ONLY when its trigger matches AND its\n\
         \x20 condition holds — droit au silence otherwise.\n"
    );

    // --- 1. Load + compile the recipe. ---------------------------------------
    let (src, origin) = match recipe_arg {
        Some(path) => {
            let bytes = std::fs::read(path)
                .map_err(|e| anyhow::anyhow!("could not read recipe {path}: {e}"))?;
            (String::from_utf8_lossy(&bytes).into_owned(), path.to_string())
        }
        None => (SAMPLE_RECIPE.to_string(), "<inline sample>".to_string()),
    };
    let recipe = Recipe::from_json(src.as_bytes())
        .map_err(|e| anyhow::anyhow!("recipe JSON did not parse: {e}"))?;

    println!("== DESIGNER RECIPE ({origin}) ==");
    println!("  ploxion: {}", recipe.ploxion.as_deref().unwrap_or("<anon>"));
    for b in &recipe.blocks {
        let r = b.ref_.as_deref().unwrap_or("");
        let p = b.param.as_deref().unwrap_or("");
        println!(
            "    [{:<7}] ref={:<16} param={}",
            b.kind,
            if r.is_empty() { "—" } else { r },
            if p.is_empty() { "—" } else { p }
        );
    }
    println!();

    // --- 2. Build the host: a tracer (observer) + the compiled recipe. -------
    let mut host = Host::new();
    let tracer_path = dir.join("tracer.wasm");
    host.load_file(&tracer_path).map_err(|e| {
        anyhow::anyhow!(
            "could not load {} ({e}). Run scripts/build-ploxions.sh first.",
            tracer_path.display()
        )
    })?;
    host.init_all()?;

    let compiled = host.register_recipe(&recipe);
    // Subscribe the WASM tracer to the rule's output topic(s) so we SEE the
    // recipe's emit routed to a real ploxion (proof it lands on the bus).
    for topic in compiled.provides() {
        host.subscribe("tracer", topic);
    }

    println!("== COMPILED RULE (registered as native bus participant) ==");
    println!("  id       : {}", compiled.id);
    println!("  requires : {:?}   (the 'quand' trigger topic(s) it subscribes to)", compiled.requires());
    println!("  provides : {:?}   (the 'action'/'sortie' topic(s) it may emit — consent)", compiled.provides());
    println!(
        "  delivers : {:?}   (the 'ploxion' target id(s) it delivers to — point-to-point, NOT a bus topic)",
        compiled.delivers_to()
    );
    println!("  conditions:");
    for c in &compiled.conditions {
        println!("    - {} {} {}", c.field, op_render(c.op), c.value);
    }
    if !compiled.ignored.is_empty() {
        println!("  ignored  : {:?}", compiled.ignored);
    }
    println!();

    // --- 3. Inject a MATCHING and a NON-MATCHING event. ----------------------
    let trigger = compiled
        .requires()
        .first()
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("recipe has no trigger topic to inject"))?;

    let down_payload = br#"{"id":"repoverse","url":"https://repoverse.j0bot.ch","code":503,"up":false}"#;
    let up_payload = br#"{"id":"ideas-map","url":"https://ideas.j0bot.ch","code":200,"up":true}"#;

    println!("== INJECT #1: MATCHING event (a DOWN service) on [{trigger}] ==");
    println!("  {}", String::from_utf8_lossy(down_payload));
    let trace_before_match = host.trace().len();
    host.inject("host", &trigger, down_payload)?;
    print_trace_slice(&host, trace_before_match);
    println!();

    println!("== INJECT #2: NON-MATCHING event (an UP service) on [{trigger}] ==");
    println!("  {}", String::from_utf8_lossy(up_payload));
    let trace_before_nonmatch = host.trace().len();
    host.inject("host", &trigger, up_payload)?;
    print_trace_slice(&host, trace_before_nonmatch);
    println!();

    // --- 4. Tally: count what the rule did across the whole run. -------------
    let recipe_emits = host
        .trace()
        .iter()
        .filter(|t| matches!(t, Trace::Emit { from, .. } if from == &compiled.id))
        .count();
    let alert_topic = compiled.provides().first().map(|s| s.to_string()).unwrap_or_default();
    let routed_to_tracer = host
        .trace()
        .iter()
        .filter(|t| matches!(t, Trace::Deliver { to, topic, .. }
            if to == "tracer" && topic == &alert_topic))
        .count();
    let fired_with_interp = host.trace().iter().any(|t| matches!(t, Trace::Emit { from, topic, payload }
        if from == &compiled.id && topic == &alert_topic
            && payload.contains("repoverse") && payload.contains("503")));
    let stayed_silent_on_up = host.trace().iter().any(|t| matches!(t, Trace::Recipe { who, step }
        if who == &compiled.id && step.contains("false")));

    // RECIPE v1 — the 'ploxion' block: a TARGETED, point-to-point delivery to the
    // tracer's plc_on_event. It is traced as a deliver hop recipe -> tracer on the
    // trigger topic (NOT alert.notify, and NOT via any bus subscription — the
    // tracer was never subscribed to service.health).
    let target_id = compiled.delivers_to().first().map(|s| s.to_string()).unwrap_or_default();
    let targeted_deliver = host.trace().iter().any(|t| matches!(t, Trace::Deliver { from, to, topic }
        if from == &compiled.id && to == &target_id && topic == &trigger));
    // The targeted ploxion actually RAN its plc_on_event (it logged the payload
    // the recipe handed it — proof the delivery landed inside the sandbox).
    let target_reacted = host.trace().iter().any(|t| matches!(t, Trace::Log { from, line }
        if from == &target_id && line.contains("targeted") && line.contains("repoverse")));
    // POINT-TO-POINT guard: the recipe never created a bus subscription that
    // would have routed service.health to the tracer — the ONLY service.health
    // hop reaching the tracer is the targeted deliver above (from the recipe).
    let no_bus_route_for_target = !host.bus().subscribers(&trigger).iter().any(|s| s == &target_id);

    println!("== VERDICT ==");
    println!("  recipe fired (emits from the rule)   : {recipe_emits}   (expect 1 — only the DOWN match)");
    println!("  rule's emit routed to the tracer     : {routed_to_tracer}   (expect 1 — landed on the bus)");
    println!("  fired payload interpolated {{id}}/{{code}}: {fired_with_interp}");
    println!("  TARGETED deliver recipe -> '{target_id}'   : {targeted_deliver}   (v1 'ploxion' kind, point-to-point)");
    println!("  '{target_id}' ran plc_on_event (logged)    : {target_reacted}");
    println!("  '{target_id}' NOT bus-subscribed to trigger: {no_bus_route_for_target}   (proves point-to-point, not broadcast)");
    println!("  stayed SILENT on the UP event        : {stayed_silent_on_up}   (droit au silence)");

    host.shutdown()?;
    println!("\n  shutdown: plc_goodbye on each, all Stores + recipes dropped (droit au silence)\n");

    let ok = recipe_emits == 1
        && routed_to_tracer == 1
        && fired_with_interp
        && targeted_deliver
        && target_reacted
        && no_bus_route_for_target
        && stayed_silent_on_up;
    if ok {
        println!(
            "RESULT: OK — the designer recipe (RECIPE v1) was COMPILED to a reactive rule and\n\
             \x20       EXECUTED on the bus. For the DOWN service it ran BOTH paths in one rule:\n\
             \x20       (1) action -> EMIT alert.notify (broadcast, routed to a WASM ploxion), and\n\
             \x20       (2) ploxion -> TARGETED deliver to the tracer's plc_on_event (point-to-point,\n\
             \x20       no bus route). It stayed silent for the UP service. designer -> host -> bus."
        );
        Ok(())
    } else {
        Err(anyhow::anyhow!(
            "recipe demo incomplete (emits={recipe_emits}, routed={routed_to_tracer}, \
             interpolated={fired_with_interp}, targeted_deliver={targeted_deliver}, \
             target_reacted={target_reacted}, point_to_point={no_bus_route_for_target}, \
             silent_on_up={stayed_silent_on_up})"
        ))
    }
}

/// Render a recipe operator for the compiled-rule printout.
fn op_render(op: xerboxion_host::recipe::Op) -> &'static str {
    use xerboxion_host::recipe::Op;
    match op {
        Op::Eq => "==",
        Op::Ne => "!=",
        Op::Lt => "<",
        Op::Gt => ">",
        Op::Le => "<=",
        Op::Ge => ">=",
    }
}

/// Print every trace hop appended since index `from` (so each inject shows only
/// its OWN cascade — the routing + the recipe's decision steps).
fn print_trace_slice(host: &Host, from: usize) {
    for t in &host.trace()[from..] {
        println!("{}", t.render());
    }
}

/// The **map** subcommand: load the full example scene, run a representative
/// scenario, and emit a JSON snapshot (`--json`) or a human summary of the live
/// core (ploxions, bus wiring, event trace, services). The snapshot is the exact
/// payload `web-map/xerboxion-map.html` bakes in to render the core from
/// `file://`. The whole run is OFFLINE + deterministic (stubbed fetch/health), so
/// the snapshot is reproducible.
fn cmd_map(dir: &Path, as_json: bool) -> Result<()> {
    let commit = git_short_sha();
    let snap = map::build_snapshot(dir, &commit)?;

    if as_json {
        // The machine-readable snapshot — straight to stdout, nothing else, so it
        // can be piped/baked verbatim.
        println!("{}", snap.to_json_pretty());
        return Ok(());
    }

    // The human summary.
    println!("================ XERB0XI0N-RT LIVE MAP ================\n");
    println!("  core commit : {}", snap.core_commit);
    println!("  scenario    : {}\n", snap.scenario);

    println!("== PARTICIPANTS ({}) ==", snap.ploxions.len());
    for p in &snap.ploxions {
        let ver = if p.version.is_empty() {
            String::new()
        } else {
            format!(" v{}", p.version)
        };
        let caps = if p.capabilities.is_empty() {
            String::new()
        } else {
            format!("  caps={:?}", p.capabilities)
        };
        println!(
            "  [{:<6}] {:<16}{}  health={}{}",
            p.kind, p.id, ver, p.health, caps
        );
        if !p.provides.is_empty() {
            println!("           provides {:?}", p.provides);
        }
        if !p.requires.is_empty() {
            println!("           requires {:?}", p.requires);
        }
    }
    println!();

    println!("== BUS WIRING ({} topic edge(s)) ==", snap.bus.len());
    for e in &snap.bus {
        let from = if e.from.is_empty() { "(external)" } else { &e.from };
        println!("  [{}]  {}  ->  {:?}", e.topic, from, e.to);
    }
    println!();

    println!("== EVENT TRACE ({} hop(s)) ==", snap.trace.len());
    for r in &snap.trace {
        // Re-render compactly per kind (mirrors Trace::render's spirit).
        let line = match r.kind.as_str() {
            "emit" => format!("emit   {} -> [{}] {}", r.from, r.topic, r.payload),
            "route" => format!("route  [{}] {} {}", r.topic, r.from, r.note),
            "log" => format!("log    {}: {}", r.from, r.payload),
            "life" => format!("life   {}: {}", r.from, r.payload),
            "fetch" => format!("fetch  {} -> {} ({})", r.from, r.payload, r.note),
            "recipe" => format!("recipe {}: {}", r.from, r.payload),
            other => format!("{other:<6} {} {}", r.from, r.payload),
        };
        println!("  {:>3}  {line}", r.seq);
    }
    println!();

    println!("== SERVICES ({}) ==", snap.services.len());
    for s in &snap.services {
        let label = if s.up { "UP  " } else { "DOWN" };
        println!("  {:<12} {} code={:03}  {}", s.id, label, s.code, s.url);
    }
    println!();

    println!(
        "RESULT: OK — live core mapped: {} participant(s), {} bus edge(s), {} trace hop(s), {} service(s).",
        snap.ploxions.len(),
        snap.bus.len(),
        snap.trace.len(),
        snap.services.len()
    );
    println!("  (run `xerboxion-rt map {} --json` for the snapshot the web map bakes in.)", dir.display());
    Ok(())
}

/// The **record** subcommand — the project thesis, reflexive.
///
/// Runs the SAME representative scene as `map` (ping/pong/tracer/watcher/
/// health-adapter/state-client/tsoin + an alerter recipe + a service sweep), but
/// instead of deriving a snapshot it records the core's OWN ordered bus trace as a
/// **tsoin timeline** through the real engine, then:
///
/// 1. prints engine-derived stats (frames, raw/stored bytes, collapse Nx, blob
///    count, xerboxions, Merkle/timeline root);
/// 2. REPLAYS the timeline and asserts the reconstructed frames are BIT-EXACT to
///    the originally recorded events (the exact ordered event stream comes back);
/// 3. FORKS the timeline, records one more event on the branch, and shows the
///    branch diverges while the original head is intact.
///
/// Exits non-zero if replay is NOT bit-exact or the fork does not diverge — the
/// claim is never papered over. The whole run is OFFLINE + deterministic.
fn cmd_record(dir: &Path) -> Result<()> {
    println!("============== XERB0XI0N-RT BUS-AS-TSOIN RECORD ==============\n");
    println!(
        "  thesis: « tout est un tsoin ». The core's life is its ordered bus trace.\n\
         \x20         We record that trace, frame by frame, as a tsoin timeline through\n\
         \x20         the REAL engine — replayable bit-exact, forkable, with measured\n\
         \x20         surprise/collapse. Reflexive: the machine à tsoins records itself.\n"
    );

    // 1. Run the representative scene; keep the LIVE host (trace intact).
    let scene = map::run_scene(dir)?;
    let trace = scene.host.trace();
    println!("== SCENE: {} ==\n", map::SCENARIO);
    println!("  the core lived {} bus event(s) this run.\n", trace.len());

    // 2. RECORD the ordered trace as a tsoin timeline via the real engine.
    let bt = recorder::record_trace(trace);
    let st = recorder::stats(&bt);

    let magic = String::from_utf8_lossy(recorder::FRAME_MAGIC);
    println!("== RECORDED AS A TSOIN TIMELINE (real engine: Recorder + Timeline) ==");
    println!("  each bus event => one canonical frame ('{magic}' magic, length-prefixed");
    println!("  {{seq,kind,from,topic,payload,note}}) recorded as the next instant; the");
    println!("  engine stores only the DELTA from the previous frame (temporal residue).\n");

    println!("== STATS (read straight from the engine, never hand-computed) ==");
    println!("  frames (= bus events)        : {}", st.frames);
    println!("  raw bytes (naive, full)      : {}", st.raw_bytes);
    println!("  stored raw (delta blobs)     : {}", st.stored_raw);
    println!("  stored zstd (per-blob)       : {}", st.stored_compressed);
    println!("  stored zstd (joint stream)   : {}", st.stored_joint);
    println!("  collapse (raw / joint)       : {:.2}x", st.collapse_ratio);
    println!("  distinct delta blobs         : {}", st.blob_count);
    println!("  xerboxions (exact repeats)   : {}", st.xerboxions);
    println!("  Merkle / timeline ROOT       : {}", st.root_hex);
    println!();

    // 3. REPLAY the timeline and assert BIT-EXACT against what was recorded.
    println!("== REPLAY (reconstruct every frame from the Merkle DAG) ==");
    let replayed = recorder::replay(&bt);
    let bit_exact = replayed == bt.frames;
    // Decode the reconstructed frames back into the ordered event stream and show
    // the head/tail so the reflexive claim is VISIBLE, not just asserted.
    let events = recorder::replay_events(&bt)
        .ok_or_else(|| anyhow::anyhow!("a replayed frame failed to decode — corruption"))?;
    let decode_matches = events
        == trace
            .iter()
            .enumerate()
            .map(|(i, t)| RecordedEvent::from_trace(i, t))
            .collect::<Vec<_>>();
    let show = events.len().min(8);
    println!("  reconstructed {} frame(s); first {} of the EXACT ordered stream:", events.len(), show);
    for ev in events.iter().take(show) {
        println!("   {:>3}  {}", ev.seq, ev.render());
    }
    if events.len() > show {
        println!("   ...  ({} more)", events.len() - show);
    }
    println!();
    println!("  replay bit-exact (bytes)     : {}", yesno(bit_exact));
    println!("  decoded stream == original   : {}", yesno(decode_matches));
    println!();

    // 4. FORK the timeline; record one more event on the branch; show divergence.
    println!("== FORK (git-of-states: free branch, shared past) ==");
    let original_head = bt
        .timeline
        .head()
        .ok_or_else(|| anyhow::anyhow!("empty timeline — the scene produced no events"))?;
    let original_frames = recorder::replay(&bt);
    let blobs_before = bt.recorder.blob_count();

    let mut bt = bt; // make mutable for the branch record
    let mut branch = recorder::fork(&bt);
    let branch_event = RecordedEvent {
        seq: trace.len() as u64,
        kind: "emit".into(),
        from: "branch".into(),
        topic: "branch.divergence".into(),
        payload: "a what-if event recorded only on the fork".into(),
        note: String::new(),
    };
    recorder::record_onto(&mut bt, &mut branch, &branch_event);

    let branch_head = branch.head().expect("branch recorded a frame");
    let original_intact = bt.timeline.head() == Some(original_head)
        && recorder::replay(&bt) == original_frames;
    let branch_diverged = branch_head != original_head;
    let branch_frames = branch.frames(&bt.recorder);
    let branch_grew_by_one = branch_frames.len() == original_frames.len() + 1;
    // Fork is FREE: only the one genuinely-new delta was added to the store.
    let only_one_new_blob = bt.recorder.blob_count() == blobs_before + 1;

    println!("  original head (root)         : {}", original_head.to_hex());
    println!("  branch   head (root)         : {}", branch_head.to_hex());
    println!("  branch recorded extra event  : {}", branch_event.render());
    println!("  branch diverged from original: {}", yesno(branch_diverged));
    println!("  original intact (head+replay): {}", yesno(original_intact));
    println!(
        "  branch has +1 frame ({} -> {}): {}",
        original_frames.len(),
        branch_frames.len(),
        yesno(branch_grew_by_one)
    );
    println!("  fork was free (+1 delta blob): {}", yesno(only_one_new_blob));
    println!();

    // 5. VERDICT.
    let ok = bit_exact && decode_matches && branch_diverged && original_intact && branch_grew_by_one;
    println!("== VERDICT ==");
    println!("  frames recorded              : {}", st.frames);
    println!("  collapse                     : {:.2}x", st.collapse_ratio);
    println!("  tsoin root                   : {}", st.root_hex);
    println!("  replay bit-exact             : {}", yesno(bit_exact));
    println!("  fork ok                      : {}", yesno(branch_diverged && original_intact));
    println!();

    if !ok {
        return Err(anyhow::anyhow!(
            "record FAILED: bit_exact={bit_exact}, decode_matches={decode_matches}, \
             branch_diverged={branch_diverged}, original_intact={original_intact}, \
             branch_grew_by_one={branch_grew_by_one}"
        ));
    }

    println!(
        "RESULT: OK — the core recorded its own {} events as a tsoin; replay is\n\
         \x20       bit-exact; the history is forkable.",
        st.frames
    );

    // Clean shutdown of the scene host (droit au silence).
    let mut host = scene.host;
    host.shutdown()?;
    Ok(())
}

/// The **serve** subcommand — the PERSISTENT DAEMON (Phase A).
///
/// Where every other subcommand is ONE-SHOT (build a host, run a scene, exit),
/// this one starts a long-running daemon that loads `dir`, inits the fleet, and
/// exposes a LIVE bus API so external things can connect, watch the bus, and emit
/// onto it. The `Host` lives on a dedicated thread (wasmtime Stores are not
/// `Sync`); an async axum server reaches it only through channels. See
/// [`xerboxion_host::serve`] for the architecture. Blocks until SIGINT.
///
/// Flags: `--port N` (default 8730), `--addr A` (default 127.0.0.1). The first
/// positional non-flag arg after `serve` is the ploxion dir (already parsed into
/// `dir` by `main`).
fn cmd_serve(dir: &Path, args: &[String]) -> Result<()> {
    let port = flag_value(args, "--port")
        .map(|v| {
            v.parse::<u16>()
                .map_err(|_| anyhow::anyhow!("invalid --port '{v}' (expected 1..=65535)"))
        })
        .transpose()?
        .unwrap_or(xerboxion_host::serve::DEFAULT_PORT);
    let addr = flag_value(args, "--addr")
        .unwrap_or_else(|| xerboxion_host::serve::DEFAULT_ADDR.to_string());

    // --- BUS FEDERATION flags (all opt-in; absent = classic single-node). ----
    let node_id = flag_value(args, "--node-id")
        .unwrap_or_else(xerboxion_host::serve::default_node_id);
    let peer_token = flag_value(args, "--peer-token");
    // `--peer <url>` is REPEATABLE: collect every occurrence. An optional
    // `--peer-token` applies to all of them (the shared link secret).
    let peers: Vec<xerboxion_host::federation::PeerSpec> = flag_values(args, "--peer")
        .into_iter()
        .map(|url| xerboxion_host::federation::PeerSpec { url, token: peer_token.clone() })
        .collect();

    println!("================ XERB0XI0N-RT SERVE (daemon) ================\n");
    println!(
        "  Loading ploxions from {} and starting the live bus daemon (node «{node_id}»)…\n",
        dir.display()
    );
    if !peers.is_empty() {
        println!("  BUS FEDERATION on: {} peer(s) to dial.\n", peers.len());
    }
    // --- DURABLE persistence (opt-in via --state-dir). -----------------------
    // When set, ploxions hot-loaded via POST /load are persisted to this dir and
    // restored on the next startup (they survive a deploy/crash). Absent = the
    // feature is OFF (the runtime-loaded delta is ephemeral, as before).
    let state_dir: Option<PathBuf> = flag_value(args, "--state-dir").map(PathBuf::from);
    if let Some(sd) = &state_dir {
        println!(
            "  DURABLE persistence on: runtime-loaded ploxions saved to {} (restored on restart).\n",
            sd.display()
        );
    }

    // --- UNIVERSAL CONSTRUCTOR (von Neumann) : reconstruct from a peer. --------
    // `--reconstruct-from <peer>`: BEFORE booting, fetch the peer's GET /replicate
    // bundle and materialize it into our --state-dir, so the subsequent boot
    // RESTORES an identical runtime-loaded delta — a fresh node becomes a COPY of
    // the peer (the self-reproducing « pourriture 4 »). Requires --state-dir (a
    // place to write the reconstructed delta). The base set still loads from this
    // node's own image; only the delta + (later) data tsoins travel.
    if let Some(peer) = flag_value(args, "--reconstruct-from") {
        let sd = state_dir.as_deref().ok_or_else(|| {
            anyhow::anyhow!("--reconstruct-from requires --state-dir (where to write the reconstructed delta)")
        })?;
        let n = reconstruct_from(&peer, sd)?;
        println!(
            "  RECONSTRUCTED {n} runtime ploxion(s) from {peer} into {} (von Neumann: a copy of the peer).\n",
            sd.display()
        );
    }

    let commit = git_short_sha();
    let fed = xerboxion_host::serve::FederationConfig { node_id, peers, peer_token };
    xerboxion_host::serve::run_fed(dir, &addr, port, commit, fed, state_dir.as_deref())
}

/// Fetch a peer's `GET /replicate` bundle and materialize it into `state_dir` —
/// the universal constructor's CLIENT side. `peer` may be the node base URL
/// (`https://xion.j0bot.ch`) or the full `/replicate` URL. The bundle is bounded
/// (64 MiB) so a hostile peer cannot OOM us. Returns the number of ploxions
/// written into the delta (which the subsequent boot restores).
fn reconstruct_from(peer: &str, state_dir: &Path) -> Result<usize> {
    let base = peer.trim_end_matches('/');
    let url = if base.ends_with("/replicate") {
        base.to_string()
    } else {
        format!("{base}/replicate")
    };
    let mut resp = ureq::get(&url).call().with_context(|| format!("GET {url}"))?;
    let status = resp.status().as_u16();
    if status != 200 {
        anyhow::bail!("reconstruct: GET {url} returned HTTP {status}");
    }
    let bytes = resp
        .body_mut()
        .with_config()
        .limit(64 * 1024 * 1024)
        .read_to_vec()
        .with_context(|| format!("reading replica bundle from {url}"))?;
    let bundle: xerboxion_host::replicate::ReplicaBundle =
        serde_json::from_slice(&bytes).context("parsing replica bundle")?;
    let store = xerboxion_host::state::StateStore::open(state_dir)
        .with_context(|| format!("opening state dir {}", state_dir.display()))?;
    xerboxion_host::replicate::import(&store, &bundle).context("importing replica bundle")
}

/// Read the value following a `--flag` in the argv (e.g. `--port 9000`). Returns
/// `None` if the flag is absent or has no following value.
fn flag_value(args: &[String], flag: &str) -> Option<String> {
    args.iter()
        .position(|a| a == flag)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

/// Read EVERY value following a REPEATABLE `--flag` (e.g. `--peer A --peer B`).
/// Returns them in order; empty if the flag never appears.
fn flag_values(args: &[String], flag: &str) -> Vec<String> {
    args.iter()
        .enumerate()
        .filter(|(_, a)| a.as_str() == flag)
        .filter_map(|(i, _)| args.get(i + 1).cloned())
        .collect()
}

/// `true`/`false` -> a stable `yes`/`NO` verdict token for the record output.
fn yesno(b: bool) -> &'static str {
    if b {
        "yes"
    } else {
        "NO"
    }
}

/// The short git sha of the core (best-effort). Resolves the worktree this binary
/// was run from; falls back to `"unknown"` if git is unavailable (e.g. a build
/// from a tarball) so the snapshot always has a value.
fn git_short_sha() -> String {
    std::process::Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown".to_string())
}
