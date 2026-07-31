use crate::mv::Move;
use crate::position::Position;
use crate::search::{Algorithm, Outcome, search};
use crate::tablebase::Tablebase;
use std::fmt;
use std::str::FromStr;

/// A move-selection policy used for the playable opponent.
///
/// The first two policies are intentionally fallible. The remaining four are
/// exact solvers and exist separately so each opponent changes only the move
/// picker, not the rules underneath.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum PlayStrategy {
    Random,
    Tactical,
    Plain,
    Memo,
    Symmetry,
    #[default]
    Tablebase,
}

impl PlayStrategy {
    pub const ALL: [Self; 6] = [
        Self::Random,
        Self::Tactical,
        Self::Plain,
        Self::Memo,
        Self::Symmetry,
        Self::Tablebase,
    ];

    pub const fn name(self) -> &'static str {
        match self {
            Self::Random => "random",
            Self::Tactical => "tactical",
            Self::Plain => "plain",
            Self::Memo => "memo",
            Self::Symmetry => "symmetry",
            Self::Tablebase => "tablebase",
        }
    }

    const fn algorithm(self) -> Option<Algorithm> {
        match self {
            Self::Random | Self::Tactical => None,
            Self::Plain => Some(Algorithm::Plain),
            Self::Memo => Some(Algorithm::Memo),
            Self::Symmetry => Some(Algorithm::Symmetry),
            Self::Tablebase => Some(Algorithm::Tablebase),
        }
    }
}

impl fmt::Display for PlayStrategy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

impl FromStr for PlayStrategy {
    type Err = &'static str;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.to_ascii_lowercase().as_str() {
            "random" => Ok(Self::Random),
            "tactical" | "oneply" | "one-ply" => Ok(Self::Tactical),
            "plain" | "negamax" => Ok(Self::Plain),
            "memo" | "memoized" => Ok(Self::Memo),
            "symmetry" | "canonical" => Ok(Self::Symmetry),
            "tablebase" | "perfect" => Ok(Self::Tablebase),
            _ => Err("strategy must be random, tactical, plain, memo, symmetry, or tablebase"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecisionReason {
    RandomChoice,
    ImmediateWin,
    ImmediateBlock,
    PositionalFallback,
    ExactSearch,
}

impl DecisionReason {
    pub const fn name(self) -> &'static str {
        match self {
            Self::RandomChoice => "random-choice",
            Self::ImmediateWin => "immediate-win",
            Self::ImmediateBlock => "immediate-block",
            Self::PositionalFallback => "positional-fallback",
            Self::ExactSearch => "exact-search",
        }
    }
}

impl fmt::Display for DecisionReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecisionReport {
    pub strategy: PlayStrategy,
    pub best_move: Move,
    pub reason: DecisionReason,
    pub nodes: u64,
    pub cache_hits: u64,
    pub outcome: Option<Outcome>,
    pub distance: Option<u8>,
}

/// Chooses one move with `strategy`.
///
/// Returns `None` when the position has no legal moves. `random_seed` affects
/// only [`PlayStrategy::Random`].
pub fn choose_move(
    position: Position,
    strategy: PlayStrategy,
    random_seed: u64,
    tablebase: &Tablebase,
) -> Option<DecisionReport> {
    let legal_moves: Vec<_> = position.legal_moves().collect();
    let &fallback = legal_moves.first()?;

    if let Some(algorithm) = strategy.algorithm() {
        let report = search(position, algorithm, tablebase);
        return Some(DecisionReport {
            strategy,
            best_move: report
                .best_move
                .expect("an ongoing position has an exact best move"),
            reason: DecisionReason::ExactSearch,
            nodes: report.stats.nodes,
            cache_hits: report.stats.cache_hits,
            outcome: Some(report.outcome),
            distance: Some(report.distance),
        });
    }

    if strategy == PlayStrategy::Random {
        let mixed = splitmix64(random_seed ^ position.key() as u64);
        let index = mixed as usize % legal_moves.len();
        return Some(DecisionReport {
            strategy,
            best_move: legal_moves[index],
            reason: DecisionReason::RandomChoice,
            nodes: 0,
            cache_hits: 0,
            outcome: None,
            distance: None,
        });
    }

    let mut nodes = 0;
    for &mv in &legal_moves {
        nodes += 1;
        if position.is_winning_move(position.side_to_move(), mv) {
            return Some(DecisionReport {
                strategy,
                best_move: mv,
                reason: DecisionReason::ImmediateWin,
                nodes,
                cache_hits: 0,
                outcome: None,
                distance: None,
            });
        }
    }
    for &mv in &legal_moves {
        nodes += 1;
        if position.is_winning_move(position.side_to_move().other(), mv) {
            return Some(DecisionReport {
                strategy,
                best_move: mv,
                reason: DecisionReason::ImmediateBlock,
                nodes,
                cache_hits: 0,
                outcome: None,
                distance: None,
            });
        }
    }

    Some(DecisionReport {
        strategy,
        best_move: fallback,
        reason: DecisionReason::PositionalFallback,
        nodes,
        cache_hits: 0,
        outcome: None,
        distance: None,
    })
}

const fn splitmix64(mut value: u64) -> u64 {
    value = value.wrapping_add(0x9e37_79b9_7f4a_7c15);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}
