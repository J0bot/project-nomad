#!/usr/bin/env bash
# End-to-end demo: build the example ploxions, then run the host demo which
# loads them, wires the bus from manifests, drives ticks, and prints the
# ping->pong trace across the WASM sandbox boundary.
#
# Exits non-zero if the events don't flow.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

# Build the WASM ploxions and stage them in target/ploxions.
"$ROOT/scripts/build-ploxions.sh"

# Build + run the host demo from the workspace root so target/ploxions resolves.
echo
echo "[demo] running host demo"
cd "$ROOT"
cargo run --release --quiet --bin xerboxion-rt -- demo "$ROOT/target/ploxions"

# Connect the LIVE deployed services (runtime.json) to the bus: the native
# service-connector health-checks each over real HTTP and the WASM watcher reacts.
echo
echo "[demo] connecting the LIVE deployed services to the bus (native connector -> watcher)"
cargo run --release --quiet --bin xerboxion-rt -- services "$ROOT/target/ploxions"
