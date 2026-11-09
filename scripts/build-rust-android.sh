#!/usr/bin/env bash
# Build the Rust ffi crate for Android and copy .so files into jniLibs.
# Requires: cargo-ndk (installs automatically if missing), Android NDK.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

ANDROID_NDK_HOME="${ANDROID_NDK_HOME:-${ANDROID_HOME:-$HOME/Android/Sdk}/ndk/27.0.12077973}"
ABIS=(arm64-v8a armeabi-v7a x86_64 x86)
JNI_LIBS="$ROOT/android/app/src/main/jniLibs"

if ! command -v cargo-ndk >/dev/null 2>&1; then
    echo ">> Installing cargo-ndk..."
    cargo install cargo-ndk --locked
fi

export ANDROID_NDK_HOME
export PATH="$HOME/.cargo/bin:$PATH"

# rust-toolchain.toml only provides the host toolchain; the Android targets
# must be installed explicitly or rustc fails with E0463 ("can't find crate
# for `core`") on the first cross build.
for target in aarch64-linux-android armv7-linux-androideabi x86_64-linux-android i686-linux-android; do
    echo ">> Ensuring rustup target $target..."
    rustup target add "$target"
done

for abi in "${ABIS[@]}"; do
    echo ">> Building for $abi..."
    cargo ndk -t "$abi" -o "$JNI_LIBS" build --release -p arkabot-ffi
done

echo ">> Done. .so files at $JNI_LIBS"
