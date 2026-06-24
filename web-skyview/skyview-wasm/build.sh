#!/usr/bin/env bash
# Construit le bion-de-wasm SkyView (libm only, no_std) et l'INLINE en base64
# dans une page autonome -> web-skyview/index.html (ouvrable direct dans Safari,
# zero serveur). C'est la version "regarde tout de suite sur ton tel".
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
OUT="$(cd "$HERE/.." && pwd)/index.html"
rustup target list --installed 2>/dev/null | grep -q wasm32-unknown-unknown || rustup target add wasm32-unknown-unknown
( cd "$HERE" && cargo build --release --target wasm32-unknown-unknown )
WASM="$HERE/target/wasm32-unknown-unknown/release/skyview.wasm"
if command -v wasm-opt >/dev/null 2>&1; then wasm-opt -Oz "$WASM" -o "$WASM.opt" && WASM="$WASM.opt"; fi
B64="$(base64 -w0 "$WASM")"
python3 - "$HERE/page.tpl.html" "$OUT" "$B64" <<'PY'
import sys
tpl=open(sys.argv[1]).read()
open(sys.argv[2],"w").write(tpl.replace("__WASM_B64__", sys.argv[3]))
print("index.html", len(open(sys.argv[2]).read()), "octets ; wasm b64", len(sys.argv[3]))
PY
echo "[skyview-solo] -> $OUT (autonome, ouvrable file://)"
