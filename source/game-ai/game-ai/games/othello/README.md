# ai-othello

An Othello rules and search library with bitboards and complete pass handling.

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
