#![doc = include_str!("../README.md")]
#![forbid(unsafe_code)]

mod lookup;
#[cfg(feature = "mcts")]
mod mcts;
mod mv;
#[cfg(feature = "mcts")]
mod network;
mod position;
mod protocol;
mod search;

#[cfg(feature = "training")]
mod training;

#[cfg(feature = "wasm")]
mod wasm;

#[cfg(feature = "mcts")]
pub use mcts::{
    MCTS_PRESETS, MctsMoveStats, MctsOptions, MctsPreset, MctsReport, MctsSearcher, MctsStrategy,
    mcts_preset,
};
pub use mv::{Move, ParseMoveError};
pub use position::{
    GameResult, MiniResult, MoveError, MoveList, Player, Position, PositionStateError,
};
pub use protocol::Engine;
pub use search::{
    SEARCH_PRESETS, SearchOptions, SearchPreset, SearchReport, Searcher, search_preset,
};

#[cfg(feature = "training")]
pub use training::{PolicyTrainingConfig, train_policy};

/// Counts legal leaf positions and stops at terminal games.
pub fn perft(position: Position, depth: u8) -> u64 {
    if depth == 0 {
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

#[cfg(feature = "wasm")]
pub use wasm::UltimateSession;
