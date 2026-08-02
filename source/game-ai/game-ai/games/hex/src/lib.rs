#![doc = include_str!("../README.md")]
#![forbid(unsafe_code)]

mod board;
mod cell_set;
mod connectivity;
mod knowledge;
mod mcts;
mod patterns;
mod position;
mod protocol;
mod virtual_connection;

#[cfg(feature = "wasm")]
mod wasm;

pub use board::{BoardSize, BoardSizeError, Cell, Move, ParseMoveError};
pub use mcts::{
    KnowledgePolicy, MCTS_PRESETS, MctsMoveStats, MctsOptions, MctsPreset, MctsReport,
    MctsSearcher, MctsStrategy, RolloutPolicy, mcts_preset,
};
pub use position::{Color, GameResult, MoveError, Position, Seat, SwapRule};
pub use protocol::Engine;

#[cfg(feature = "wasm")]
pub use wasm::HexSession;
