#!/usr/bin/env bash
# Build the example WASM ploxions and stage them where the host loads them.
#
# Compiles ploxions/{ping,pong,tracer,tsoin,state-client,watcher,health-adapter,
# ideas-map-adapter,repoverse-adapter} to wasm32-unknown-unknown and copies the
# resulting .wasm into
# ./target/ploxions/ (the host's default load dir). The tsoin ploxion is the
# versioning/state layer (it depends on the REAL tsoin engine built for wasm32);
# state-client drives it. The watcher reacts to service.health events. The
# health-adapter is a PURE-WASM service adapter (PLC v1.1): with the net.fetch
# capability it does a real plc_fetch and emits service.health itself — proving a
# sandboxed ploxion can bridge a service to the bus without native host code.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PLOXIONS_DIR="$ROOT/ploxions"
OUT_DIR="$ROOT/target/ploxions"

# Make sure the wasm target is present.
if ! rustup target list --installed 2>/dev/null | grep -q wasm32-unknown-unknown; then
  echo "[build-ploxions] adding wasm32-unknown-unknown target"
  rustup target add wasm32-unknown-unknown
fi

echo "[build-ploxions] building ploxions (release, wasm32-unknown-unknown)"
cargo build --release --manifest-path "$PLOXIONS_DIR/Cargo.toml" \
  --target wasm32-unknown-unknown

mkdir -p "$OUT_DIR"
WASM_SRC="$PLOXIONS_DIR/target/wasm32-unknown-unknown/release"

# Map each cargo artifact name (crate name with '-' -> '_') to the staged
# ploxion filename the host loads (the manifest id, '-' preserved).
count=0
stage() {
  local artifact="$1" staged="$2"
  local src="$WASM_SRC/$artifact.wasm"
  if [ -f "$src" ]; then
    cp "$src" "$OUT_DIR/$staged.wasm"
    echo "[build-ploxions]   staged $staged.wasm"
    count=$((count + 1))
  else
    echo "[build-ploxions]   WARN: $src not found" >&2
  fi
}

stage ploxion_ping          ping
stage ploxion_pong          pong
stage ploxion_tracer        tracer
stage ploxion_tsoin         tsoin
stage ploxion_state_client  state-client
stage ploxion_watcher       watcher
stage ploxion_health_adapter health-adapter
stage ploxion_ideas_map_adapter ideas-map-adapter
stage ploxion_repoverse_adapter repoverse-adapter
stage ploxion_gitea_adapter     gitea-adapter
stage ploxion_osiris_adapter    osiris-adapter
stage ploxion_index             index
stage ploxion_science           science
stage ploxion_synthe            synthe
stage ploxion_xp                xp
stage ploxion_ultra_detector    ultra-detector
stage ploxion_tester            tester
stage ploxion_link              link
stage ploxion_carte             carte
stage ploxion_kion              kion
stage ploxion_spectre           spectre
stage ploxion_mc_adapter        mc-adapter
stage ploxion_xerbion           xerbion
stage ploxion_xerbion_bit       xerbion-bit
stage ploxion_block_stone       block-stone
stage ploxion_block_water       block-water
stage ploxion_tsoin_store       tsoin-store
stage ploxion_diff              diff
stage ploxion_generator         generator
stage ploxion_clock_coherence  clock-coherence
stage ploxion_player            player

stage ploxion_house_brain_core       house-brain-core
stage ploxion_house_mode_manager     house-mode-manager
stage ploxion_proto_tcp           proto-tcp
stage ploxion_proto_udp           proto-udp
stage ploxion_proto_dns           proto-dns
stage ploxion_proto_http          proto-http
stage ploxion_proto_icmp          proto-icmp
stage ploxion_proto_tls           proto-tls
stage ploxion_proto_ntp           proto-ntp
stage ploxion_proto_dhcp          proto-dhcp
stage ploxion_proto_quic          proto-quic
stage ploxion_proto_ssh           proto-ssh
stage ploxion_proto_ws            proto-ws
stage ploxion_bion_tester         bion-tester
stage ploxion_tsoin_montage       tsoin-montage
stage ploxion_object              object
stage ploxion_bion_accelerator    bion-accelerator
stage ploxion_forge               forge

stage ploxion_port_22           port-22
stage ploxion_port_80           port-80
stage ploxion_port_443           port-443
stage ploxion_port_53           port-53
stage ploxion_pkg_bash         pkg-bash
stage ploxion_pkg_coreutils    pkg-coreutils
stage ploxion_pkg_curl         pkg-curl
stage ploxion_pkg_git          pkg-git
echo "[build-ploxions] staged $count ploxion(s) into $OUT_DIR"
