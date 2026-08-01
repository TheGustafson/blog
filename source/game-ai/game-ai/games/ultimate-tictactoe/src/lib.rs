mod lookup;
mod mv;
mod position;
mod protocol;
mod search;

#[cfg(feature = "wasm")]
mod wasm;

pub use mv::{Move, ParseMoveError};
pub use position::{
    GameResult, MiniResult, MoveError, MoveList, Player, Position, PositionStateError,
};
pub use protocol::Engine;
pub use search::{
    SEARCH_PRESETS, SearchOptions, SearchPreset, SearchReport, Searcher, search_preset,
};

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
