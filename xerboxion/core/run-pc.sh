#!/usr/bin/env bash
# run-pc.sh — lance le xerboxion-core (le xion) avec TOUS les ploxions, en local.
#
# Le xion = l'hote WASM + bus PLC. Il charge tous les .wasm d'un dossier et les fait
# tourner ensemble. Par defaut: 127.0.0.1:8730 (rien d'expose vers l'exterieur).
#
# Usage:
#   ./run-pc.sh                 # prend le bundle prebuilt si present, sinon build, puis serve
#   PORT=9000 ./run-pc.sh       # autre port
#   ADDR=0.0.0.0 ./run-pc.sh    # exposer sur le reseau local (attention)
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$ROOT"
ADDR="${ADDR:-127.0.0.1}"
PORT="${PORT:-8730}"
# Windows (Git Bash/MSYS) : le binaire Rust s'appelle xerboxion-rt.exe
case "$(uname -s 2>/dev/null)" in MINGW*|MSYS*|CYGWIN*) EXE=".exe";; *) EXE="";; esac
PLOX="$ROOT/target/ploxions"
BIN="$ROOT/target/release/xerboxion-rt$EXE"

# 1) bundle prebuilt fourni ? (binaire + wasm deja compiles) -> on s'en sert tel quel.
if [ ! -x "$BIN" ] && [ -x "$ROOT/prebuilt/xerboxion-rt$EXE" ]; then
  echo "[bundle] binaire prebuilt detecte"
  mkdir -p "$ROOT/target/release"
  cp "$ROOT/prebuilt/xerboxion-rt$EXE" "$BIN"
fi
if [ ! -d "$PLOX" ] && [ -d "$ROOT/prebuilt/ploxions" ]; then
  echo "[bundle] ploxions prebuilt detectes"
  mkdir -p "$ROOT/target"
  cp -r "$ROOT/prebuilt/ploxions" "$PLOX"
fi

# 2) sinon, build depuis les sources (Rust requis).
if [ ! -x "$BIN" ]; then
  command -v cargo >/dev/null || { echo "!! Rust manquant. Installe-le: https://rustup.rs  puis relance."; exit 1; }
  echo "[build] host (release) — premiere fois = quelques minutes..."
  cargo build --release -p xerboxion-host
fi
if [ ! -d "$PLOX" ] || [ -z "$(ls -A "$PLOX" 2>/dev/null)" ]; then
  command -v cargo >/dev/null || { echo "!! Rust manquant pour builder les ploxions: https://rustup.rs"; exit 1; }
  rustup target list --installed 2>/dev/null | grep -q wasm32-unknown-unknown || rustup target add wasm32-unknown-unknown
  echo "[build] ploxions (wasm)..."
  bash scripts/build-ploxions.sh
fi

N=$(ls -1 "$PLOX"/*.wasm 2>/dev/null | wc -l | tr -d ' ')
echo
echo "[run] $N ploxions  ->  http://$ADDR:$PORT"
echo "      sante   : curl http://$ADDR:$PORT/healthz"
echo "      flux    : curl -N http://$ADDR:$PORT/events"
echo "      nourrir : curl -X POST http://$ADDR:$PORT/emit -H 'Content-Type: application/json' -d '{\"topic\":\"xerbion.feed\",\"payload\":\"{\\\"x\\\":0.5}\"}'"
echo "      stop    : Ctrl-C (goodbye propre)"
echo
exec "$BIN" serve "$PLOX" --addr "$ADDR" --port "$PORT" --state-dir "$ROOT/xion-state"
