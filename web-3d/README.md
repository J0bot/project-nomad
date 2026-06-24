# web-3d — the xion in 3D/4D, station « pourriture 4 »

`xion-3d.html` is the **live 3D/4D view of the running xerboxion-core**, served by
the persistent daemon (`xerboxion-rt serve`) at **`GET /3d`**. It is the reference
3D view of the CORE: fly through your running xion like the guts of a computer /
a dark-space satellite named **« pourriture 4 »**. The fleet builds the full
site-3D on the very same data (`/snapshot` + `/ws`).

It does, in 3D, exactly what the 2D live map (`GET /`) does in SVG:

1. on load it **`GET /snapshot`** (same-origin) and builds the scene;
2. it opens a **WebSocket to `/ws`** and animates every bus hop AS IT HAPPENS.

The feed is the REAL bus — nothing is faked.

## What the scene shows

- **Each ploxion = a node** laid out on a sphere (a Fibonacci shell) so the core
  reads as a station you orbit. Shape + colour by the **size-to-type taxonomy**:
  - `wasm`  → cyan **octahedron** (prime),
  - `native`→ blue **cube** (cubion),
  - `recipe`→ amber **sphere** (Spherion),
  - anything else → pink tetrahedron (fallback, still visible).
  Node size scales with connectivity (provides + requires + capabilities). A
  degraded ploxion (`health != 0`) turns red.
- **Bus wiring = edges**: for every `bus[].topic` `from -> to`, a line coloured
  deterministically by topic.
- **4D = time**: every live event from `/ws` animates. A **pulse travels along
  the edge** from emitter to subscriber (`route` hops carry the target in
  `note` = `"=> id"`; `emit` hops pulse along every edge leaving the emitter on
  that topic) and the **target node lights up** and swells, in real time.

## Camera — « foncer dedans »

- **drag** to orbit, **scroll** to zoom,
- **WASD / arrow keys** to fly the focus point through the station, **Q/E** to
  lift/drop, **R** to recenter,
- **hover** a node for its id / kind / provides / requires / capabilities / health.

A small HUD shows the station name, the core commit, the ploxion + bus-edge
count, a live pill, the last-event ticker, the legend, and an **emit box** (POSTs
to `/emit`, or pushes `{emit:{…}}` over the open WS) so you can inject onto the
live bus and watch it flow.

## Same-origin, self-contained — no CDN

[Three.js](https://threejs.org) is **vendored** in `vendor/three.min.js` and
served by the daemon at **`GET /3d/three.min.js`** (the binary inlines both the
HTML and the JS via `include_str!`). Nothing ever reaches a CDN; everything is
same-origin from the daemon, so it works behind the basic-auth once the user is
authed, exactly like `/`.

### Refreshing the vendored Three.js

```sh
scripts/fetch-three.sh        # re-downloads vendor/three.min.js (pinned r160)
```

The file is committed so the build is reproducible offline; the script only
exists to bump the pinned version deliberately.
