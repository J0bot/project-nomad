#!/usr/bin/env bash
# Regenerate the BAKED snapshot inside web-map/xerboxion-map.html.
#
# Runs `xerboxion-rt map --json` against the staged example ploxions and injects
# the resulting JSON into the page's <script type="application/json"
# id="baked-snapshot"> block — exactly the way /bi0ns/.../tsoin-web bakes its
# wasm, so the committed HTML renders the core ALIVE from file:// with no setup.
#
# Prereqs: the example ploxions must be built first
#   scripts/build-ploxions.sh
# (this script will attempt it if target/ploxions is empty).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
HTML="$ROOT/web-map/xerboxion-map.html"
PLOXIONS_DIR="$ROOT/target/ploxions"

if [ ! -f "$HTML" ]; then
  echo "[build-map] ERROR: $HTML not found" >&2
  exit 1
fi

# Ensure the ploxions exist (the snapshot loads real wasm).
if [ -z "$(ls -A "$PLOXIONS_DIR"/*.wasm 2>/dev/null || true)" ]; then
  echo "[build-map] no ploxions staged — building them first"
  bash "$ROOT/scripts/build-ploxions.sh"
fi

echo "[build-map] building xerboxion-rt (release)"
cargo build --release --manifest-path "$ROOT/Cargo.toml" --bin xerboxion-rt >/dev/null

BIN="$ROOT/target/release/xerboxion-rt"

echo "[build-map] running: xerboxion-rt map $PLOXIONS_DIR --json"
SNAP="$(cd "$ROOT" && "$BIN" map "$PLOXIONS_DIR" --json)"

# Sanity: it must be non-trivial JSON with a trace.
echo "$SNAP" | python3 -c '
import json,sys
d=json.load(sys.stdin)
assert d.get("generated_by")=="xerboxion-rt map", "wrong generator"
assert len(d.get("ploxions",[]))>=1, "no ploxions"
assert len(d.get("trace",[]))>=1, "empty trace"
print("[build-map]   snapshot ok: %d ploxions, %d bus edges, %d trace hops, %d services, commit %s"
      % (len(d["ploxions"]),len(d["bus"]),len(d["trace"]),len(d["services"]),d["core_commit"]),
      file=sys.stderr)
' 1>&2

# Inject between the baked-snapshot <script> markers. Done in Python so JSON
# special characters (&, <, /, quotes) are handled verbatim — no sed escaping.
SNAP="$SNAP" HTML="$HTML" python3 - <<'PY'
import os, re, sys

html_path = os.environ["HTML"]
snap = os.environ["SNAP"].strip()

with open(html_path, "r", encoding="utf-8") as f:
    html = f.read()

# Inject between the explicit marker comments — an UNAMBIGUOUS region that only
# exists once and never appears in prose/comments elsewhere. We rebuild the whole
# block (markers + the JSON <script>) so re-runs are idempotent.
pat = re.compile(
    r"<!-- XERB-MAP-SNAPSHOT-BEGIN.*?-->.*?<!-- XERB-MAP-SNAPSHOT-END -->",
    re.DOTALL,
)
if not pat.search(html):
    print("[build-map] ERROR: XERB-MAP-SNAPSHOT markers not found in the HTML", file=sys.stderr)
    sys.exit(1)

# A JSON-in-HTML script block must not contain the literal "</script"; the map
# snapshot never does, but guard anyway by breaking that exact sequence (every
# such occurrence would be inside a JSON string, where "<\/" is a valid escape).
safe = re.sub(r"</(script)", r"<\\/\1", snap, flags=re.IGNORECASE)

block = (
    "<!-- XERB-MAP-SNAPSHOT-BEGIN : scripts/build-map.sh rewrites the block below -->\n"
    '<script type="application/json" id="baked-snapshot">\n'
    + safe
    + "\n</script>\n"
    "<!-- XERB-MAP-SNAPSHOT-END -->"
)
new_html = pat.sub(lambda _m: block, html, count=1)

with open(html_path, "w", encoding="utf-8") as f:
    f.write(new_html)

print("[build-map]   baked snapshot into %s" % html_path, file=sys.stderr)
PY

echo "[build-map] done — open web-map/xerboxion-map.html (file://) to see the core alive"
