# ai-othello

An Othello rules and search library with bitboards and complete pass handling.

You can play against it compiled to Wasm here: <https://thegustafson.com/games/othello>

## Self-play

The `selfplay` binary runs deterministic paired-color matches. Each opening is
played twice with the engines swapping Black and White.

```sh
cargo run --release --bin selfplay -- \
  --a maximum --b expert --games 12 --opening-plies 8 --seed 1
```

The six built-in profiles are `beginner`, `easy`, `medium`, `hard`, `expert`,
and `maximum`. They progressively increase ordinary search depth and introduce
an empty-square threshold where the engine solves the rest of the game exactly.
Use the `--a-*` and `--b-*` options shown by `selfplay --help` to override them.
Library users can read the same settings with `search_preset` or
`SEARCH_PRESETS`.

## Use it

Add the crate to your project:

```toml
[dependencies]
ai-othello = "0.1"
```

```rust
use ai_othello::{EvaluationProfile, Position, SearchConfig, search};

let position = Position::default();
let config = SearchConfig::fixed_depth(5, EvaluationProfile::Phase);
let result = search(position, config);

assert!(result.best_move.is_some());
```

The crate includes reversible moves, perft, phase-aware evaluation, alpha-beta
search, exact endgame search, and a Cassio protocol adapter.

```sh
cargo test
cargo run
cargo run -- --cassio
```

Licensed under the MIT License.
