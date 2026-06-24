# web-map — the xerboxion-core made VISIBLE

`xerboxion-map.html` is a **single, self-contained** page that shows the
operational xerboxion-core *alive*: the WASM ploxions, how they are wired on the
PLC bus, the events that actually flowed, and the services bridged onto the bus.

- **No CDN, no external fonts, no network.** Everything (CSS + JS) is inline and
  the page runs straight from `file://`. Double-click it.
- **A real snapshot is baked in.** The page embeds the verbatim output of
  `xerboxion-rt map --json` inside a `<script type="application/json">` block —
  the same trick the `tsoin-web` page uses to bake its wasm — so it renders with
  zero setup. You can also drop a fresh snapshot file via the header input.

It renders three things, all hand-rolled (SVG + DOM, no libraries):

1. **node graph** — each participant a node, coloured by `kind`
   (`wasm` / `native` / `recipe`); edges are bus wiring (`provides → requires`)
   labelled by topic. A degraded ploxion (`health != 0`) gets a red border/dot.
2. **event trace** — the host journal as a readable timeline
   (`emit` / `route` / `recipe` / `fetch` / `log` / `life`), filterable by kind.
3. **services** — the bridged deployed services (id / url / code / up), green/red.

The layout is responsive (CSS grid + flex; the graph scrolls horizontally on
narrow screens), so it adapts to any screen size.

## Regenerating the baked snapshot

```sh
scripts/build-ploxions.sh   # build the example WASM ploxions (once)
scripts/build-map.sh        # run `map --json` and inject it into the HTML
```

`scripts/build-map.sh` runs the host's `map` subcommand and rewrites the block
between the `XERB-MAP-SNAPSHOT-BEGIN` / `XERB-MAP-SNAPSHOT-END` markers. It is
idempotent — run it any time the core changes to refresh the page.

## Snapshot schema (v1)

`xerboxion-rt map --json` emits exactly this shape (every field is derived from
live `Host` state / the trace journal — nothing is hardcoded):

```json
{
  "generated_by": "xerboxion-rt map",
  "core_commit":  "<git short sha>",
  "scenario":     "<one-line description of the run>",
  "ploxions": [
    {"id","version","provides":[],"requires":[],"capabilities":[],
     "kind":"wasm|native|recipe","health":0}
  ],
  "bus":      [ {"topic","from","to":[]} ],
  "trace":    [ {"seq","kind","from","topic","payload","note"} ],
  "services": [ {"id","url","code","up"} ]
}
```

- `ploxions[].kind` — `wasm` (sandboxed module), `native` (host-side participant,
  e.g. the service connector), `recipe` (a compiled designer rule run as a native
  participant). `health` is the live `plc_health` result for wasm ploxions
  (`0` = ok), `0` for native/recipe.
- `bus[]` — one row per `provider → topic → [subscribers]` edge. `from` is the
  provider id (`""` for a host-injected / external topic such as `tick`).
- `trace[]` — the journal in order. `kind` ∈ {`emit`,`route`,`log`,`life`,
  `fetch`,`recipe`}; `seq` is the 0-based index; fields that do not apply to a
  kind are `""`.
- `services[]` — the deployed services the connector bridged onto the bus this
  run, with their health result.

> Note: the `map` scene runs fully OFFLINE and deterministic (stubbed
> `plc_fetch` + health check), so the snapshot is reproducible — the live
> network belongs only to the `services` / `capdemo` subcommands.
