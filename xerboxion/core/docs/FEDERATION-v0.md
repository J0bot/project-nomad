# BUS FEDERATION v0 — the inter-node xion protocol

> The xion is the **protocol connecting all ploxions BETWEEN machines**. v0
> peer-links two `xerboxion-rt serve` daemons so a ploxion on node A reacts to an
> event emitted on node B and vice versa. Built and verified with two local
> daemon instances (PC + VPS simulation); it connects José's real PC to the VPS
> unchanged — only the peer URL and auth differ.

This is **opt-in and additive**: with no `--peer`, the daemon behaves exactly as
before. All pre-existing tests/demos pass untouched.

---

## 1. Node identity

Every daemon has a stable **node id** — its origin label.

```
xerboxion-rt serve --node-id A        # default = $HOSTNAME / /etc/hostname / "xion-node"
```

The node id is reported at `GET /healthz` (`"node_id":"A"`). Every
**locally-originated** emit on this daemon is tagged, when forwarded, with
`origin_node = <node-id>`.

## 2. The peer link

```
xerboxion-rt serve --node-id B --port 8731 --peer ws://127.0.0.1:8730/peer
```

`--peer <ws-url>` is **repeatable** (link to many peers). On startup the daemon
opens a **WebSocket CLIENT** connection to each peer's dedicated **`/peer`**
endpoint and runs a bidirectional federation loop. If a peer is down it
**reconnects with exponential backoff** (500 ms → ×2 → 15 s cap, forever — a peer
may boot later or restart). A daemon also *accepts* inbound peer links on its own
`/peer`. A link is fully bidirectional regardless of who dialed.

`/peer` is a **dedicated endpoint, distinct from `/ws`**: `/ws` speaks the client
frame `{"emit":{…}}`; `/peer` speaks the federation frame `{"fed":{…}}`. A
federated inject can therefore never be confused with an ordinary client emit.

## 3. The federation message (wire frame)

JSON, both directions, on `/peer`:

```json
{"fed":{"origin_node":"A","seq":62,"topic":"ping","payload":"from-PC-to-VPS"}}
```

| field         | meaning                                                        |
|---------------|----------------------------------------------------------------|
| `origin_node` | the node the event ORIGINATED on (its `--node-id`)             |
| `seq`         | that node's trace index of the emit (monotone per origin)      |
| `topic`       | the bus topic                                                  |
| `payload`     | opaque UTF-8 payload (the host never inspects it)              |

## 4. Forwarding rules

On the **host thread**, after every bus-touching command, each newly-appended
`Trace::Emit` hop is examined:

- **Local origin** (`from` does NOT start with `fed:`) → forward to all connected
  peers as `{"fed":{origin_node:<self>, seq, topic, payload}}`.
- **Remote origin** (`from` starts with `fed:`) → **NEVER forward onward.**

When a daemon **receives** a `{"fed":{…}}` frame it injects the event onto its
**local bus** marked **remote-origin**: `from = "fed:<origin_node>"`. Local
ploxions that `require` the topic react exactly as for any bus event. Two hops are
traced distinctly so the cross-node path is visible in `/snapshot` and `/ws`:

- `fed-in from <node> [<topic>] (seq N)` — recorded on receipt, before injection.
- `fed-out to peers [<topic>] (seq N)` — recorded when a local emit is forwarded
  (only when at least one peer link is actually attached — honest tracing).

## 5. Loop safety (provable)

Two independent guards make the protocol provably loop-, echo-, and storm-free:

1. **Never re-forward a remote-origin emit.** The forwarding predicate is exactly
   `forward ⇔ ¬is_remote_origin(from)`, and `is_remote_origin` is the single-bit
   test `from.starts_with("fed:")`. An event A emits is injected on B with
   `from = "fed:A"`; because B forwards only local-origin hops, the `fed:A` emit
   is **not** sent back to A. Symmetrically A never reflects a `fed:B` hop. So a
   given emit crosses the link **at most once per direction** and cannot echo.

