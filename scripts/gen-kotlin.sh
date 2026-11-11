#!/usr/bin/env bash
# Generate uniffi Kotlin bindings from the Rust ffi crate.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

export PATH="$HOME/.cargo/bin:$PATH"
TARGET_DIR="${CARGO_TARGET_DIR:-$ROOT/target}"

# Build the host cdylib (bindgen reads the metadata from it)
cargo build --release -p arkabot-ffi

# Generate Kotlin bindings
cargo run -q -p bindgen -- generate \
    --library "$TARGET_DIR/release/libarkabot_ffi.so" \
    --language kotlin \
    -o android/app/src/main/rust \
    --crate arkabot_ffi

echo ">> Kotlin bindings generated at android/app/src/main/rust/"
