#![doc = include_str!("../README.md")]
#![forbid(unsafe_code)]

mod attacks;
mod evaluation;
mod fen;
mod movegen;
mod mv;
mod nnue;
mod perft;
mod position;
mod protocol;
mod psqt_tuned;
mod search;
mod types;
mod zobrist;

#[cfg(feature = "wasm")]
mod wasm;

pub use evaluation::{
    Evaluation, EvaluationProfile, PieceContribution, classical_piece_value, evaluate,
    piece_contributions,
};
pub use mv::{Move, MoveKind, MoveList};
pub use nnue::{
    Accumulator as NnueAccumulator, FEATURE_COUNT as NNUE_FEATURES, FeatureDelta,
    FloatNetwork as FloatNnueNetwork, HIDDEN as NNUE_HIDDEN, NetworkError, QA as NNUE_QA,
    QB as NNUE_QB, QuantizedNetwork as QuantizedNnueNetwork, builtin_network as builtin_nnue,
    feature_index as nnue_feature_index,
};
pub use perft::{divide, perft};
pub use position::{CastlingRights, GameResult, MoveError, Position, Undo};
pub use protocol::Engine;
pub use search::{
    Candidate, IterationSummary, IterativeSearchReport, SEARCH_PRESETS, Score, ScoreKind,
    SearchConfig, SearchPreset, SearchReport, SearchStats, iterative_search,
    iterative_search_with_history, search, search_preset, search_with_history,
};
pub use types::{Color, Piece, PieceKind, Square};
