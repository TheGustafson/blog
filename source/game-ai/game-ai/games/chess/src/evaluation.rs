use crate::psqt_tuned::{
    ENDGAME_PIECE_SQUARE, ENDGAME_VALUE, MIDDLEGAME_PIECE_SQUARE, MIDDLEGAME_VALUE,
};
use crate::{Color, PieceKind, Position, Square, builtin_nnue};
use std::fmt;
use std::str::FromStr;

const MAX_PHASE: i32 = 24;
const PHASE_VALUE: [u8; 6] = [0, 1, 1, 2, 4, 0];

/// Controlled evaluator configurations used by tests and opponent settings.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum EvaluationProfile {
    Material,
    #[default]
    PieceSquare,
    TinyNnue,
}

impl EvaluationProfile {
    pub const ALL: [Self; 3] = [Self::Material, Self::PieceSquare, Self::TinyNnue];

    pub const fn name(self) -> &'static str {
        match self {
            Self::Material => "material",
            Self::PieceSquare => "piece-square",
            Self::TinyNnue => "tiny-nnue",
        }
    }
}

impl fmt::Display for EvaluationProfile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

impl FromStr for EvaluationProfile {
    type Err = &'static str;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.to_ascii_lowercase().as_str() {
            "material" => Ok(Self::Material),
            "piece-square" | "piece_square" | "piecesquare" | "psqt" | "full" => {
                Ok(Self::PieceSquare)
            }
            "tiny-nnue" | "tiny_nnue" | "tinynnue" | "nnue" => Ok(Self::TinyNnue),
            _ => Err("evaluator must be material, piece-square, or tiny-nnue"),
        }
    }
}

/// A fully traceable score, always from the side-to-move's perspective.
///
/// `phase` is remaining non-pawn material: 24 in the initial position and
/// zero in a bare-king ending. Each contribution is independently tapered,
/// so `total == terms_sum()` is an intentionally simple invariant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Evaluation {
    pub profile: EvaluationProfile,
    pub side_to_move: Color,
    pub phase: u8,
    pub middlegame_material: i32,
    pub endgame_material: i32,
    pub middlegame_piece_square: i32,
    pub endgame_piece_square: i32,
    pub material: i32,
    pub piece_square: i32,
    pub nnue: i32,
    pub total: i32,
}

