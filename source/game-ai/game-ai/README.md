# Game AI integration

The browser games use six independent Rust crates:

- [`ai-backgammon`](games/backgammon/README.md)
- [`ai-ultimate-tictactoe`](games/ultimate-tictactoe/README.md)
- [`ai-connect4`](games/connect4/README.md)
- [`ai-hex`](games/hex/README.md)
- [`ai-othello`](games/othello/README.md)
- [`ai-chess`](games/chess/README.md)

Each directory under `games/` is the exact tagged source used for the published
WebAssembly engine. The workspace manifest builds and tests the six crates
together without imposing a shared game framework.

From the corresponding-source root:

```bash
cargo check --locked --manifest-path game-ai/Cargo.toml --workspace --all-targets --all-features
cargo clippy --locked --manifest-path game-ai/Cargo.toml --workspace --all-targets --all-features -- -D warnings
cargo test --release --locked --manifest-path game-ai/Cargo.toml --workspace --all-features
RUSTDOCFLAGS="-D warnings" cargo doc --locked --manifest-path game-ai/Cargo.toml --workspace --all-features --no-deps --lib
bash game-ai/tools/build-wasm.sh
```

The documentation command is library-only because several crates ship a binary
named `selfplay`. The browser workers under `browser/` send commands and render
snapshots; rules and search stay in Rust.
