# ai-connect4

A Connect Four rules and search library built on two bitboards.

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
