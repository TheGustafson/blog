use crate::{PieceKind, Square};
use std::fmt;
use std::str::FromStr;

/// Rules-level move classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum MoveKind {
    Normal,
    CastleKingSide,
    CastleQueenSide,
    EnPassant,
    Promotion(PieceKind),
}

/// One fully classified chess move.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Move {
    from: Square,
    to: Square,
    kind: MoveKind,
}

impl Move {
    pub(crate) const PLACEHOLDER: Self = Self {
        from: Square::new(0),
        to: Square::new(0),
        kind: MoveKind::Normal,
    };

    pub const fn new(from: Square, to: Square, kind: MoveKind) -> Self {
        Self { from, to, kind }
    }

    pub const fn normal(from: Square, to: Square) -> Self {
        Self::new(from, to, MoveKind::Normal)
    }

    pub const fn from(self) -> Square {
        self.from
    }

    pub const fn to(self) -> Square {
        self.to
    }

    pub const fn kind(self) -> MoveKind {
        self.kind
    }

    pub const fn promotion(self) -> Option<PieceKind> {
        match self.kind {
            MoveKind::Promotion(kind) => Some(kind),
            MoveKind::Normal
            | MoveKind::CastleKingSide
            | MoveKind::CastleQueenSide
            | MoveKind::EnPassant => None,
        }
    }
}

impl fmt::Display for Move {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", self.from, self.to)?;
        if let Some(kind) = self.promotion() {
            write!(
                f,
                "{}",
                kind.promotion_char()
                    .expect("promotion move must contain a promotion piece")
            )?;
        }
        Ok(())
    }
}

impl FromStr for Move {
    type Err = &'static str;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if !value.is_ascii() || (value.len() != 4 && value.len() != 5) {
            return Err("move must use long algebraic notation such as e2e4 or a7a8q");
        }
        let from = value[0..2].parse()?;
        let to = value[2..4].parse()?;
        let kind = if value.len() == 5 {
            let promotion = match value.as_bytes()[4].to_ascii_lowercase() {
                b'n' => PieceKind::Knight,
                b'b' => PieceKind::Bishop,
                b'r' => PieceKind::Rook,
                b'q' => PieceKind::Queen,
                _ => return Err("promotion piece must be n, b, r, or q"),
            };
            MoveKind::Promotion(promotion)
        } else {
            MoveKind::Normal
        };
        Ok(Self::new(from, to, kind))
    }
}

/// Fixed-capacity legal or pseudo-legal move list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MoveList {
    moves: [Move; 256],
    len: u16,
}

impl Default for MoveList {
    fn default() -> Self {
        Self {
            moves: [Move::PLACEHOLDER; 256],
            len: 0,
        }
    }
}

impl MoveList {
    pub const fn len(self) -> usize {
        self.len as usize
    }

    pub const fn is_empty(self) -> bool {
        self.len == 0
    }

    pub fn as_slice(&self) -> &[Move] {
        &self.moves[..self.len()]
    }

    pub(crate) fn as_mut_slice(&mut self) -> &mut [Move] {
        let len = self.len();
        &mut self.moves[..len]
    }

    pub(crate) fn push(&mut self, mv: Move) {
        debug_assert!(self.len() < self.moves.len());
        self.moves[self.len()] = mv;
        self.len += 1;
    }
}

impl IntoIterator for MoveList {
    type Item = Move;
    type IntoIter = std::iter::Take<std::array::IntoIter<Move, 256>>;

    fn into_iter(self) -> Self::IntoIter {
        self.moves.into_iter().take(self.len())
    }
}
