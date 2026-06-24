#!/usr/bin/env bash
# core-size.sh — empreinte du xerboxion-core OS vs la LIMITE des 16 Go (José).
#
# « tous les ploxions + les bions dans le core, le moins de place possible ; tout
#   l'OS tient dans 16 Go max, hors photos/data. Si ça dépasse, il faut redescendre. »
#
# L'OS = le CODE : runtime (binaire) + tous les ploxions (.wasm) + les bions (source).
# Hors données/photos. A lancer apres chaque batch (la stratégie bions garde ca minuscule).
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
LIM=$((16*1024*1024*1024))
BIN=$(du -sb target/release/xerboxion-rt 2>/dev/null | cut -f1 || echo 0)
WASM=$(du -scb target/ploxions/*.wasm 2>/dev/null | tail -1 | cut -f1 || echo 0)
SRC=$(du -sb --exclude=target --exclude=.git . 2>/dev/null | cut -f1)
python3 - "$BIN" "$WASM" "$SRC" "$LIM" <<'PY'
import sys, glob
b, w, s, lim = map(int, sys.argv[1:5])
tot = b + w + s
n = len(glob.glob('target/ploxions/*.wasm'))
print(f'  binaire runtime : {b/1e6:8.2f} Mo')
print(f'  {n:>2} wasm ploxions : {w/1e6:8.2f} Mo  ({w/max(n,1)/1024:.0f} Ko/ploxion)')
print(f'  source (bions)  : {s/1e6:8.2f} Mo')
print(f'  -- TOTAL core OS: {tot/1e6:8.2f} Mo  = {100*tot/lim:.4f}% de 16 Go  (marge {(lim-tot)/1e9:.2f} Go)')
print('  OK sous la limite' if tot <= lim else '  !! DEPASSE 16 Go -> REDESCENDRE (uncraft / compress / dedup)')
PY
