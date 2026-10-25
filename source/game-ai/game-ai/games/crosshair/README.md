# ai-crosshair

A compact **Crosshair** connection game and search engine written in Rust. It
provides complete rules, AlphaZero-style PUCT search with an embedded residual
policy-value network, handcrafted PUCT, rollout UCT, iterative-deepening
alpha-beta search, and WebAssembly bindings.

Play the WebAssembly build at <https://thegustafson.com/games/crosshair>.

## Rules

- **Board and objective:** Played on a 7×7 grid (`a1` through `g7`, 49 cells).
  Red (`R`) moves first and wins by joining the top edge (`a1..g1`, rank 0) to
  the bottom edge (`a7..g7`, rank 6) with a 4-orthogonal chain of Red stones.
  Blue (`B`) moves second and wins by joining the left edge (`a1..a7`, file 0)
  to the right edge (`g1..g7`, file 6) with a 4-orthogonal chain of Blue stones.
- **Crosshair constraint:** The first move may use any of the 49 cells. Every
  subsequent move at `(file, rank)` constrains the next move to empty cells in
  that same column (`file`) or row (`rank`). If both that column and row are
  full, wildcard activates and the player to move may choose any empty cell on
  the board.
- **Pinch flips and zero draws:** On a 4-orthogonal square grid, pure placement
  can deadlock where two diagonal pairs cross inside a 2×2 square (`R B / B R`).
  Crosshair resolves every move by placing a stone of color `C` and applying two
  capture steps in order:
  1. **Linear custodial pinch:** In each of the four cardinal directions
     `(±1, 0)` and `(0, ±1)`, if the adjacent cell holds an opponent stone and
     the cell two steps away holds a friendly stone (`R [B] R` or `B [R] B`),
     the sandwiched opponent stone flips to `C`.
  2. **2×2 crosscut pinch:** In any 2×2 sub-square where a newly placed or
     flipped stone of color `C` and its diagonal mate are both `C` while the
     other two orthogonal corners are both `C.opponent()` (`R B / B R`), both
     opponent stones flip to `C`. Crosscut flips cascade if they complete
     additional 2×2 crosscuts. Because no 2×2 checkerboard deadlock can survive,
     every game terminates with a connection win and zero draws.

## Use the crate

```toml
[dependencies]
ai-crosshair = "0.1"
```

```rust
use ai_crosshair::{Color, Move, Position};

let position = Position::new()
    .play("d4".parse::<Move>()?)?
    .play("d5".parse::<Move>()?)?;
assert_eq!(position.side_to_move(), Color::Red);
assert_eq!(position.active_col(), Some(3));
assert_eq!(position.active_row(), Some(4));
assert!(position.legal_moves().contains("d6".parse::<Move>()?));

// Playing d6 sandwiches Blue's stone at d5 between d4 and d6, flipping d5 to Red.
let next = position.play("d6".parse::<Move>()?)?;
assert_eq!(next.color_at("d5".parse::<Move>()?), Some(Color::Red));
assert_eq!(next.counts(), (3, 0));
# Ok::<(), Box<dyn std::error::Error>>(())
```

## Search strategies

`MctsSearcher` provides a unified interface across all five search strategies:

- `az-puct`: AlphaZero-style PUCT guided by the embedded 2-residual-block MLP
  policy-value network (`CXHRNET1`, 114,290 parameters). When Blue is to move,
  the board is transposed before encoding so the active player always connects
  top to bottom in canonical coordinates.
- `tactical-uct`: UCT with tactically biased rollouts that prioritize immediate
  wins, one-move road blocks, pinch flips, and connection span growth.
- `handcrafted-puct`: PUCT with connection-and-pinch move priors and a static
  handcrafted leaf evaluator.
- `random-uct`: Vanilla UCT with uniform random rollouts.
- `alpha-beta`: Iterative-deepening principal variation search with a
  65,536-entry Zobrist transposition table, history heuristic, and tactical move
  ordering.

```rust
use ai_crosshair::{MctsOptions, MctsSearcher, Position, Strategy};

let mut searcher = MctsSearcher::new();
let options = MctsOptions {
    max_simulations: 200,
    strategy: Strategy::AzPuct,
    ..MctsOptions::default()
};
let report = searcher.search(Position::new(), options);
assert!(report.best_move.is_some());
```

## Protocol and command-line tools

Run the line-oriented engine protocol:

```text
cargo run
newgame
position startpos moves d4 d5 d6
search strategy az-puct simulations 2000 softtime 300 seed 1
state
```

The `selfplay` binary supports self-play data generation, paired-opening arena
matches, and forward-pass parity verification for the JAX GPU training pipeline
in `training/`:

```sh
# Generate self-play training samples with root Dirichlet noise and V4 symmetry augmentation
cargo run --release --bin selfplay -- --datagen \
  --games 200 --simulations 400 --augment-symmetries --out selfplay.cxhrdat

# Run a paired-opening arena match with colors swapped per opening
cargo run --release --bin selfplay -- --arena \
  --games 20 --simulations 400 \
  --candidate az-puct --baseline tactical-uct

# Verify bit-exact forward-pass parity against a CXHRDAT1 sample file
cargo run --release --bin selfplay -- --verify-forward selfplay.cxhrdat embedded --count 8
```

## Development

```sh
cargo fmt --check
cargo +1.85.0 check --locked --all-targets --all-features
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --release --locked --all-features
RUSTDOCFLAGS="-D warnings" cargo doc --locked --all-features --no-deps --lib
```

The browser build uses the optional `wasm` feature (`--features wasm`).

Licensed under the MIT License.
