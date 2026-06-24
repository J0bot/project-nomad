# xerboxion-rt — the operational xerboxion-core

The **connection fabric** of the ploxion ecosystem: a Rust + `wasmtime` host
that loads every ploxion as an isolated **WASM** module, wires them together
over the **XERB0XI0N bus**, and drives their lifecycle under the **PLC v1**
contract.

> Mission (José, 2026-06-18): *"finir le xerboxion-core pour que tous les
> services et ploxions soient connectés à ce xerboxion"*, et *"tout peut être en
> WASM pour les ploxions."* This repo is that operational core. (The bare-metal
> kernel `~/xerboxion-core` is a separate long-horizon substrate — untouched.)

## État — où on en est (2026-06-20)

Tableau de bord pour José. Le core OS entier = **~15 Mo / 16 Go** (`scripts/core-size.sh`), **26 ploxions**, bus LIVE.

**Live** : daemon `xion.j0bot.ch` (= `10.0.0.1:8730`, systemd + Traefik). Endpoints `/healthz /emit /events(SSE) /load /unload /snapshot /replicate /paper`.

**La grammaire incarnée** : `bion → cubion → ploxion → xion`. Les **bions** (code partagé) vivent dans `ploxions/sdk/src/bions.rs` (json_*/fnv1a64/to_hex/tsoin_record/decode_event/…). **uncraft** = diviser un ploxion jusqu'à retrouver le bion d'un autre = la lib compound.

