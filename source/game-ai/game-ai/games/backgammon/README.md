# ai-backgammon

A small Backgammon engine written in Rust. It implements complete legal-turn
generation, single, gammon, and backgammon outcomes, and weighted expectimax
checker search for cubeless single-game play.

The doubling cube, match scoring, and the Crawford rule are intentionally out
of scope.

Callers supply every dice roll. Search integrates all 21 unordered future
rolls with their exact multiplicities.

The crate treats the two dice as separate moves. Legal plays therefore retain
their checker-step order, while `legal_outcomes` groups orders that reach the
same position for search.

`Turn` filters legal continuations from complete plays for clients that enter a
move one checker at a time. `Game` manages the opening roll and turn phases.
The optional `wasm` feature exposes the same game controller to browser workers.

```rust
use ai_backgammon::{Dice, Position};

let position = Position::new();
let dice = Dice::new(3, 1)?;
let plays = position.legal_plays(dice);
let next = position.play(dice, &plays[0])?;

assert_eq!(next.side_to_move(), position.side_to_move().other());
# Ok::<(), Box<dyn std::error::Error>>(())
```

Search averages all 21 unordered dice outcomes with their exact multiplicities.
Depth counts complete future checker turns.

```rust
use ai_backgammon::{Dice, Position, SearchOptions, Searcher};

let position = Position::new();
let dice = Dice::new(3, 1)?;
let report = Searcher::new().search(
    position,
    dice,
    SearchOptions {
        max_depth: 2,
        node_limit: 100_000,
        soft_time_ms: 0,
    },
);
let best = report.best_play.expect("the opening position has legal plays");
assert!(position.legal_plays(dice).contains(&best));
# Ok::<(), Box<dyn std::error::Error>>(())
```

The paired self-play runner uses identical dice streams with agent colors
reversed:

```text
cargo run --release --bin selfplay -- \
  --pairs 50 --a search:2:100000 --b static --seed 0x7a1c4e90b368d25f
```

Run the checks with:

```text
cargo fmt --check
cargo +1.85.0 check --locked --all-targets --all-features
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --release --locked --all-features
RUSTDOCFLAGS="-D warnings" cargo doc --locked --all-features --no-deps --lib
cargo check --locked --target wasm32-unknown-unknown --no-default-features
cargo check --locked --target wasm32-unknown-unknown --all-features
cargo publish --dry-run
```
