# PLC v1 — the PL0XI0N Link Contract (WASM ABI)

> The "software CFC": once frozen, these names and shapes do not change within
> v1. A ploxion compiled against PLC v1 plugs into any PLC-v1 host.

PLC v1 is the contract a **WASM ploxion module** must satisfy to be loaded,
isolated, and connected by the operational xerboxion-core (`xerboxion-host`).
It is intentionally minimal: a tiny linear-memory `alloc`, a JSON manifest, five
lifecycle exports, and two host imports. JSON is used for the manifest and for
event payloads (payloads are opaque to the host — the bus never inspects them).

This contract is the in-process, WASM-level realisation of the PLC paper §3.8:
a ploxion **declares** itself, **runs standalone**, supports **clean shutdown**
(no residual — *droit au silence* §4.4), exposes **health** + a **capabilities
manifest**, never **sends without consent**, never **listens when off**. Two
ploxions talk only through the **XERB0XI0N bus**, which **routes + traces** but
never **filters**.

---

## 1. Memory ABI

All `ptr`/`len` values are `i32` indices into the **module's own** WASM linear
memory (exported as `memory`). UTF-8 for text, raw bytes for payloads.

| symbol | signature | who calls it | meaning |
|---|---|---|---|
| `memory` | exported linear memory | host reads/writes | the module's address space (per-instance, isolated) |
| `alloc` | `fn alloc(len: i32) -> i32` | host | reserve `len` bytes, return a pointer the host writes into before a call |

The host **borrows** a region (writes topic/payload bytes) immediately before a
call; it owns nothing inside the module. Freeing is wholesale: on `plc_goodbye`
the host drops the module's entire `Store`, reclaiming all linear memory.

---

## 2. Exports — what a ploxion module MUST provide

| export | signature | required | meaning |
|---|---|---|---|
| `plc_manifest` | `fn plc_manifest() -> i64` | yes | returns the manifest JSON as a packed `(ptr,len)` — see §4 |
| `plc_init` | `fn plc_init()` | yes | called exactly once after the manifest is read; set up state; MAY emit |
| `plc_health` | `fn plc_health() -> i32` | yes | `0` = OK, non-zero = degraded; swept periodically |
| `plc_on_event` | `fn plc_on_event(topic_ptr,i32, topic_len,i32, payload_ptr,i32, payload_len,i32)` | required iff it declares `requires` | bus delivers a subscribed event here |
| `plc_goodbye` | `fn plc_goodbye()` | yes | clean shutdown; release anything; host then drops the Store |
| `alloc` | see §1 | yes | memory ABI |
| `memory` | exported linear memory | yes | isolation boundary |

A module missing any required export is **rejected cleanly at load time**
(an `Err`, never a panic, nothing half-registered).

---

## 3. Imports — what the host provides to a ploxion

Registered under the WASM import module name **`xerboxion`**.

| import | signature | meaning |
|---|---|---|
| `plc_emit` | `fn plc_emit(topic_ptr,i32, topic_len,i32, payload_ptr,i32, payload_len,i32)` | send an event to the bus; host ROUTES it to every *other* subscriber and TRACES it; never filters the payload |
| `plc_log` | `fn plc_log(ptr,i32, len,i32)` | write a UTF-8 line to the host journal |

`plc_emit` called inside a ploxion is **queued** by the host and dispatched
after the call returns (no cross-`Store` re-entrancy), then cascaded until the
bus is quiescent.

### 3.1 `plc_fetch` — a **gated** host import (PLC v1.1)

`plc_fetch` is a host import the host links into a ploxion's `Store` **only if**
that ploxion's manifest declares the `net.fetch` capability (§4.1). It is the
brokered-HTTP power that lets a *pure-WASM ploxion* be a service adapter without
native host code.

| import | signature | gate |
|---|---|---|
| `plc_fetch` | `fn plc_fetch(method_ptr,i32, method_len,i32, url_ptr,i32, url_len,i32, body_ptr,i32, body_len,i32) -> i64` | linked iff manifest declares `net.fetch` |

The host does the real I/O (native, blocking `ureq`, rustls for https; the same
engine as the native connector) and returns a packed `(ptr, len)` (§4) into the
**ploxion's own** linear memory holding a JSON result:

```json
{ "status": 200, "body": "…", "truncated": false, "error": "…?" }
```

- `status` — HTTP code, or **`0`** when the request never produced a response
  (DNS/TLS/timeout). The host **never panics** on a network error.
- `body` — the response text, truncated to the host's `max_body` limit
  (`truncated:true` when the cap was hit).
- `error` — present only on a transport failure or a rejected request.

The host **validates and limits** every fetch: only `GET`/`POST` over
`http`/`https` (no `file://`, no arbitrary verbs), a per-request timeout, and a
max body. Each fetch is **traced** in the journal like an emit
(`fetch  <id> -> <method> <url> => <status>`). The sandbox sees a function, not
the network stack — least authority. A ploxion without the capability has no
such import; if its wasm references `plc_fetch` it is rejected at load (§4.1).

---

## 4. Manifest

`plc_manifest()` returns an `i64` packing `(ptr, len)` of a JSON document in the
module's memory:

```
ptr = (ret >> 32) as u32
len = (ret & 0xffff_ffff) as u32
```

The JSON shape:

```json
{
  "id": "ping",
  "version": "1.0.0",
  "provides": ["ping"],
  "requires": ["tick"],
  "children_types": [],
  "parent_types": []
}
```

| field | meaning |
|---|---|
| `id` | stable unique id of the ploxion |
| `version` | semver of the ploxion |
| `provides` | topics this ploxion MAY emit (its consent surface) |
| `requires` | topics it wants delivered (host auto-subscribes it) |
| `children_types` | ploxion kinds allowed below it in the tree (registry metadata) |
| `parent_types` | ploxion kinds allowed above it (registry metadata) |

