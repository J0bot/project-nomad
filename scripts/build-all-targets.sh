#!/usr/bin/env bash
# build-all-targets.sh — build le xerboxion-core (xion) pour CHAQUE CPU.
#
# Pour chaque cible RÉUSSIE: dist/<triple>/ (binaire + ploxions/ + run.sh + README) et
# dist/xion-<triple>.tar.gz. Les ploxions (wasm32) sont COMMUNS à toutes les cibles ;
# seul le binaire hôte recompile par-archi. Un toolchain manquant => SKIP propre (dit quoi
# installer). Aucun échec de build ne fait planter le script.
set -uo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"; cd "$ROOT"
source "$HOME/.cargo/env" 2>/dev/null || true

# triple | linker-à-vérifier | paquet-debian | étiquette
TARGETS=(
  "x86_64-unknown-linux-gnu|cc|build-essential|PC / serveur Linux x86-64"
  "aarch64-unknown-linux-gnu|aarch64-linux-gnu-gcc|gcc-aarch64-linux-gnu|Raspberry Pi 64-bit / ARM64 Linux"
  "armv7-unknown-linux-gnueabihf|arm-linux-gnueabihf-gcc|gcc-arm-linux-gnueabihf|Raspberry Pi 32-bit / ARMv7"
  "riscv64gc-unknown-linux-gnu|riscv64-linux-gnu-gcc|gcc-riscv64-linux-gnu|RISC-V 64 Linux"
  "x86_64-pc-windows-gnu|x86_64-w64-mingw32-gcc|mingw-w64|Windows x86-64"
)

echo "[build-all] ploxions WASM (communs à toutes les cibles)..."
bash scripts/build-ploxions.sh >/dev/null || { echo "  échec build ploxions"; exit 1; }
echo "  ploxions: $(ls target/ploxions/*.wasm 2>/dev/null | wc -l) wasm prêts."

mkdir -p dist; made=0
for entry in "${TARGETS[@]}"; do
  IFS='|' read -r triple linker pkg label <<< "$entry"
  echo; echo "=== $label  ($triple) ==="
  if ! command -v "$linker" >/dev/null 2>&1; then
    echo "  [SKIP] linker '$linker' absent -> 'sudo apt install $pkg' (ou build sur la cible / cargo-zigbuild)."
    continue
  fi
  rustup target add "$triple" >/dev/null 2>&1 || true
  echo "  cargo build --release -p xerboxion-host --target $triple ..."
  if cargo build --release -p xerboxion-host --target "$triple" 2>"/tmp/bt-$triple.log"; then
    out="dist/$triple"; rm -rf "$out"; mkdir -p "$out/ploxions"
    bin=$(ls "target/$triple/release/xerboxion-rt" "target/$triple/release/xerboxion-rt.exe" 2>/dev/null | head -1)
    cp "$bin" "$out/"; cp target/ploxions/*.wasm "$out/ploxions/"
    printf '#!/usr/bin/env bash\ncd "$(dirname "$0")"\nexec ./%s serve ./ploxions --addr 127.0.0.1 --port 8730 --state-dir ./xion-state\n' "$(basename "$bin")" > "$out/run.sh"
    chmod +x "$out/run.sh"
    printf 'xerboxion-core (xion) — %s\nLancer: ./run.sh   puis  curl http://127.0.0.1:8730/healthz\n' "$label" > "$out/README.txt"
    tar -C dist -czf "dist/xion-$triple.tar.gz" "$triple"
    echo "  [OK] dist/$triple ($(du -h "$out/$(basename "$bin")" | cut -f1)) + dist/xion-$triple.tar.gz"; made=$((made+1))
  else
    echo "  [FAIL] voir /tmp/bt-$triple.log (souvent une dep C à cross-compiler). Build sur la cible ou 'cargo install cargo-zigbuild'."
  fi
done
echo; echo "[build-all] $made cible(s) produite(s) dans dist/. Embarqué (Arduino/ESP32) = wasm3 : installers/flash.sh arduino + docs/embedded-os-roadmap.md."
