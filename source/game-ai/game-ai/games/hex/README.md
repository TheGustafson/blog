# ai-hex

A small Hex engine written in Rust. It supports boards from 9×9 through 24×24,
the swap rule, seeded UCT and UCT-RAVE search, and WebAssembly. Fixed-simulation
searches are reproducible; timed searches stop at a soft wall-clock limit.

The default search uses UCT-RAVE with MCTS-Solver, bounded virtual-connection
search, inferior-cell pruning, and bridge-aware rollouts. The rules and search
APIs are independent of the command protocol and browser integration.

```rust
use ai_hex::{BoardSize, Move, Position, SwapRule};

let size = BoardSize::new(13)?;
let position = Position::new(size, SwapRule::Enabled);
let next = position.play("h8".parse::<Move>()?)?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

Run the engine protocol:

```text
cargo run
newgame size 13 swap on
position size 13 swap on moves g7 swap
mcts simulations 20000 softtime 500 exploration 0.2 strategy uct-rave rave 1000 rollout save-bridge knowledge 32 connections on seed 1
state
```

Use `strategy plain-uct`, `rollout random`, `connections off`, or `knowledge off`
to run feature ablations. The `selfplay` binary runs paired-opening
matches with independent settings for each side:

```text
cargo run --release --bin selfplay -- --games 20 --size 13 --simulations 4000
```

Run the tests with `cargo test`. The browser build uses the optional `wasm`
feature.
