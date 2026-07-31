# Game AI integration

The browser games use four independent Rust crates:

- [`gai-tictactoe`](games/tictactoe/README.md)
- [`gai-connect4`](games/connect4/README.md)
- [`gai-othello`](games/othello/README.md)
- [`gai-chess`](games/chess/README.md)

Each crate is maintained in its own public repository. The directories under
`games/` are Git submodules pinned to exact release commits. The workspace
manifest is only a convenience for building and testing the four engines with
the browser integration.

The crates deliberately share no game framework. Their public interfaces use
the same basic shape—`Position`, legal moves, make/unmake, a search
configuration, and a search report—without forcing unrelated games through one
trait.

From the blog repository root:

```bash
cargo test --release --manifest-path game-ai/Cargo.toml --workspace --all-features
cargo +1.85.0 check --manifest-path game-ai/Cargo.toml --workspace --all-targets --all-features
cargo doc --manifest-path game-ai/Cargo.toml --workspace --all-features --no-deps
bash game-ai/tools/build-wasm.sh
```

Clone the authoring repository with its engine pins using:

```bash
git clone --recurse-submodules <private-blog-url>
```

For an existing checkout, initialize or restore the recorded pins with
`git submodule update --init --recursive`. Run
`game-ai/tools/check-engine-pins.sh` to verify that every engine is clean and
checked out at the tag matching its Cargo version.

`tools/build-wasm.sh` compiles the optional browser bindings and pairs them
with the workers under `browser/`. The React code only sends commands and
renders snapshots; rules and search stay in Rust.
