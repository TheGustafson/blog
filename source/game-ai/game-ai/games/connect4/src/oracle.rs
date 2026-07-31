use crate::{Column, Move, Position};

/// Exact outcome under perfect play, imported as a reference fact rather than
/// computed by this educational live search.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OracleOutcome {
    Win,
    Draw,
    Loss,
}

impl OracleOutcome {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Win => "win",
            Self::Draw => "draw",
            Self::Loss => "loss",
        }
    }
}

/// One auditable perfect-play regression case from Pascal Pons's public
/// Connect Four solver tutorial.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OracleCase {
    pub notation: &'static str,
    pub pons_score: i8,
    pub outcome: OracleOutcome,
    pub description: &'static str,
}

/// This is intentionally not an opening book. It is a tiny set of published
/// exact facts used to prove the boundary between oracle data and live search.
pub const ORACLE_CASES: [OracleCase; 2] = [
    OracleCase {
        notation: "4455",
        pons_score: 18,
        outcome: OracleOutcome::Win,
        description: "The side to move can force a win with its fourth disc",
    },
    OracleCase {
        notation: "44455554221",
        pons_score: -15,
        outcome: OracleOutcome::Loss,
        description: "The side to move loses and the opponent wins next turn",
    },
];

pub fn probe_oracle(position: Position) -> Option<&'static OracleCase> {
    ORACLE_CASES
        .iter()
        .find(|case| case_position(case.notation) == position)
}

fn case_position(notation: &str) -> Position {
    let moves: Vec<_> = notation
        .bytes()
        .map(|digit| Move::new(Column::new(digit - b'1')))
        .collect();
    Position::from_moves(&moves).expect("embedded oracle cases must be legal")
}
