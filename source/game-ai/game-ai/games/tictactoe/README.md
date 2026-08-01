# ai-tictactoe

A tic-tac-toe rules and search library with perfect play.

## Use it

Add the crate to your project:

```toml
[dependencies]
ai-tictactoe = "0.1"
```

Build the tablebase once, then use it for each position:

```rust
use ai_tictactoe::{Algorithm, Position, Tablebase, search};

let position = Position::default();
let tablebase = Tablebase::build();
let result = search(position, Algorithm::Tablebase, &tablebase);

assert!(result.best_move.is_some());
```

The crate also includes negamax, memoized search, symmetry reduction, and a
small line-oriented engine.

```sh
cargo test
cargo run
```

Licensed under the MIT License.
