#!/usr/bin/env bash
# install.sh — installe le xerboxion-core (xion) sur Linux (toute archi : x86-64/ARM/Pi/RISC-V).
#   ./install.sh             build depuis les sources si Rust présent, sinon utilise un dist/ prebuild
#   ./install.sh --service   installe en plus un service systemd UTILISATEUR (démarre au login)
set -euo pipefail
PREFIX="${PREFIX:-$HOME/.xerboxion}"
ARCH=$(uname -m)
SELF="$(cd "$(dirname "$0")/.." && pwd)"
echo "== xerboxion-core installer — Linux $ARCH -> $PREFIX =="

case "$ARCH" in
  x86_64)  T=x86_64-unknown-linux-gnu ;;
  aarch64|arm64) T=aarch64-unknown-linux-gnu ;;
  armv7l)  T=armv7-unknown-linux-gnueabihf ;;
  riscv64) T=riscv64gc-unknown-linux-gnu ;;
  *) T="" ;;
esac

if [ -n "$T" ] && [ -x "$SELF/dist/$T/xerboxion-rt" ]; then
  echo "  source: binaire prebuild ($T)"
  BIN="$SELF/dist/$T/xerboxion-rt"; PLOX="$SELF/dist/$T/ploxions"
elif command -v cargo >/dev/null 2>&1; then
  echo "  source: build depuis les sources (cargo)..."
  ( cd "$SELF" && cargo build --release -p xerboxion-host && bash scripts/build-ploxions.sh ) >/dev/null
  BIN="$SELF/target/release/xerboxion-rt"; PLOX="$SELF/target/ploxions"
else
  echo "  !! Pas de binaire prebuild pour $ARCH et Rust absent."
  echo "     -> installe Rust (https://rustup.rs) puis relance,"
  echo "        OU lance scripts/build-all-targets.sh sur une machine et copie dist/$T/ ici."
  exit 1
fi

mkdir -p "$PREFIX/ploxions"
install -m755 "$BIN" "$PREFIX/xerboxion-rt"
cp "$PLOX"/*.wasm "$PREFIX/ploxions/"
cat > "$PREFIX/xion" <<EOS
#!/usr/bin/env bash
exec "$PREFIX/xerboxion-rt" serve "$PREFIX/ploxions" --addr 127.0.0.1 --port 8730 --state-dir "$PREFIX/state" "\$@"
EOS
chmod +x "$PREFIX/xion"
echo "  installé: $PREFIX/xerboxion-rt + $(ls "$PREFIX/ploxions" | wc -l) ploxions"

if [ "${1:-}" = "--service" ]; then
  UD="$HOME/.config/systemd/user"; mkdir -p "$UD"
  cat > "$UD/xion.service" <<EOS
[Unit]
Description=xerboxion-core (xion)
[Service]
ExecStart=$PREFIX/xion
Restart=always
RestartSec=5
[Install]
WantedBy=default.target
EOS
  if systemctl --user daemon-reload 2>/dev/null && systemctl --user enable --now xion 2>/dev/null; then
    echo "  service systemd 'xion' actif (systemctl --user status xion)"
  else
    echo "  (systemd --user indispo — lance '$PREFIX/xion' à la main)"
  fi
fi
echo "  lancer:  $PREFIX/xion   puis  curl http://127.0.0.1:8730/healthz"
echo "== fini =="
