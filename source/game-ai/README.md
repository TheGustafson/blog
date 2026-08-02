# Game AIs corresponding source

This directory contains the source used to build the Game AIs distributed at
[thegustafson.com/games](https://thegustafson.com/games).

It is deliberately narrower than the private authoring repository. It includes
four self-contained Rust crates, WebAssembly worker glue, game routes and React
components, shared layout files needed by those routes, exact package
manifests, and the source and license for Chessground 10.1.1. It excludes blog
drafts, notes, training corpora, generated build directories, and unrelated
site components.

`SOURCE-REVISION` records the private authoring commit. `ENGINE-REVISIONS`
records the public release tag and commit for each engine.
`SOURCE-MANIFEST.sha256` records every file in this bundle.

## Rebuild

Use Node.js 24, a current stable Rust toolchain, and
`wasm-bindgen-cli 0.2.126`.

```bash
npm ci
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.126 --locked
npm run build
```

The package build first compiles the five Rust engines to WebAssembly and then
exports the Game AI routes with Next.js. The static output is written to
`out/`.

Run the engine tests with:

```bash
npm test
```

Each directory under `game-ai/games/` is a source snapshot of its independently
versioned public repository. It has its own README, license, manifest, and
tests and can be built or packaged without the other engines.

## License

The browser work in this bundle is distributed under GPL-3.0-or-later. See
`LICENSE` and `SOURCE-NOTICE.md`.
