#!/usr/bin/env bash
# make-usb.sh — rend une clé USB BOOTABLE sur le xerboxion.
#
# ⚠️ À lancer CHEZ TOI avec TA clé. Le mode d'écriture EFFACE le disque ciblé.
#   ./make-usb.sh list                 liste les disques (pour identifier ta clé sans te tromper)
#   ./make-usb.sh bare  /dev/sdX       le KERNEL bare-metal (Multiboot via GRUB) = le VRAI boot xerboxion, sans Linux
#   ./make-usb.sh linux /dev/sdX       Linux minimal + le xion qui auto-démarre (tourne tous les ploxions WASM)
#
# Deux chemins, par exigence :
#   bare  = pureté (la machine boote directement sur le cœur xerboxion-core ; pas encore le runtime WASM complet,
#           c'est le kernel M4c qui boote — la couche hôte-WASM-sur-bare-metal est l'étape suivante du roadmap).
#   linux = fonctionnel TOUT DE SUITE (un Linux qui démarre et lance `xerboxion-rt serve` avec les 31 ploxions).
set -euo pipefail
MODE="${1:-}"; DEV="${2:-}"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"

confirm_wipe() {
  [ -b "$DEV" ] || { echo "!! $DEV n'est pas un disque bloc. Lance './make-usb.sh list' d'abord."; exit 1; }
  echo "⚠️  Va EFFACER $DEV : $(lsblk -ndo SIZE,MODEL "$DEV" 2>/dev/null)"
  read -r -p "Taper exactement 'EFFACER' pour continuer : " ok
  [ "$ok" = "EFFACER" ] || { echo "annulé."; exit 1; }
}

case "$MODE" in
  list)
    echo "Disques (repère ta clé USB par sa taille/modèle, PAS le disque système) :"
    lsblk -do NAME,SIZE,MODEL,TRAN | grep -iE 'usb|NAME' || lsblk -do NAME,SIZE,MODEL,TRAN
    echo "Puis : ./make-usb.sh {bare|linux} /dev/<ta-cle>"
    ;;

  bare)
    confirm_wipe
    # 1) build du kernel bare-metal (repo xerboxion-core, Multiboot)
    KSRC="$HOME/xerboxion-core"; [ -d "$KSRC" ] || KSRC="$HOME/repos/xerboxion-core"
    [ -d "$KSRC" ] || { echo "!! repo xerboxion-core introuvable (~/xerboxion-core)."; exit 1; }
    command -v grub-mkrescue >/dev/null || { echo "!! installe: sudo apt install grub-pc-bin grub-common xorriso mtools"; exit 1; }
    echo "[bare] build du kernel..."
    ( cd "$KSRC" && cargo build --release 2>/dev/null || cargo build --release )
    KERNEL=$(find "$KSRC/target" -name '*.elf' -o -name 'xerboxion*' -type f 2>/dev/null | grep -iE 'release' | head -1)
    [ -n "$KERNEL" ] || { echo "!! binaire kernel introuvable — vérifie le build de xerboxion-core."; exit 1; }
    # 2) arbo GRUB Multiboot -> ISO
    T=$(mktemp -d); mkdir -p "$T/boot/grub"
    cp "$KERNEL" "$T/boot/xerboxion.elf"
    cat > "$T/boot/grub/grub.cfg" <<EOG
set timeout=2
menuentry "XERBOXION (bare-metal)" {
  multiboot2 /boot/xerboxion.elf
  boot
}
EOG
    grub-mkrescue -o "$T/xerboxion.iso" "$T" 2>/dev/null
    echo "[bare] écriture sur $DEV (dd)..."
    sudo dd if="$T/xerboxion.iso" of="$DEV" bs=4M status=progress conv=fsync
    sudo sync; rm -rf "$T"
    echo "[bare] OK — boote $DEV (régler le BIOS sur USB). Le cœur xerboxion démarre."
    ;;

  linux)
    confirm_wipe
    echo "[linux] chemin Linux-minimal + xion. Recette (à exécuter avec un Alpine extended ISO) :"
    cat <<'EOL'
  Le plus simple et robuste, étapes manuelles documentées (pas de magie auto qui casse ta clé) :
   1) Grave Alpine 'extended' x86_64 sur la clé (dd l'ISO Alpine, ou Ventoy + l'ISO).
   2) Boote dessus, `setup-alpine` (install sur la clé, mode 'sys' pour la persistance).
   3) Copie le xion :  scp/clone ce repo, puis  bash installers/install.sh --service
      (ou copie dist/x86_64-unknown-linux-gnu/ + un service openrc/systemd qui lance run.sh).
   4) Le xion démarre au boot ; ouvre http://127.0.0.1:8730/healthz.
  -> Un PC qui boote sur la clé = un xerboxion qui tourne, avec tous les ploxions WASM.
  (Une image préfabriquée 'xion-live.img' ddable directement = livrable suivant ; demande à cloudion.)
EOL
    ;;

  *)
    echo "Usage: ./make-usb.sh {list | bare /dev/sdX | linux /dev/sdX}"
    echo "  D'abord  ./make-usb.sh list  pour trouver ta clé sans te tromper de disque."
    exit 1 ;;
esac
