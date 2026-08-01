# ai-ultimate-tictactoe

A Rust rules and search library for Ultimate Tic-Tac-Toe with the wildcard
rule.

Play against it compiled to Wasm here: <https://thegustafson.com/games/tic-tac-toe>

## Rules

- The first move may use any of the 81 cells.
- The local cell played determines which mini-board the opponent must use.
- Three marks in a row claim a mini-board. A claimed board closes immediately.
- A full mini-board without a winner is drawn and also closes.
- If the destination board is closed, the opponent may use any empty cell in
  any open board.
- Three claimed mini-boards in a row win. If every mini-board closes without a
  macro line, the game is drawn.

## Self-play

The `selfplay` binary runs deterministic paired-color matches. Each randomized
opening is played twice with the engines swapping X and O.

```sh
cargo run --release --bin selfplay -- \
  --a maximum --b expert --games 12 --opening-plies 4 --seed 1
```

The six built-in profiles are `beginner`, `easy`, `medium`, `hard`, `expert`,
and `maximum`. Use `--a-depth`, `--a-nodes`, `--a-time`, and the corresponding
`--b-*` options to override their limits. Library users can read the same
settings with `search_preset` or `SEARCH_PRESETS`.

## Use it

Add the crate to your project:

```toml
[dependencies]
ai-ultimate-tictactoe = "0.1"
```

```rust
use ai_ultimate_tictactoe::{Move, Position};

let position = Position::start().play(Move::new(4, 0))?;
assert_eq!(position.active_board(), Some(0));
# Ok::<(), ai_ultimate_tictactoe::MoveError>(())
```

The engine uses iterative-deepening PVS alpha-beta search, a transposition
table, tactical move ordering, a threat extension, and a handcrafted
evaluation. Mini-board tactics come from a precomputed table covering all
19,683 ternary cell patterns.

```sh
cargo test
cargo run
```

Licensed under the MIT License.
