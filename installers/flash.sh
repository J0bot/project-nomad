#!/usr/bin/env bash
# flash.sh — flashe / déploie le xerboxion-core sur du matériel embarqué.
#   ./flash.sh pi <user@host>     deploie le build aarch64 sur un Raspberry Pi via SSH + autostart
#   ./flash.sh pi-sd </mnt/rootfs> copie sur la partition root d'une carte SD montée
#   ./flash.sh arduino            prépare/explique le firmware wasm3 (Arduino/ESP32)
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"; cd "$ROOT"
MODE="${1:-}"; TARGET="${2:-}"
DIST="dist/aarch64-unknown-linux-gnu"

need_pi() { [ -x "$DIST/xerboxion-rt" ] || { echo "Build Pi manquant -> 'bash scripts/build-all-targets.sh' (besoin de gcc-aarch64-linux-gnu)."; exit 1; }; }

case "$MODE" in
  pi)
    need_pi; [ -n "$TARGET" ] || { echo "Usage: ./flash.sh pi user@host"; exit 1; }
    echo "== déploie sur $TARGET (Raspberry Pi aarch64) =="
    ssh "$TARGET" 'mkdir -p ~/xion/ploxions'
    scp "$DIST/xerboxion-rt" "$TARGET:~/xion/"
    scp "$DIST"/ploxions/*.wasm "$TARGET:~/xion/ploxions/"
    ssh "$TARGET" 'bash -s' <<'REMOTE'
chmod +x ~/xion/xerboxion-rt
cat > ~/xion/run.sh <<EOS
#!/usr/bin/env bash
cd ~/xion && exec ./xerboxion-rt serve ./ploxions --addr 0.0.0.0 --port 8730 --state-dir ./state
EOS
chmod +x ~/xion/run.sh
mkdir -p ~/.config/systemd/user
cat > ~/.config/systemd/user/xion.service <<EOS
[Unit]
Description=xerboxion-core (xion)
[Service]
ExecStart=%h/xion/run.sh
Restart=always
[Install]
WantedBy=default.target
EOS
systemctl --user daemon-reload 2>/dev/null && systemctl --user enable --now xion 2>/dev/null && echo "service xion actif" || { nohup ~/xion/run.sh >/dev/null 2>&1 & echo "lancé en background"; }
REMOTE
    echo "== xion sur le Pi. Teste: ssh $TARGET curl -s localhost:8730/healthz =="
    ;;
  pi-sd)
    need_pi; [ -d "$TARGET" ] || { echo "Usage: ./flash.sh pi-sd /media/.../rootfs"; exit 1; }
    sudo mkdir -p "$TARGET/home/pi/xion/ploxions"
    sudo cp "$DIST/xerboxion-rt" "$TARGET/home/pi/xion/"
    sudo cp "$DIST"/ploxions/*.wasm "$TARGET/home/pi/xion/ploxions/"
    echo "== copié sur la SD ($TARGET). Au 1er boot du Pi: ~/xion/run.sh (ajoute-le à l'autostart, cf docs/install-guide.md). =="
    ;;
  arduino)
    cat <<'EOA'
== Arduino / ESP32 : le xion via wasm3 (le bion sur une puce) ==
Le binaire hôte (wasmtime) est trop gros pour un microcontrôleur. Sur Arduino/ESP32
on remplace le runtime par wasm3 (interpréteur WASM, ~64 Ko flash + 10 Ko RAM) + UN
ploxion minuscule (un bion) embarqué en tableau d'octets.

1) Choisis le plus petit ploxion :  ls -lS target/ploxions/*.wasm | tail -1
2) Convertis-le en header C :        xxd -i target/ploxions/<id>.wasm > ploxion_wasm.h
3) Projet wasm3 ESP32/Arduino :      https://github.com/wasm3/wasm3-arduino
   - charge le module, expose les imports PLC (emit/log) côté C,
   - appelle plc_init() puis plc_on_event() sur tes entrées (capteurs, série).
4) Flash :  pio run -t upload   (PlatformIO)  ou Arduino IDE.

Détails + roadmap (no_std, wasmi sur Pi, wasm3 sur MCU) : docs/embedded-os-roadmap.md
EOA
    ;;
  *)
    echo "Usage: ./flash.sh {pi user@host | pi-sd /mnt/rootfs | arduino}"; exit 1 ;;
esac
