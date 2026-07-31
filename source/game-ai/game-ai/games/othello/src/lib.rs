#![doc = include_str!("../README.md")]
#![forbid(unsafe_code)]

mod evaluation;
mod mv;
mod perft;
mod position;
mod protocol;
mod reference;
mod search;

#[cfg(not(target_arch = "wasm32"))]
mod cassio;

#[cfg(feature = "wasm")]
mod wasm;

pub use evaluation::{Evaluation, EvaluationProfile, EvaluationWeights, evaluate};
pub use mv::{Move, MoveList, Square};
pub use perft::perft;
pub use position::{GameResult, MoveError, Position, Side, Undo};
pub use protocol::Engine;
pub use search::{
    Candidate, Score, ScoreKind, SearchConfig, SearchReport, SearchStats, search, search_until,
};

#[cfg(not(target_arch = "wasm32"))]
pub use cassio::CassioEngine;

/// Slow reference code used to verify the bitboard implementation.
///
/// It favors straightforward ray walking over the production engine's
/// bit-parallel operations.
pub mod verification {
    pub use crate::perft::reference_perft;
    pub use crate::reference::{ReferencePosition, ReferenceUndo};
}