`provides`/`requires`/`children_types`/`parent_types` default to `[]` if omitted.

### 4.1 `capabilities` (PLC **v1.1**, additive)

v1.1 adds **one optional manifest field**, `capabilities: [string]`, and is
**purely additive / backward compatible**: `PLC_ABI_VERSION` stays `1`; a new
`PLC_ABI_MINOR = 1` marks the extension. A manifest that omits `capabilities`
defaults it to `[]` and behaves **exactly** as in v1.0 — every existing ploxion
and host is unaffected.

```json
{
  "id": "health-adapter",
  "version": "1.0.0",
  "capabilities": ["net.fetch"],
  "provides": ["service.health"],
  "requires": ["health.check"]
}
```

A **capability** is *consent to a scoped host power*, mirroring how `provides`
is consent to emit on a topic. The host grants a power — links the matching host
import into that ploxion's `Store` — **only if** the manifest declares the
token. This is **least authority**: a ploxion gets exactly the powers it asked
for, nothing more.

| token | grants | gated import |
|---|---|---|
| `net.fetch` | host-brokered HTTP (GET/POST, http/https) | `plc_fetch` (§3.1) |

Rules (host, fail-closed):

- **Absent ⇒ `[]` ⇒ v1.0.** No capability token, no gated import linked: pure
  sandboxed compute, only `plc_emit`/`plc_log`.
- **Unknown token ⇒ rejected at load.** A manifest declaring a capability the
  host does not recognise (not in `capabilities::KNOWN`) is rejected cleanly —
  the host never pretends to grant a power that does not exist.
- **Import without declaration ⇒ rejected at load.** A wasm that *references* a
  gated import (e.g. `xerboxion.plc_fetch`) but does **not** declare the matching
  capability is rejected with a clear error (it tried to grab a power it never
  asked for). The gate is **load-bearing**, not advisory.
- **Declaration without use ⇒ fine.** A ploxion may declare `net.fetch` and
  never call `plc_fetch`; the power is available but unused.

---

## 5. Lifecycle & guarantees

1. **Load** — host compiles the wasm, validates required exports, instantiates
   it in its **own** `Store`/`Instance` (isolation), reads the manifest, and
   auto-subscribes it to its `requires` topics.
2. **Init** — `plc_init()` runs exactly once. Emits produced here are routed.
3. **Run** — the bus routes events: `plc_emit` (out) and `plc_on_event` (in).
   Periodic `plc_health()` sweeps.
4. **Shutdown** — `plc_goodbye()` on each, then the host **drops the Store**.
   No linear memory, callback, or state of the ploxion survives — *droit au
   silence*: a ploxion that is off cannot listen or send.

**Isolation:** each ploxion has a separate `Store`; WASM linear memory is
per-instance, so one ploxion cannot read another's memory. The host never shares
a `Store` or a `memory` between ploxions. Adapters (themselves ploxions) bridge
incompatible formats; the bus itself stays format-agnostic.

---

## 6. Native adapters / connectors — the I/O boundary

A WASM ploxion is **sandboxed pure compute**: it has no syscalls, no sockets, no
filesystem — it can only `plc_emit` / `plc_log` and react in `plc_on_event`.
That is by design (isolation + *droit au silence*). But the ecosystem must still
touch the real world (HTTP, files, sensors, …).

The host is **native Rust**, so the real world enters the bus through **native
adapters / connectors**: trusted host-side code that does the I/O on one side and
speaks the bus on the other.

```
  the real world                native host                    WASM sandbox
 (HTTP / files / …) ──I/O──▶  a native connector  ──emit──▶  bus ──▶ ploxion (pure)
                            (e.g. service-connector)        topic        reacts
```

A native participant:

- has **no `Store`**, no linear memory, no PLC exports — it is host code, not a
  wasm module, so the §2 export contract does not apply to it;
- **registers on the bus** under a stable id with the topics it `provides`
  (tracked separately from WASM ploxions, but routed identically);
- **emits** onto the bus exactly like a ploxion: the host routes its events to
  every *other* subscriber and traces them — the bus does not distinguish a
  native emitter from a wasm one. Subscribers (WASM ploxions) cannot tell, and
  do not need to.

**Reference connector — `service-connector`** (`crates/xerboxion-host/src/connector.rs`):
reads the runtime overlay `runtime.json` (read-only) for `deployed:true`
services with a health URL, does a real HTTP GET against each (native, via
`ureq`), and emits `service.health {id,url,code,up}` per service. The WASM
`watcher` ploxion `requires ["service.health"]` and reacts — without ever
touching the network.

> **Trust boundary.** Native adapters are trusted host code; WASM ploxions are
> untrusted sandboxed code. The current rule is blunt and safe: *only the native
> host touches the outside world.* **Capability-based WASM adapters** — granting
> a specific ploxion *scoped* host access (e.g. "may GET these URLs") brokered
> through the host — are future work, not part of PLC v1.

---

## 7. Reference

- Host-side constants & types: crate `xerboxion-plc` (`exports::*`, `imports::*`,
  `Manifest`, `pack_ptr_len`/`unpack_ptr_len`).
- Ploxion-side glue: crate `ploxion-sdk` (`alloc`, `emit`, `log`,
  `export_manifest!`).
- Example ploxions: `ploxions/{ping,pong,tracer,watcher}`.
- Native connector: `crates/xerboxion-host/src/connector.rs` (`ServiceConnector`,
  `service.health` topic), driven by `Host::connector_sweep`.
