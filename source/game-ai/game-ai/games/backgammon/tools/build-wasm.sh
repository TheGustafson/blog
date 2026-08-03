#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
OUTPUT_DIR="${1:-$REPO_DIR/pkg}"
WASM_PATH="$REPO_DIR/target/wasm32-unknown-unknown/wasm-release/ai_backgammon.wasm"
EXPECTED_WASM_BINDGEN_VERSION="0.2.126"

if ! command -v wasm-bindgen >/dev/null 2>&1; then
  echo "wasm-bindgen CLI $EXPECTED_WASM_BINDGEN_VERSION is required" >&2
  exit 1
fi

ACTUAL_WASM_BINDGEN_VERSION="$(wasm-bindgen --version)"
if [[ "$ACTUAL_WASM_BINDGEN_VERSION" != "wasm-bindgen $EXPECTED_WASM_BINDGEN_VERSION" ]]; then
  echo "wasm-bindgen CLI mismatch: expected $EXPECTED_WASM_BINDGEN_VERSION, got $ACTUAL_WASM_BINDGEN_VERSION" >&2
  exit 1
fi

cargo build \
  --locked \
  --manifest-path "$REPO_DIR/Cargo.toml" \
  --lib \
  --profile wasm-release \
  --target wasm32-unknown-unknown \
  --features wasm

mkdir -p "$OUTPUT_DIR"
wasm-bindgen \
  --target no-modules \
  --no-typescript \
  --out-dir "$OUTPUT_DIR" \
  --out-name backgammon \
  "$WASM_PATH"