2. **Idempotent injection per `(origin_node, seq)`.** Each receiver keeps a
   per-origin high-water mark of the greatest `seq` it has injected. A frame with
   `seq ≤ high-water` for that origin is dropped. With a single link this never
   triggers; with **redundant bidirectional links** (A dials B *and* B dials A,
   so two parallel links carry each direction) it collapses the duplicate, so the
   bus applies the event **exactly once**.

   `seq` is the origin's own trace index of the emit, strictly increasing per
   origin — so "seen" is a monotone water-mark, O(1) per inject, bounded by the
   number of distinct peer nodes.

**Not a loop:** if B's ploxion reacts to a federated event by emitting a *new*
topic, that new event is local-origin **on B**, carries a different
`(origin, topic, seq)`, and legitimately forwards to A as a genuine new event —
it is application cascade, not a protocol echo, and still crosses at most once per
direction. (The two-process demo shows exactly this: A's ping triggers B's `pong`,
which federates back to A as a *distinct* event — and the original ping never
bounces back to A.)

## 6. Authentication

The link **must be authenticatable** (the real PC↔VPS link will be). Two
mechanisms, combinable:

- **Basic-auth in the peer URL** — `--peer ws://user:pass@host:port/peer`. The
  client sends `Authorization: Basic base64(user:pass)` on the upgrade. This is
  what a **Traefik basic-auth in front of the VPS daemon** expects (the daemon
  itself is unauthenticated on `10.0.0.1`; Traefik terminates auth).
- **Shared peer token** — `--peer-token <tok>`. The dialing client sends it as
  the `X-Xion-Peer-Token` header; a daemon started with `--peer-token` **refuses
  `/peer` upgrades that lack the matching token (`401`)**. This is the daemon's
  own in-process check, so the link is authenticatable even with no proxy.

## 7. Run it (two-instance recipe)

```bash
scripts/federation-demo.sh        # two real daemon processes on 127.0.0.1
```

Or by hand:

```bash
# node A (the PC): plain daemon
xerboxion-rt serve target/ploxions --node-id A --port 8730

# node B (the VPS): dials A's /peer
xerboxion-rt serve target/ploxions --node-id B --port 8731 \
  --peer ws://127.0.0.1:8730/peer

# emit on A; B's pong (requires ping) reacts — across two separate processes
curl -X POST http://127.0.0.1:8730/emit \
  -H 'content-type: application/json' \
  -d '{"topic":"ping","payload":"from-PC-to-VPS"}'

curl -s http://127.0.0.1:8731/snapshot   # B's trace: fed-in from A, emit fed:A ping, route => pong
```

Authenticated (real PC↔VPS) variant:

```bash
# VPS daemon behind Traefik basic-auth; PC dials with creds + a shared token
xerboxion-rt serve --node-id VPS --peer-token "$XION_PEER_TOKEN"      # on the VPS
xerboxion-rt serve --node-id PC \
  --peer "wss://user:pass@xion.vps.example/peer" --peer-token "$XION_PEER_TOKEN"
```

## 8. What changed (additivity)

- **`lib.rs`**: one additive method `Host::note(who, what)` (pushes a
  `Trace::Lifecycle`) — the seam used to make `fed-in`/`fed-out` hops visible in
  the snapshot/trace. Nothing calls it unless federation is active.
- **`federation.rs`** (new): the wire types, the `fed:` origin convention, the
  loop-rule predicates, and peer parsing — all unit-tested.
- **`serve.rs`**: a `FederationConfig`, a `FedInject` host-thread command (with
  the per-origin dedup), the `/peer` server endpoint (+ token auth), the peer
  CLIENT task (reconnect/backoff + basic-auth/token), and the forwarding check in
  `broadcast_new`. `run`/`build_server` keep their old signatures as shims over
  `run_fed`/`build_server_fed`.
- With no `--peer`: no peer task is spawned, `fed_out` has no subscribers, the
  forwarding check is a cheap prefix test that never sends, and `/peer` simply
  accepts a link that never arrives. **Byte-for-byte the old behavior.**
