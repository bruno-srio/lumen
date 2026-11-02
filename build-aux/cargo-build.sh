#!/bin/sh
# Build the Lumen binary with Cargo. Meson passes the paths and profile.
set -eu

BUILD_ROOT="$1"
SOURCE_ROOT="$2"
PROFILE="$3"
OUTPUT="$4"

if [ -z "${CARGO_HOME:-}" ]; then
  export CARGO_HOME="${BUILD_ROOT}/cargo-home"
fi

FLAGS=""
TARGET_DIR="debug"
if [ "$PROFILE" = "release" ]; then
  FLAGS="--release"
  TARGET_DIR="release"
fi

# shellcheck disable=SC2086
cargo build $FLAGS \
  --manifest-path "${SOURCE_ROOT}/Cargo.toml" \
  --target-dir "${BUILD_ROOT}/target"
cp "${BUILD_ROOT}/target/${TARGET_DIR}/lumen" "$OUTPUT"
