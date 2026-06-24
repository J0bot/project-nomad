#!/usr/bin/env bash
# ===========================================================================
# BUS FEDERATION demo — two SEPARATE xion daemons peer-link their buses.
#
# Simulates José's PC (node A) and the VPS (node B) as two real, independent
# `xerboxion-rt serve` PROCESSES on 127.0.0.1. Node B opens a WebSocket peer
# link to node A (`--peer ws://127.0.0.1:<A>/peer`). We then emit `ping` on A
# via `POST /emit`; the event crosses the link and triggers B's local ploxions
# (pong/tracer require `ping`). The trace on BOTH nodes is printed via /snapshot
# so you can SEE the cross-node hop (`fed-in from A`, an emit from `fed:A`, and a
# route to a local subscriber on B) — nothing faked.
#
# Loop safety: A emits ping ONCE; it lands on B exactly once and does NOT echo
# back to A (B never re-forwards a remote-origin `fed:A` event).
#
# Usage:  scripts/federation-demo.sh
# Offline; binds loopback only. Requires the built ploxions (build-ploxions.sh).
# ===========================================================================
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

PORT_A=8730
PORT_B=8731
DIR="target/ploxions"
BIN="target/release/xerboxion-rt"

if [[ ! -x "$BIN" ]]; then
  echo "[demo] building the daemon (release)…"
  cargo build --release -p xerboxion-host
fi
if [[ ! -f "$DIR/ping.wasm" ]]; then
  echo "[demo] building ploxions…"
  bash scripts/build-ploxions.sh
fi

LOG_A="$(mktemp)"
LOG_B="$(mktemp)"
PIDS=()
cleanup() {
  echo
  echo "[demo] shutting both nodes down…"
  for pid in "${PIDS[@]:-}"; do kill "$pid" 2>/dev/null || true; done
  wait 2>/dev/null || true
  rm -f "$LOG_A" "$LOG_B"
}
trap cleanup EXIT INT TERM

echo "============================================================"
echo " BUS FEDERATION demo — node A (PC) <—peer— node B (VPS)"
echo "============================================================"

# --- Node A: the emitter. Plain daemon, no peers. -------------------------
echo "[demo] starting node A (--node-id A --port $PORT_A)…"
"$BIN" serve "$DIR" --node-id A --port "$PORT_A" >"$LOG_A" 2>&1 &
PIDS+=("$!")

# --- Node B: dials A's /peer (the inter-node link). -----------------------
echo "[demo] starting node B (--node-id B --port $PORT_B --peer ws://127.0.0.1:$PORT_A/peer)…"
"$BIN" serve "$DIR" --node-id B --port "$PORT_B" \
  --peer "ws://127.0.0.1:$PORT_A/peer" >"$LOG_B" 2>&1 &
PIDS+=("$!")

# Wait for both daemons to bind + the peer link to come up.
echo "[demo] waiting for both nodes + the peer link…"
for _ in $(seq 1 40); do
  if curl -fsS "http://127.0.0.1:$PORT_A/healthz" >/dev/null 2>&1 \
     && curl -fsS "http://127.0.0.1:$PORT_B/healthz" >/dev/null 2>&1 \
     && grep -q "peer link UP" "$LOG_B"; then
    break
  fi
  sleep 0.25
done

echo
echo "[demo] node A healthz: $(curl -fsS http://127.0.0.1:$PORT_A/healthz)"
echo "[demo] node B healthz: $(curl -fsS http://127.0.0.1:$PORT_B/healthz)"
echo "[demo] peer link line on B: $(grep 'peer link UP' "$LOG_B" | head -1)"

# --- Emit ping on A (locally originated -> forwarded to B). ----------------
echo
echo "[demo] >>> emitting ping on node A (POST /emit)…"
curl -fsS -X POST "http://127.0.0.1:$PORT_A/emit" \
  -H 'content-type: application/json' \
  -d '{"topic":"ping","payload":"from-PC-to-VPS"}' && echo

# Let the event cross the link + cascade on B.
sleep 1

echo
echo "============================================================"
echo " NODE A trace (the origin) — ping emitted locally, fed-out:"
echo "============================================================"
curl -fsS "http://127.0.0.1:$PORT_A/snapshot" \
  | grep -oE '"seq":[0-9]+,"kind":"[^"]*","from":"[^"]*","topic":"[^"]*","payload":"[^"]*","note":"[^"]*"' \
  | grep -E 'ping|fed-' || echo "(no matching rows)"

echo
echo "============================================================"
echo " NODE B trace (the peer) — RECEIVED ping as fed:A, routed:"
echo "============================================================"
curl -fsS "http://127.0.0.1:$PORT_B/snapshot" \
  | grep -oE '"seq":[0-9]+,"kind":"[^"]*","from":"[^"]*","topic":"[^"]*","payload":"[^"]*","note":"[^"]*"' \
  | grep -E 'ping|fed-|pong' || echo "(no matching rows)"

echo
echo "[demo] DONE — the ping emitted on A crossed the peer link and triggered B."
