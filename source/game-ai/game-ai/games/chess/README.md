# ai-chess

A small chess rules and search library with classical and NNUE evaluation.

Play against the Wasm engine here: <https://thegustafson.com/games/chess>

## Self-play

The `selfplay` binary runs deterministic paired-color matches. Each opening is
played twice with the engines swapping White and Black.

```sh
cargo run --release --bin selfplay -- \
  --a maximum --b expert --games 12 --opening-plies 6 --seed 1
```

The six built-in profiles are `beginner`, `easy`, `medium`, `hard`, `expert`,
and `maximum`. They all use Tiny NNUE so the matches isolate search strength;
each defines a maximum depth, node budget, time limit, and search features. Use
the `--a-*` and `--b-*` options shown by `selfplay --help` to override them,
including the evaluator when you want to compare evaluation methods. Library
users can read the same settings with `search_preset` or `SEARCH_PRESETS`.

## Use it

Add the crate to your project:

```toml
[dependencies]
ai-chess = "0.1"
```

```rust
use ai_chess::{EvaluationProfile, Position, SearchConfig, iterative_search};

let position = Position::default();
let config = SearchConfig::classical(4, EvaluationProfile::TinyNnue)
    .with_nodes(100_000);
let result = iterative_search(position, config);

assert!(result.result.best_move.is_some());
```

The crate includes legal move generation, FEN and UCI notation, reversible
moves, perft, iterative deepening, quiescence search, and transposition tables.
The included binary speaks UCI.

```sh
cargo test
cargo run
```

Licensed under the MIT License.
