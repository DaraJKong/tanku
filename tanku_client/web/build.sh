#!/usr/bin/env bash
# Builds the web version of the game into target/web-dist.
#
# Needs the wasm-bindgen tool of the version in Cargo.lock, installed once with:
#   cargo install wasm-bindgen-cli --version 0.2.129 --root target/tools --locked
set -euo pipefail
cd "$(dirname "$0")/../.."

# A target directory of its own, so that this build doesn't wait for the desktop one.
export CARGO_TARGET_DIR=target/web
cargo build -p tanku_client --target wasm32-unknown-unknown --profile wasm-release --no-default-features

mkdir -p target/web-dist
# Function names double the size of the file, so they are left out.
target/tools/bin/wasm-bindgen --target web --remove-name-section --remove-producers-section \
    --out-dir target/web-dist --out-name tanku \
    target/web/wasm32-unknown-unknown/wasm-release/tanku_client.wasm
cp tanku_client/web/index.html target/web-dist/
echo "Built target/web-dist. Serve it with: python3 -m http.server --directory target/web-dist"
