# ai-chess

A small chess rules and search library with classical and NNUE evaluation.

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
