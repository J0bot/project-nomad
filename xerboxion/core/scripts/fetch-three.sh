#!/usr/bin/env bash
# Re-download the VENDORED Three.js build into web-3d/vendor/three.min.js.
#
# The 3D view (web-3d/xion-3d.html, served at GET /3d) loads Three.js from the
# same-origin route GET /3d/three.min.js — NEVER a CDN. The daemon inlines the
# file via include_str!, so it must exist at build time. It is committed for a
# reproducible offline build; this script only exists to bump the pinned version
# deliberately.
set -euo pipefail

# Pinned version. r160 is the last revision shipping the UMD build (which exposes
# the global `THREE` the page uses); bump deliberately and re-test the page.
THREE_VERSION="0.160.0"
URL="https://unpkg.com/three@${THREE_VERSION}/build/three.min.js"

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT="$ROOT/web-3d/vendor/three.min.js"
mkdir -p "$(dirname "$OUT")"

echo "[fetch-three] downloading Three.js r${THREE_VERSION} -> $OUT"
curl -fsSL "$URL" -o "$OUT.tmp"

# Sanity: it must be the real UMD build that defines the THREE namespace + a
# REVISION marker. Refuse to install a 404 page or an empty file.
if ! grep -q "REVISION" "$OUT.tmp"; then
  echo "[fetch-three] ERROR: downloaded file has no REVISION marker — not Three.js" >&2
  rm -f "$OUT.tmp"
  exit 1
fi
mv "$OUT.tmp" "$OUT"
echo "[fetch-three] done: $(wc -c <"$OUT") bytes, $(grep -o 'REVISION=[a-z0-9]*' "$OUT" | head -1)"
