#![doc = include_str!("../README.md")]
#![forbid(unsafe_code)]

mod mcts;
mod network;
mod position;
mod protocol;
mod search;

#[cfg(feature = "wasm")]
mod wasm;

pub use mcts::{
    MCTS_PRESETS, MctsMoveStats, MctsOptions, MctsPreset, MctsReport, MctsSearcher, Strategy,
    mcts_preset,
};
pub use network::{
    ACTIONS, B_POL, B_RES1A, B_RES1B, B_RES2A, B_RES2B, B_STEM, B_VAL1, B_VAL2, HEADER_BYTES,
    HIDDEN, INPUT_CHANNELS, INPUTS, MAGIC, PARAM_COUNT, PolicyValueNetwork, VALUE_HIDDEN, VERSION,
    W_POL, W_RES1A, W_RES1B, W_RES2A, W_RES2B, W_STEM, W_VAL1, W_VAL2, canonical_index, checksum,
    encode, transform_index, transpose_index,
};
pub use position::{
    BOARD_MASK, BOTTOM_EDGE_MASK, CELLS, Color, GameResult, LEFT_EDGE_MASK, Move, MoveError,
    MoveIter, MoveList, MovePreview, ParseMoveError, Position, RIGHT_EDGE_MASK, SIZE,
    TOP_EDGE_MASK, WinReason,
};
pub use protocol::Engine;
pub use search::{
    SEARCH_PRESETS, SearchBranch, SearchOptions, SearchPreset, SearchReport, Searcher,
    connection_potential, evaluate_normalized, evaluate_position, normalized_tactical_priors,
    score_to_expected, search_preset, tactical_move_score,
};

#[cfg(feature = "wasm")]
pub use wasm::CrosshairSession;

/// Counts legal leaf positions at `depth` plies, stopping at terminal games.
pub fn perft(position: Position, depth: u8) -> u64 {
    if depth == 0 || position.result() != GameResult::Ongoing {
        return 1;
    }
    position
        .legal_moves()
        .iter()
        .map(|mv| {
            let child = position
                .play(mv)
                .expect("legal move generation must agree with play");
            perft(child, depth - 1)
        })
        .sum()
}
