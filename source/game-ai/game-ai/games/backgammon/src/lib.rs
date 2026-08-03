#![doc = include_str!("../README.md")]
#![forbid(unsafe_code)]

mod board;
mod dice;
mod evaluation;
mod game;
mod movegen;
mod play;
mod protocol;
mod reference;
mod search;
/// Deterministic paired-game tools for screening agents with shared dice streams.
pub mod selfplay;
mod turn;
#[cfg(feature = "wasm")]
mod wasm;

pub use board::{GameKind, GameOutcome, Player, Point, Position, PositionError, Undo};
pub use dice::{DICE_OUTCOMES, Dice, DiceError, DiceOutcome};
pub use evaluation::{Equity, evaluate_position, pip_count};
pub use game::{Game, GameError, GamePhase};
pub use play::{Location, Play, PlayError, PlayOutcome, Step, StepError};
pub use protocol::Engine;
pub use search::{
    SEARCH_PRESETS, SearchOptions, SearchPreset, SearchReport, Searcher, search_preset,
};
pub use turn::{Turn, TurnError};
#[cfg(feature = "wasm")]
pub use wasm::BackgammonSession;

#[doc(hidden)]
pub mod verification {
    pub use crate::reference::legal_plays as reference_legal_plays;
}
