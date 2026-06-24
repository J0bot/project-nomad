#!/usr/bin/env bash
# Build SkyView: Rust -> wasm32 -> wasm-bindgen JS bindings into ./pkg.
# Requires: rustup target add wasm32-unknown-unknown ; wasm-bindgen-cli (=0.2.100).
set -euo pipefail
cd "$(dirname "$0")"

# Optional: run the astro/catalogue unit tests (native target).
if [ "${1:-}" = "--test" ]; then
  cargo test --lib
fi

cargo build --release --target wasm32-unknown-unknown
wasm-bindgen --target web --out-dir pkg \
  target/wasm32-unknown-unknown/release/skyview.wasm

echo "Built pkg/. Serve with:  python3 -m http.server 8088"
echo "Root harness:            http://localhost:8088/index.html"
echo "www harness:             http://localhost:8088/www/index.html"
