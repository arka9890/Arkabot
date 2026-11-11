#!/usr/bin/env bash
# Build the Rust ffi crate for Android and copy .so files into jniLibs.
#
# The NDK ships only an x86_64 host toolchain. On an aarch64 host the
# wrapper script compiles C with zig (native) and links with the NDK's
# clang under qemu — see /opt/ndk-wrappers/ndk-android.sh.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

export PATH="$HOME/.cargo/bin:$PATH"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$ROOT/target}"
TARGET_DIR="$CARGO_TARGET_DIR"

# rust target triple : ABI dir name
#
# Only the 64-bit ABIs: Google Play has required 64-bit since 2019 and
# Android 8+ devices are 64-bit in practice. The 32-bit targets need the
# NDK's x86_64 clang to spawn a subprocess, which does not work on an
# aarch64 host without binfmt_misc.
TARGETS=(
    aarch64-linux-android
    x86_64-linux-android
)
ABIS=(arm64-v8a x86_64)
JNI_LIBS="$ROOT/android/app/src/main/jniLibs"

NDK_WRAPPER="${NDK_WRAPPER:-/opt/ndk-wrappers/ndk-android.sh}"
# Cargo executes the linker by path, so the per-target wrapper scripts live
# next to it as cc-<target>, cxx-<target>, ar-<target>, ranlib-<target> and
# ld-<target>.
NDK_WRAPPER_DIR="${NDK_WRAPPER_DIR:-$(dirname "$NDK_WRAPPER")}"

if [ ! -x "$NDK_WRAPPER" ]; then
    echo ">> NDK wrapper not found at $NDK_WRAPPER" >&2
    echo ">> Set NDK_WRAPPER to a script that forwards <triple> <cmd> [args...]" >&2
    exit 1
fi

if ! command -v cargo-ndk >/dev/null 2>&1; then
    echo ">> Installing cargo-ndk..."
    cargo install cargo-ndk --locked
fi

# rust-toolchain.toml only provides the host toolchain; the Android targets
# must be installed explicitly or rustc fails with E0463 ("can't find crate
# for `core`") on the first cross build.
for target in "${TARGETS[@]}"; do
    echo ">> Ensuring rustup target $target..."
    rustup target add "$target"
done

# Wire the wrapper into cc-rs / rustc per target.
#
# Cargo reads the linker from CARGO_TARGET_<TRIPLE>_LINKER with the triple in
# UPPERCASE — a lowercase name is ignored and rustc falls back to the host
# `cc`, which cannot link an Android library.
for target in "${TARGETS[@]}"; do
    env_key="${target//-/_}"
    env_key_upper="$(echo "$env_key" | tr '[:lower:]' '[:upper:]')"
    export "CC_${env_key}=$NDK_WRAPPER_DIR/cc-$target"
    export "CC_${env_key_upper}=$NDK_WRAPPER_DIR/cc-$target"
    export "CXX_${env_key}=$NDK_WRAPPER_DIR/cxx-$target"
    export "CXX_${env_key_upper}=$NDK_WRAPPER_DIR/cxx-$target"
    export "AR_${env_key}=$NDK_WRAPPER_DIR/ar-$target"
    export "AR_${env_key_upper}=$NDK_WRAPPER_DIR/ar-$target"
    export "RANLIB_${env_key}=$NDK_WRAPPER_DIR/ranlib-$target"
    export "RANLIB_${env_key_upper}=$NDK_WRAPPER_DIR/ranlib-$target"
    export "CARGO_TARGET_${env_key_upper}_LINKER=$NDK_WRAPPER_DIR/ld-$target"
    # zig crashes compiling some 32-bit ARM sources, so that target uses the
    # NDK's own clang under qemu instead.
    if [ "$target" = "armv7-linux-androideabi" ]; then
        export "CC_${env_key}=env CC_BACKEND=ndk $NDK_WRAPPER_DIR/cc-$target"
        export "CC_${env_key_upper}=env CC_BACKEND=ndk $NDK_WRAPPER_DIR/cc-$target"
    fi
done

for i in "${!TARGETS[@]}"; do
    target="${TARGETS[$i]}"
    abi="${ABIS[$i]}"
    echo ">> Building for $abi ($target)..."
    cargo build --release --target "$target" -p arkabot-ffi
    mkdir -p "$JNI_LIBS/$abi"
    cp "$TARGET_DIR/$target/release/libarkabot_ffi.so" "$JNI_LIBS/$abi/libarkabot_ffi.so"
done

echo ">> Done. .so files at $JNI_LIBS"