**Chantiers ouverts** :
- **Blocs du jeu** (campagne Minecraft) : `block-bion` (`sdk/src/block.rs` : `BlockDef` résidu + macro `block_ploxion!`) + `flow-bion` ; `block-stone`/`block-water` LIVE (cycle place→tsoin→placed→flow prouvé). Chaque bloc = ~15 lignes de résidu.
- **Machine à tsoins** : substrat fait (record + adressage génératif en morceaux : carte/kion/spectre/xerbion). Organes manquants = `docs/forge-feed.json` : **tsoin-store → diff → generator → clock-coherence → player** (fermer la boucle record→adresse→rejoue→regénère).
- **La boucle infinie de création** (`docs/boucle-creation-infinie.md`) : créer = chaos × amour ; STATE→taper le chaos→sélectionner→créer→uncraft→extrapoler→mesurer(16Go)→graver→**monter**. Deux opérateurs : **CLIMB** (améliorer) + **JUMP** (sortir du plateau = ajouter une dimension hors-span).
- **Le premier xerbion** : `xerbion` (MLP prédictif = codage prédictif mesuré, résidu sinus 0.815→0.009) + `xerbion-bit` (ternaire/BitNet, plafonne = 1-bit a besoin d'échelle).
- **Taps autonomes** : `chaos-tap` → file CLIMB (`forge-feed.json`) ; `jump-tap` → file JUMP (`jump-feed.json`, #1 = **iris/vision** : le pixel entre dans le xion via `gpu.infer`, cahier + 3080).
- **Page Turing** (`web-turing/`) : interprète + **compilateur Brainfuck→WASM dans le navigateur** (zéro serveur). Substrat self-hosting → unlock RepoVerse. `node web-turing/test.cjs` le prouve.
- **Ploxion maison** (objectif impossible en cours) : maison autonome = système de feature-ploxions (cubions) ; `maison-bion` + énumération A→Z.

**Limite dure** : tout l'OS (runtime + ploxions + bions, hors data/photos) ≤ **16 Go** ; dépassement → redescendre (uncraft/compress). `scripts/core-size.sh`.

**Lancer / tester** : `./run-pc.sh` (build + serve les 26 ploxions sur `127.0.0.1:8730`) — voir `RUN-ON-PC.md`. Empreinte : `scripts/core-size.sh`.

## What it is

- **PLC v1** (`crates/xerboxion-plc` + `docs/PLC-v1.md`): the freezable WASM ABI
  a ploxion must satisfy — the "software CFC".
- **The host** (`crates/xerboxion-host`, bin `xerboxion-rt`): loads N `.wasm`
  ploxions, each in its **own** `wasmtime::Store` (isolation), reads its
  manifest, runs `plc_init`, maintains the bus (topic → subscribers from
  manifests), ROUTES + TRACES `plc_emit` to subscribers' `plc_on_event`
  (never filters), sweeps `plc_health`, and on shutdown calls `plc_goodbye`
  then drops every Store (*droit au silence* — no residual).
- **Registry**: reads the runtime session's `ploxi0ns.json` (read-only) as the
  known-ploxion catalogue; the host's loaded set is the live registry.
- **Example ploxions** (`ploxions/{ping,pong,tracer}`, compiled to
  `wasm32-unknown-unknown`): prove the fabric end-to-end.
- **tsoin state ploxion** (`ploxions/tsoin` + `ploxions/state-client`): a real,
  useful ploxion — the **versioning / state layer**. `tsoin` wraps the actual
  `tsoin` record/replay engine (built for wasm32, `default-features` off so the
  native-only zstd/clap parts are excluded) and exposes it on the bus: on
  `tsoin.record {name, bytes}` it stores a new state as an XOR-delta in a
  content-addressed BLAKE3 timeline (its OWN sandbox); on `tsoin.replay {id}` it
  reconstructs that state **bit-exact**. `state-client` records 3 evolving states,
  then replays an old one and checks the bytes come back byte-for-byte identical —
  across the bus + two WASM boundaries.
- **Service connector** (`crates/xerboxion-host/src/connector.rs`, native) +
  **watcher ploxion** (`ploxions/watcher`, WASM): connect the **LIVE deployed
  services** to the bus. The connector is a **native host-side adapter** (see
  below) — it reads `runtime.json` (read-only) for the `deployed:true` services
  and their health URLs, performs a real **HTTP GET** against each (via `ureq`),
  and emits a `service.health {id,url,code,up}` event per service onto the bus as
  the native participant `service-connector`. The sandboxed `watcher` ploxion
  `requires` `service.health`, logs each service UP/DOWN, counts them, and emits
  an `alert` for any that are down — a real WASM ploxion **reacting to the live
  deployed services** through the bus.
- **Capability layer** (`docs/PLC-v1.md §3.1/§4.1`, `ploxions/health-adapter`):
  PLC **v1.1** adds consented `capabilities`; the host links `plc_fetch` only into
  ploxions that declare `net.fetch`. A service adapter can now be a **pure WASM
  ploxion** (least authority). `cargo run -- capdemo`.
- **Recipe runner** (`src/recipe.rs`, `docs/RECIPE-v1.md`): the host **executes a
  designer recipe** on the bus — script ploxion connections without Rust. RECIPE
  **v1** (frozen) adds the `ploxion` kind: a TARGETED, point-to-point delivery to
  one ploxion's `plc_on_event`. `cargo run -- recipedemo`.
- **Live map** (`src/map.rs`, `web-map/xerboxion-map.html`): the host's real state
  as a JSON snapshot + a self-contained page — the core, *visible*.
  `cargo run -- map`.
- **Bus-as-tsoin** (`src/recorder.rs`): the core records its **own** ordered bus
  trace as a real tsoin timeline — replayable bit-exact, forkable. `cargo run --
  record`. (*Tout est un tsoin*, applied to the runtime itself.)

## Two ways real I/O enters the bus (native adapter OR consented WASM)

The host is **native Rust** and can do real I/O (network, files, …). WASM
ploxions are **sandboxed pure compute** in an isolated `Store`. There are now
**two** disciplined ways the outside world reaches the bus:

**(A) Native adapter** — trusted host code does the I/O and bridges the result
onto the bus. The `service-connector` is the first: it registers as a native
participant that `provides ["service.health"]`, reads `runtime.json`, does real
HTTP, and emits per service.

```
  the real world                native host                    WASM sandbox
 (HTTP / files / …) ──I/O──▶  service-connector  ──emit──▶  bus ──▶ watcher (pure)
                            (native adapter, ureq)        service.health   reacts
```

**(B) Consented capability (PLC v1.1)** — a ploxion **declares** a capability in
its manifest (`capabilities: ["net.fetch"]`) and the host links the matching host
import (`plc_fetch`) into **only that ploxion's** `Store`. A ploxion that does not
declare it has **no** such import (least authority; if its wasm references it, the
host rejects it at load). So a service adapter can be a **pure sandboxed WASM
ploxion** — `ploxions/health-adapter` declares `net.fetch`, does real `plc_fetch`
HTTP, and emits `service.health` exactly like the native connector, but from
inside the sandbox.

```
  health-adapter (WASM, declares net.fetch) ──plc_fetch──▶ host I/O ──▶
       └── emit service.health ──▶ bus ──▶ watcher (reacts)
```

Use (A) for trusted broad host access, (B) for scoped, consent-checked access
that keeps the adapter sandboxed. See `docs/PLC-v1.md §3.1, §4.1`.

## Scripting connections without Rust — the recipe runner

The designer (ploxion5's block editor) exports a **recipe** —
`{ploxion, blocks:[{kind,label,ref,param}]}` with kinds
`quand/si/action/sortie/entrée/attendre`. The host **compiles a recipe into a
reactive rule and runs it on the bus** as a participant (no Rust, no recompile):
a `quand` block is its trigger topic (`requires`), `si` blocks are payload
predicates, `action`/`sortie` blocks `emit` (`provides`) with `{field}`
interpolation, and (RECIPE **v1**) a `ploxion` block delivers DIRECTLY to one
ploxion's `plc_on_event` (targeted, not a bus broadcast). This closes the loop
**designer → recipe → host → bus**. See the FROZEN contract `docs/RECIPE-v1.md`;
run `cargo run --bin xerboxion-rt -- recipedemo`.

## Seeing the core — the live map

`cargo run --bin xerboxion-rt -- map [--json]` dumps the host's **real** state
(loaded ploxions + manifests, bus wiring, the event trace, services, and this
run's `tsoin_root`). `web-map/xerboxion-map.html` is a **self-contained** page
(inline, `file://`, zero network, responsive) that renders that snapshot as a
node-graph of the bus + the trace + services — so the core is *visible* by
double-clicking a file. (Delivered to José at
`/bi0ns/xion_OUTPUT/xerboxion-map/index.html`.)

This host is **complementary** to the runtime session's read-only `boxion` CLI:
`boxion` does discovery + health-check over deployed services; `xerboxion-rt`
actually loads and connects WASM ploxions in-process — and, via the connector,
bridges the live deployed services onto that same in-process bus.

## The live bus daemon — `serve`

`cargo run --bin xerboxion-rt -- serve` turns the same host into a long-running
HTTP daemon exposing the **live bus** so external things can watch every event as
it flows and emit onto it. Endpoints:

- `GET /` — the self-contained 2D live map. `GET /3d` — the 3D/4D view.
- `GET /snapshot` — the host's real state as JSON. `GET /ecosystem` — the whole
  computer (running + registered + deployed). `GET /healthz` — liveness.
- `POST /emit {topic,payload}` — inject onto the real bus.
- **`POST /load`** — **HOT-LOAD a ploxion into the running core, no restart.** Body
  is EITHER `{"id":"<registered-id>"}` (loads the staged `target/ploxions/<id>.wasm`)
  OR `{"wasm":"<base64>"}` (an arbitrary wasm module, with optional `{"id"}` to name
  it). The daemon instantiates a fresh `Store`, reads its manifest, WIRES it to the
  bus (its `requires` → it now receives those topics; its `provides` → routable),
  runs `plc_init`, and traces a `load`/`init` lifecycle pair (visible on `/events` +
  `/ecosystem`). Returns `200` with the loaded manifest as JSON. Clean errors, never
  a panic: `400` (malformed body / neither field / bad base64 / id with a path
  separator), `404` (`{id}` with no staged wasm), `422` (invalid wasm / missing
  required export / rejected manifest / **duplicate id already loaded**). After a
  load the ploxion shows `running` in `/snapshot` + `/ecosystem` and reacts on the
  bus immediately.
- **`POST /unload {"id":"<loaded-id>"}`** — **HOT-UNLOAD by id.** Runs `plc_goodbye`,
  removes its bus wiring (so it receives nothing afterwards), and drops its `Store`
  (*droit au silence*). Traces a `goodbye`/`unload` lifecycle pair. Returns `200`
  (`{"unloaded":"<id>"}`), or `404` if no ploxion with that id is loaded. The
  ploxion then disappears from `/snapshot` (or reverts to `registered` in
  `/ecosystem` if it is a registry id).

  ```bash
  # hot-load a registered ploxion, watch it react, then unload it — no restart
  curl -X POST http://127.0.0.1:8730/load   -d '{"id":"pong"}'      # 200 + manifest
  curl -X POST http://127.0.0.1:8730/emit   -d '{"topic":"ping","payload":"hi"}'
  #   -> /events shows: route ping => pong, emit pong [pong] pong#1
  curl -X POST http://127.0.0.1:8730/unload -d '{"id":"pong"}'      # 200, goodbye + dropped
  curl -X POST http://127.0.0.1:8730/emit   -d '{"topic":"ping","payload":"hi"}'
  #   -> /events shows NO route to pong (silence)
  ```

  The Host (which owns every wasmtime `Store`, not `Sync`) is touched ONLY on the
  dedicated host thread: the async handler resolves the bytes, sends a `Load`/
  `Unload` command, and awaits a oneshot result — same one-owner invariant `/emit`
  and `/snapshot` use.

### Durable hot-loads — `--state-dir` (the runtime-loaded delta survives a restart)

By default the runtime-loaded set is **ephemeral**: a ploxion you `POST /load` into
a running daemon is gone after a deploy/crash/restart (the base set in
`<ploxions_dir>` always reloads, but your hot-loads do not). Start the daemon with
**`--state-dir <path>`** to make those hot-loads **DURABLE**:

```bash
cargo run --bin xerboxion-rt -- serve target/ploxions --state-dir ./xion-state
```

- On each successful **`POST /load`** the daemon persists that ploxion to the state
  dir; on **`POST /unload`** it removes it. Only the **runtime delta** is saved —
  **never the startup base set** (that always reloads from `<ploxions_dir>`, so
  persisting it would double-load).
- On the **next startup** the daemon loads the base set as usual, then **restores**
  each persisted runtime ploxion automatically — so a previously hot-loaded ploxion
  is loaded again and reacts on the bus, with no manual step.
- A `{"id":"…"}` (staged) load persists only the id; its bytes reload from
  `<ploxions_dir>/<id>.wasm`. A `{"wasm":"<base64>"}` upload persists the **wasm
  bytes** too (they live nowhere else), so even an uploaded module survives.

State dir layout (atomic writes — temp file + rename; the manifest is the source of
truth):

```text
<state-dir>/
  loaded.json          # {"version":1,"loaded":[{"id":"…","source":"staged"|"wasm"}, …]}
  loaded/<id>.wasm     # the persisted bytes, only for source = "wasm" records
```

**Graceful by contract.** No `--state-dir` = feature off (behaves exactly as before).
A missing state dir = nothing to restore. A corrupt/partial state dir (bad JSON,
truncated/absent wasm, a now-duplicate id, a staged wasm that vanished) is **skipped
with a `WARN`** — the daemon still boots its base set, never panics. Persistence is
**best-effort**: a persist/remove I/O error is logged but never fails the live
`/load`/`/unload`.

```bash
# durability in one shot: load a ploxion, kill the daemon, restart, it is back
cargo run --bin xerboxion-rt -- serve target/ploxions --state-dir ./xion-state &
curl -X POST http://127.0.0.1:8730/load -d '{"id":"pong"}'   # 200, persisted
kill -9 %1                                                   # hard crash
cargo run --bin xerboxion-rt -- serve target/ploxions --state-dir ./xion-state &
#   boot log: "restored runtime ploxion 'pong' from state dir"
curl http://127.0.0.1:8730/snapshot | grep pong             # pong is loaded again
```
- **`GET /ws`** — a **WebSocket** (`101` upgrade) streaming every bus hop as JSON
  (`{seq,kind,from,topic,payload,note}`) and accepting inbound `{emit:{...}}`.
- **`GET /events`** — the **SAME live feed over plain HTTP** as **Server-Sent
  Events** (`text/event-stream`, a normal `200` chunked stream, no upgrade). It
  subscribes to the same broadcast channel as `/ws`, so it carries identical
  events; each hop is one SSE `data:` line with the same JSON shape (SSE `event:`
  = the hop kind, `id:` = the `seq`), plus a periodic keep-alive comment. Because
  it is plain chunked HTTP (and sets `X-Accel-Buffering: no`), it is trivially
  proxyable server-side — e.g. a PHP fleet relay can forward it with any HTTP
  client, no WebSocket sidecar. Read it straight with curl:

  ```bash
  curl -N http://127.0.0.1:8730/events
  ```

## Run it

```bash
# from /home/debian/xerboxion-rt
source "$HOME/.cargo/env"

# build the example WASM ploxions + run the full demo (ping -> pong trace)
./scripts/demo.sh

# or, step by step:
./scripts/build-ploxions.sh                 # compile ploxions to wasm, stage them
cargo run --bin xerboxion-rt -- ls          # registry + manifests + bus wiring
cargo run --bin xerboxion-rt -- status      # + a live health sweep
cargo run --bin xerboxion-rt -- demo        # drive ticks, print ping->pong trace
cargo run --bin xerboxion-rt -- tsoin       # record 3 states, replay an old one BIT-EXACT
cargo run --bin xerboxion-rt -- services    # LIVE deployed services -> bus -> WASM watcher (native adapter)
cargo run --bin xerboxion-rt -- capdemo     # pure-WASM health-adapter fetches via CONSENTED plc_fetch -> bus
cargo run --bin xerboxion-rt -- recipedemo  # the host EXECUTES a designer recipe on the bus
cargo run --bin xerboxion-rt -- map --json  # snapshot of the core's real state (-> web-map/xerboxion-map.html)
cargo run --bin xerboxion-rt -- record      # record the core's OWN bus trace as a TSOIN,
                                            # replay it bit-exact, fork it (the thesis, reflexive)

# tests + lint
cargo test
cargo clippy --all-targets -- -D warnings
(cd ploxions && cargo clippy --all-targets --target wasm32-unknown-unknown -- -D warnings)
```

`demo` exits non-zero if the events don't flow; `tsoin` exits non-zero on any
byte mismatch (or if the record→replay round trip never completes); `services`
does real HTTP against the live deployed services and exits non-zero unless the
sweep ran and every `service.health` event was delivered into the watcher.
(`cargo test` stays fully offline/deterministic — the live network is only
exercised by the `services` subcommand, never by the tests.)

`record` is the project thesis — *« tout est un tsoin »* — applied to the runtime
itself: it runs the representative scene, then records the core's OWN ordered bus
trace (every `Emit`/`Route`/`Log`/`Lifecycle`/`Fetch`/`Recipe` hop) as a real
[tsoin](../tsoin) timeline. Each event becomes one canonical frame
(`{seq,kind,from,topic,payload,note}`); the engine stores only the delta from the
previous frame, so the timeline collapses (a real, zstd-measured ratio read back
out of the engine). It then **replays** the timeline and asserts the frames come
back bit-exact, and **forks** it — recording one more event on the branch to show
the history diverges while the original stays intact. Exits non-zero if replay is
not bit-exact or the fork does not diverge; the verdict is never faked. The live
`map --json` also stamps this run's `tsoin_root` (the same Merkle root) so the
viewer can show the core's own tsoin identity.

## Layout

```
crates/xerboxion-plc     PLC v1 types + ABI constants (shared, freezable)
crates/xerboxion-host    the wasmtime host: loader, bus, lifecycle, CLI
   src/connector.rs      NATIVE service connector: runtime.json + real HTTP -> bus
   src/fetch.rs          the gated plc_fetch host import (capability net.fetch)
   src/recipe.rs         compiles a designer recipe into a reactive bus rule
   src/map.rs            snapshots the host's real state (ploxions/bus/trace/services)
   src/recorder.rs       records the core's OWN bus trace as a real tsoin timeline
                         (canonical frame per event; replay bit-exact; free fork)
ploxions/sdk             tiny SDK for writing WASM ploxions (alloc/emit/log/fetch)
ploxions/{ping,pong,tracer}  example ploxions, compiled to wasm32-unknown-unknown
ploxions/tsoin           the versioning/state layer ploxion (wraps the real tsoin engine)
ploxions/state-client    drives tsoin: records states, replays + checks bit-exact
ploxions/watcher         WASM ploxion reacting to service.health (live deployed services)
ploxions/health-adapter  PURE-WASM service adapter (declares net.fetch, real plc_fetch -> bus)
web-map/xerboxion-map.html   self-contained live map of the core (file://, zero network)
docs/PLC-v1.md           the contract spec (incl. v1.1 capabilities)
docs/RECIPE-v1.md        the FROZEN recipe <-> bus execution contract (designer integration)
scripts/                 build-ploxions.sh, demo.sh, build-map.sh
```

See `docs/PLC-v1.md` for the exact exports/imports a ploxion module must satisfy,
and `docs/RECIPE-v1.md` for how the host executes a designer recipe.
