# ai-ultimate-tictactoe

A compact Ultimate Tic-Tac-Toe engine written in Rust. It implements the
wildcard rule and provides alpha-beta, UCT, and PUCT search behind a small
library API.

Play the WebAssembly build at
<https://thegustafson.com/games/tic-tac-toe>.

## Rules

- The first move may use any of the 81 cells.
- The local cell played determines which mini-board the opponent must use.
- Three marks in a row claim a mini-board. A claimed board closes immediately.
- A full mini-board without a winner is drawn and also closes.
- If the destination board is closed, the opponent may use any empty cell in
  any open board.
- Three claimed mini-boards in a row win. If every mini-board closes without a
  macro line, the game is drawn.

## Use the crate

```toml
[dependencies]
ai-ultimate-tictactoe = "0.2"
```

```rust
use ai_ultimate_tictactoe::{Move, Position};

let position = Position::start()
    .play(Move::new(4, 0))
    .expect("the opening move is legal");
assert_eq!(position.active_board(), Some(0));
```

The primary types are `Position`, `Move`, `Searcher`, and `MctsSearcher`.
`SEARCH_PRESETS` and `MCTS_PRESETS` provide the same six strength levels used
by the browser game.

## Search

The alpha-beta engine uses iterative deepening, principal variation search, a
transposition table, tactical move ordering, threat extensions, and a
handcrafted evaluation. Mini-board tactics come from a precomputed table for
all 19,683 ternary cell patterns.

The Monte Carlo searcher supports:

- UCT with random rollouts;
- UCT with tactical rollouts;
- PUCT with handcrafted move priors;
- PUCT with a small learned move-prior policy.

PUCT evaluates leaves with the same handcrafted evaluator in both modes.
Initial learned-value experiments did not improve match play, so the shipped
engine focuses on the methods that produced useful results: learned priors,
handcrafted evaluation, and rollout search.

```rust
# #[cfg(feature = "mcts")]
# fn main() {
use ai_ultimate_tictactoe::{MctsSearcher, Position, mcts_preset};

let options = mcts_preset("medium").unwrap().options;
let report = MctsSearcher::new().search(Position::start(), options);
assert!(report.best_move.is_some());
# }
# #[cfg(not(feature = "mcts"))]
# fn main() {}
```

## Command-line tools

Run a game against the alpha-beta engine:

```sh
cargo run
```

Compare MCTS with alpha-beta or another Monte Carlo configuration using paired
openings:

```sh
cargo run --release --bin mcts-match -- \
  --mcts medium --strategy learned-puct \
  --opponent tactical --games 20
```

Run every shipped MCTS configuration against every other configuration:

```sh
cargo run --release --bin mcts-round-robin
```

The optional `training` feature trains another policy generation from PUCT
self-play visit distributions. Training data is split by complete games, and
the command writes a candidate under `target/` so it cannot replace the
embedded policy accidentally. Compare a candidate in match play before
promoting it into `src/networks/`.

```sh
cargo run --release --features training --bin train-mcts
```

## Development

```sh
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --release --all-features
cargo doc --no-deps --all-features
```

The library can also be built with the `wasm` feature for browser use.

Licensed under the MIT License.
