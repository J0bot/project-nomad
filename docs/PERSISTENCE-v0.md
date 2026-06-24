# Durable runtime-loaded ploxions — `--state-dir` (PERSISTENCE v0)

The `serve` daemon loads a **base set** of ploxions from `<ploxions_dir>` at
startup (those always reload from disk). At runtime, `POST /load` hot-loads MORE
ploxions into the running host without a restart, and `POST /unload` tears them
down. That **runtime-loaded set is otherwise ephemeral** — a deploy/crash/restart
loses it.

`--state-dir <path>` makes that runtime delta **DURABLE**: it is persisted on each
`/load`, removed on each `/unload`, and **restored on the next startup**, so a
hot-loaded ploxion survives a deploy/crash. Without the flag the feature is OFF and
the daemon behaves exactly as before.

## What is persisted — the DELTA only, never the base set

The base set is **not** persisted: it always reloads from `<ploxions_dir>`, so
persisting it would **double-load** on restart. We persist only ploxions loaded via
`/load` *after* startup. Each is one record:

| field    | meaning                                                                   |
|----------|---------------------------------------------------------------------------|
| `id`     | the ploxion's **manifest** id (its bus identity, and the wasm filename)    |
| `source` | `"staged"` (loaded by registered id) or `"wasm"` (a base64-uploaded module) |

- **`source = "staged"`** (`POST /load {"id":"<id>"}`): only the id is stored. On
  restore the bytes reload from `<ploxions_dir>/<id>.wasm` — the same path the
  original load read — so the staged wasm is never duplicated.
- **`source = "wasm"`** (`POST /load {"wasm":"<base64>"}`): the **wasm bytes are
  stored too** at `<state-dir>/loaded/<id>.wasm`, because an uploaded module lives
  nowhere else. On restore it is re-instantiated from those bytes.

The record is keyed by the ploxion's **real manifest id**, not the request's
fallback id, so a restore is unambiguous and an unload removes the right record.

## On-disk layout

```text
<state-dir>/
  loaded.json          # { "version": 1, "loaded": [ { "id": "…", "source": "staged"|"wasm" }, … ] }
  loaded/
    <id>.wasm          # the persisted bytes — ONLY for source = "wasm" records
```

## Atomicity + crash-safety

Every write is **atomic**: bytes go to a `<file>.tmp` in the same directory,
`fsync`, then a `rename` swaps it into place (rename within one dir is atomic on
POSIX). A crash mid-write leaves the previous good file intact, never a half-written
one. On a `/load`, the **wasm blob is written before the manifest**, so the manifest
never references a blob that does not yet exist. The **manifest is the source of
truth**: a stray `loaded/<id>.wasm` not referenced by it is simply ignored on
restore (and overwritten on the next persist of that id).

## Startup restore

1. Load the base set from `<ploxions_dir>` (as today).
2. If `--state-dir` is set, read `loaded.json` and, for each record, `load_runtime`
   the bytes — from the persisted blob (`wasm`) or `<ploxions_dir>/<id>.wasm`
   (`staged`).
3. Then serve.

## Graceful degradation — never crash startup

- **No `--state-dir`** → feature off; identical to the classic daemon.
- **Missing state dir / missing `loaded.json`** → nothing to restore (normal).
- **Corrupt/unknown-version manifest** → ignored with a `WARN`; the base set still
  boots.
- **A record whose wasm is missing/truncated/empty**, a now-**duplicate id** (already
  in the base set), or a module that fails to compile → that one record is **skipped
  with a `WARN`**; every other record and the whole base set still load.
- **A state dir that cannot be opened** (permission, etc.) → logged; the daemon runs
  **without persistence** rather than failing to start.
- Persistence is **best-effort**: a persist/remove I/O error is logged but never
  fails the live `/load`/`/unload` — the in-memory load already succeeded.

## Invariants kept

- The **host thread stays the sole owner** of the `Host` and of the state store; the
  async handlers only send commands. Persistence happens on the host thread right
  after the in-memory `load_runtime`/`unload` succeeds.
- Existing endpoints/behavior are unchanged; this is purely additive.
