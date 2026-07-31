#![doc = include_str!("../README.md")]
#![forbid(unsafe_code)]

mod mv;
mod play;
mod position;
mod protocol;
mod search;
mod tablebase;
mod trace;

#[cfg(feature = "wasm")]
mod wasm;

pub use mv::{Move, Square};
pub use play::{DecisionReason, DecisionReport, PlayStrategy, choose_move};
pub use position::{GameResult, MoveError, Position, Side};
pub use protocol::Engine;
pub use search::{Algorithm, Candidate, Outcome, SearchReport, SearchStats, perft, search};
pub use tablebase::Tablebase;
pub use trace::{SearchTree, TreeEdge, build_tree};
