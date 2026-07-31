#![doc = include_str!("../README.md")]
#![forbid(unsafe_code)]

mod mv;
mod oracle;
mod perft;
mod position;
mod protocol;
mod reference;
mod search;

#[cfg(feature = "wasm")]
mod wasm;

pub use mv::{Cell, Column, Move, MoveList};
pub use perft::perft;
pub use position::{GameResult, HEIGHT, MoveError, Position, Side, Undo, WIDTH};
pub use protocol::Engine;
pub use search::{
    Algorithm, IterationSummary, IterativeSearchReport, RootBranch, Score, ScoreKind, SearchLimits,
    SearchReport, SearchStats, iterative_search, search,
};

/// Slow reference code and fixed oracle cases used to verify the engine.
///
/// These types favor obvious rules over speed and are useful for differential
/// tests of alternative Connect Four representations.
pub mod verification {
    pub use crate::oracle::{ORACLE_CASES, OracleCase, OracleOutcome, probe_oracle};
    pub use crate::perft::reference_perft;
    pub use crate::reference::{ReferencePosition, ReferenceUndo};
}
