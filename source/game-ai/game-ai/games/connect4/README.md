# ai-connect4

A Connect Four rules and search library built on two bitboards.

Play against it compiled to Wasm here: <https://thegustafson.com/games/connect-four>

## Self-play

The `selfplay` binary runs deterministic paired-color matches. Each opening is
played twice with the engines swapping Red and Yellow.

```sh
cargo run --release --bin selfplay -- \
  --a maximum --b expert --games 12 --opening-plies 4 --seed 1
```

The six built-in profiles are `beginner`, `easy`, `medium`, `hard`, `expert`,
and `maximum`. Use `--a-depth`, `--a-nodes`, and the corresponding `--b-*`
options to override their limits. Library users can read the same settings with
`search_preset` or `SEARCH_PRESETS`.

## Use it

Add the crate to your project:

```toml
[dependencies]
ai-connect4 = "0.1"
```

```rust
use ai_connect4::{Algorithm, Move, Position, SearchLimits, search};

let mut position = Position::default();
position
    .make_move("4".parse::<Move>().expect("valid column"))
    .expect("legal move");

let result = search(
    position,
    Algorithm::TranspositionTable,
    SearchLimits::depth(7),
);

assert!(result.best_move.is_some());
```

The crate includes reversible moves, perft, alpha-beta search, move ordering,
and an optional transposition table.

```sh
cargo test
cargo run
```

Licensed under the MIT License.
