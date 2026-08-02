#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
GAME_AI_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
BLOG_DIR="$(cd "$GAME_AI_DIR/.." && pwd)"
ULTIMATE_OUTPUT_DIR="$BLOG_DIR/public/game-ai/ultimate-tictactoe"
ULTIMATE_WASM_PATH="$GAME_AI_DIR/target/wasm32-unknown-unknown/wasm-release/ai_ultimate_tictactoe.wasm"
CONNECT4_OUTPUT_DIR="$BLOG_DIR/public/game-ai/connect4"
CONNECT4_WASM_PATH="$GAME_AI_DIR/target/wasm32-unknown-unknown/wasm-release/ai_connect4.wasm"
OTHELLO_OUTPUT_DIR="$BLOG_DIR/public/game-ai/othello"
OTHELLO_WASM_PATH="$GAME_AI_DIR/target/wasm32-unknown-unknown/wasm-release/ai_othello.wasm"
CHESS_OUTPUT_DIR="$BLOG_DIR/public/game-ai/chess"
CHESS_WASM_PATH="$GAME_AI_DIR/target/wasm32-unknown-unknown/wasm-release/ai_chess.wasm"
HEX_OUTPUT_DIR="$BLOG_DIR/public/game-ai/hex"
HEX_WASM_PATH="$GAME_AI_DIR/target/wasm32-unknown-unknown/wasm-release/ai_hex.wasm"
EXPECTED_WASM_BINDGEN_VERSION="0.2.126"

if ! command -v wasm-bindgen >/dev/null 2>&1; then
  echo "wasm-bindgen CLI $EXPECTED_WASM_BINDGEN_VERSION is required" >&2
  exit 1
fi

ACTUAL_WASM_BINDGEN_VERSION="$(wasm-bindgen --version)"
if [[ "$ACTUAL_WASM_BINDGEN_VERSION" != "wasm-bindgen $EXPECTED_WASM_BINDGEN_VERSION" ]]; then
  echo \
    "wasm-bindgen CLI mismatch: expected $EXPECTED_WASM_BINDGEN_VERSION, got $ACTUAL_WASM_BINDGEN_VERSION" \
    >&2
  exit 1
fi

cargo build \
  --locked \
  --manifest-path "$GAME_AI_DIR/Cargo.toml" \
  --package ai-ultimate-tictactoe \
  --lib \
  --profile wasm-release \
  --target wasm32-unknown-unknown \
  --no-default-features \
  --features wasm

mkdir -p "$ULTIMATE_OUTPUT_DIR"
wasm-bindgen \
  --target no-modules \
  --no-typescript \
  --out-dir "$ULTIMATE_OUTPUT_DIR" \
  --out-name ultimate-tictactoe \
  "$ULTIMATE_WASM_PATH"
cp "$GAME_AI_DIR/browser/ultimate-tictactoe.worker.js" "$ULTIMATE_OUTPUT_DIR/worker.js"

cargo build \
  --locked \
  --manifest-path "$GAME_AI_DIR/Cargo.toml" \
  --package ai-ultimate-tictactoe \
  --lib \
  --profile wasm-release \
  --target wasm32-unknown-unknown \
  --features wasm,mcts

wasm-bindgen \
  --target no-modules \
  --no-typescript \
  --out-dir "$ULTIMATE_OUTPUT_DIR" \
  --out-name ultimate-tictactoe-mcts \
  "$ULTIMATE_WASM_PATH"
cp "$GAME_AI_DIR/browser/ultimate-tictactoe-mcts.worker.js" "$ULTIMATE_OUTPUT_DIR/mcts-worker.js"

cargo build \
  --locked \
  --manifest-path "$GAME_AI_DIR/Cargo.toml" \
  --package ai-connect4 \
  --profile wasm-release \
  --target wasm32-unknown-unknown \
  --features wasm

mkdir -p "$CONNECT4_OUTPUT_DIR"
wasm-bindgen \
  --target no-modules \
  --no-typescript \
  --out-dir "$CONNECT4_OUTPUT_DIR" \
  --out-name connect4 \
  "$CONNECT4_WASM_PATH"
cp "$GAME_AI_DIR/browser/connect4.worker.js" "$CONNECT4_OUTPUT_DIR/worker.js"

cargo build \
  --locked \
  --manifest-path "$GAME_AI_DIR/Cargo.toml" \
  --package ai-othello \
  --profile wasm-release \
  --target wasm32-unknown-unknown \
  --features wasm

mkdir -p "$OTHELLO_OUTPUT_DIR"
wasm-bindgen \
  --target no-modules \
  --no-typescript \
  --out-dir "$OTHELLO_OUTPUT_DIR" \
  --out-name othello \
  "$OTHELLO_WASM_PATH"
cp "$GAME_AI_DIR/browser/othello.worker.js" "$OTHELLO_OUTPUT_DIR/worker.js"

cargo build \
  --locked \
  --manifest-path "$GAME_AI_DIR/Cargo.toml" \
  --package ai-chess \
  --profile wasm-release \
  --target wasm32-unknown-unknown \
  --features wasm

mkdir -p "$CHESS_OUTPUT_DIR"
wasm-bindgen \
  --target no-modules \
  --no-typescript \
  --out-dir "$CHESS_OUTPUT_DIR" \
  --out-name chess \
  "$CHESS_WASM_PATH"
cp "$GAME_AI_DIR/browser/chess.worker.js" "$CHESS_OUTPUT_DIR/worker.js"

cargo build \
  --locked \
  --manifest-path "$GAME_AI_DIR/Cargo.toml" \
  --package ai-hex \
  --profile wasm-release \
  --target wasm32-unknown-unknown \
  --features wasm

mkdir -p "$HEX_OUTPUT_DIR"
wasm-bindgen \
  --target no-modules \
  --no-typescript \
  --out-dir "$HEX_OUTPUT_DIR" \
  --out-name hex \
  "$HEX_WASM_PATH"
cp "$GAME_AI_DIR/browser/hex.worker.js" "$HEX_OUTPUT_DIR/worker.js"