impl Evaluation {
    pub const fn terms_sum(self) -> i32 {
        self.material + self.piece_square + self.nnue
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PieceContribution {
    pub square: Square,
    pub color: Color,
    pub kind: PieceKind,
    pub middlegame_material: i32,
    pub endgame_material: i32,
    pub middlegame_piece_square: i32,
    pub endgame_piece_square: i32,
    pub material: i32,
    pub piece_square: i32,
    pub total: i32,
}

/// Evaluates `position` from its side-to-move perspective.
pub fn evaluate(position: &Position, profile: EvaluationProfile) -> Evaluation {
    let us = position.side_to_move();
    let mut phase = 0u8;
    let mut middlegame_material = 0;
    let mut endgame_material = 0;
    let mut middlegame_piece_square = 0;
    let mut endgame_piece_square = 0;

    for color in Color::ALL {
        let sign = if color == us { 1 } else { -1 };
        for kind in PieceKind::ALL {
            let mut pieces = position.pieces(color, kind);
            phase = phase.saturating_add(
                PHASE_VALUE[kind.index()].saturating_mul(pieces.count_ones() as u8),
            );
            while pieces != 0 {
                let square = Square::new(pieces.trailing_zeros() as u8);
                pieces &= pieces - 1;
                middlegame_material += sign * MIDDLEGAME_VALUE[kind.index()];
                endgame_material += sign * ENDGAME_VALUE[kind.index()];
                middlegame_piece_square += sign * piece_square(kind, color, square, false);
                endgame_piece_square += sign * piece_square(kind, color, square, true);
            }
        }
    }

    phase = phase.min(MAX_PHASE as u8);
    let (material, piece_square, nnue) = match profile {
        EvaluationProfile::Material => (taper(middlegame_material, endgame_material, phase), 0, 0),
        EvaluationProfile::PieceSquare => (
            taper(middlegame_material, endgame_material, phase),
            taper(middlegame_piece_square, endgame_piece_square, phase),
            0,
        ),
        EvaluationProfile::TinyNnue => (0, 0, builtin_nnue().evaluate_refresh(position)),
    };
    let mut evaluation = Evaluation {
        profile,
        side_to_move: us,
        phase,
        middlegame_material,
        endgame_material,
        middlegame_piece_square,
        endgame_piece_square,
        material,
        piece_square,
        nnue,
        total: 0,
    };
    evaluation.total = evaluation.terms_sum();
    evaluation
}

/// Returns one traceable classical contribution per occupied square.
///
/// Neural-network evaluation has no additive per-piece decomposition, so the
/// returned classical terms are zero for [`EvaluationProfile::TinyNnue`].
pub fn piece_contributions(
    position: &Position,
    profile: EvaluationProfile,
) -> Vec<PieceContribution> {
    let phase = phase(position);
    let us = position.side_to_move();
    let mut contributions = Vec::with_capacity(position.occupied().count_ones() as usize);
    for square in Square::all() {
        let Some(piece) = position.piece_at(square) else {
            continue;
        };
        let sign = if piece.color == us { 1 } else { -1 };
        let middlegame_material = sign * MIDDLEGAME_VALUE[piece.kind.index()];
        let endgame_material = sign * ENDGAME_VALUE[piece.kind.index()];
        let middlegame_piece_square = sign * piece_square(piece.kind, piece.color, square, false);
        let endgame_piece_square = sign * piece_square(piece.kind, piece.color, square, true);
        let (material, piece_square) = match profile {
            EvaluationProfile::Material => (taper(middlegame_material, endgame_material, phase), 0),
            EvaluationProfile::PieceSquare => (
                taper(middlegame_material, endgame_material, phase),
                taper(middlegame_piece_square, endgame_piece_square, phase),
            ),
            EvaluationProfile::TinyNnue => (0, 0),
        };
        contributions.push(PieceContribution {
            square,
            color: piece.color,
            kind: piece.kind,
            middlegame_material,
            endgame_material,
            middlegame_piece_square,
            endgame_piece_square,
            material,
            piece_square,
            total: material + piece_square,
        });
    }
    contributions
}

fn phase(position: &Position) -> u8 {
    let mut phase = 0u8;
    for color in Color::ALL {
        for kind in PieceKind::ALL {
            phase = phase.saturating_add(
                PHASE_VALUE[kind.index()]
                    .saturating_mul(position.pieces(color, kind).count_ones() as u8),
            );
        }
    }
    phase.min(MAX_PHASE as u8)
}

const fn taper(middlegame: i32, endgame: i32, phase: u8) -> i32 {
    let phase = phase as i32;
    (middlegame * phase + endgame * (MAX_PHASE - phase)) / MAX_PHASE
}

fn piece_square(kind: PieceKind, color: Color, square: Square, endgame: bool) -> i32 {
    let relative_square = match color {
        Color::White => square.index(),
        Color::Black => square.index() ^ 56,
    };
    let table = if endgame {
        &ENDGAME_PIECE_SQUARE
    } else {
        &MIDDLEGAME_PIECE_SQUARE
    };
    i32::from(table[kind.index()][relative_square])
}

#[doc(hidden)]
pub fn classical_piece_value(kind: PieceKind, square: Square, endgame: bool) -> i32 {
    let material = if endgame {
        ENDGAME_VALUE[kind.index()]
    } else {
        MIDDLEGAME_VALUE[kind.index()]
    };
    material + piece_square(kind, Color::White, square, endgame)
}
