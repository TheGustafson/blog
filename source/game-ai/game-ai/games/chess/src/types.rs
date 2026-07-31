use std::fmt;
use std::str::FromStr;

/// One of the two chess sides.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(u8)]
pub enum Color {
    White,
    Black,
}

impl Color {
    pub const ALL: [Self; 2] = [Self::White, Self::Black];

    pub const fn other(self) -> Self {
        match self {
            Self::White => Self::Black,
            Self::Black => Self::White,
        }
    }

    pub const fn index(self) -> usize {
        self as usize
    }

    pub const fn pawn_step(self) -> i8 {
        match self {
            Self::White => 1,
            Self::Black => -1,
        }
    }

    pub const fn home_rank(self) -> u8 {
        match self {
            Self::White => 0,
            Self::Black => 7,
        }
    }

    pub const fn pawn_rank(self) -> u8 {
        match self {
            Self::White => 1,
            Self::Black => 6,
        }
    }

    pub const fn promotion_rank(self) -> u8 {
        match self {
            Self::White => 7,
            Self::Black => 0,
        }
    }
}

impl fmt::Display for Color {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::White => "white",
            Self::Black => "black",
        })
    }
}

/// The six orthodox chess piece kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(u8)]
pub enum PieceKind {
    Pawn,
    Knight,
    Bishop,
    Rook,
    Queen,
    King,
}

impl PieceKind {
    pub const ALL: [Self; 6] = [
        Self::Pawn,
        Self::Knight,
        Self::Bishop,
        Self::Rook,
        Self::Queen,
        Self::King,
    ];

    pub const PROMOTIONS: [Self; 4] = [Self::Queen, Self::Rook, Self::Bishop, Self::Knight];

    pub const fn index(self) -> usize {
        self as usize
    }

    pub const fn fen_char(self) -> char {
        match self {
            Self::Pawn => 'p',
            Self::Knight => 'n',
            Self::Bishop => 'b',
            Self::Rook => 'r',
            Self::Queen => 'q',
            Self::King => 'k',
        }
    }

    pub const fn promotion_char(self) -> Option<char> {
        match self {
            Self::Knight => Some('n'),
            Self::Bishop => Some('b'),
            Self::Rook => Some('r'),
            Self::Queen => Some('q'),
            Self::Pawn | Self::King => None,
        }
    }
}

/// A piece's color and kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Piece {
    pub color: Color,
    pub kind: PieceKind,
}

impl Piece {
    pub const fn new(color: Color, kind: PieceKind) -> Self {
        Self { color, kind }
    }

    pub const fn fen_char(self) -> char {
        let base = self.kind.fen_char();
        match self.color {
            Color::White => base.to_ascii_uppercase(),
            Color::Black => base,
        }
    }

    pub fn from_fen_char(value: char) -> Option<Self> {
        let color = if value.is_ascii_uppercase() {
            Color::White
        } else {
            Color::Black
        };
        let kind = match value.to_ascii_lowercase() {
            'p' => PieceKind::Pawn,
            'n' => PieceKind::Knight,
            'b' => PieceKind::Bishop,
            'r' => PieceKind::Rook,
            'q' => PieceKind::Queen,
            'k' => PieceKind::King,
            _ => return None,
        };
        Some(Self::new(color, kind))
    }
}

/// One chess square in little-endian rank-file order (`a1 == 0`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Square(u8);

impl Square {
    pub const COUNT: usize = 64;

    /// Creates a square from its little-endian rank-file index.
    ///
    /// # Panics
    ///
    /// Panics when `index` is greater than 63.
    pub const fn new(index: u8) -> Self {
        assert!(index < 64, "square index must be 0..64");
        Self(index)
    }

    /// Creates a square from zero-based file and rank coordinates.
    ///
    /// # Panics
    ///
    /// Panics when either coordinate is greater than seven.
    pub const fn from_file_rank(file: u8, rank: u8) -> Self {
        assert!(file < 8 && rank < 8, "file and rank must be 0..8");
        Self(rank * 8 + file)
    }

    pub const fn index(self) -> usize {
        self.0 as usize
    }

    pub const fn file(self) -> u8 {
        self.0 & 7
    }

    pub const fn rank(self) -> u8 {
        self.0 >> 3
    }

    pub fn offset(self, file_delta: i8, rank_delta: i8) -> Option<Self> {
        let file = self.file() as i8 + file_delta;
        let rank = self.rank() as i8 + rank_delta;
        if (0..8).contains(&file) && (0..8).contains(&rank) {
            Some(Self::from_file_rank(file as u8, rank as u8))
        } else {
            None
        }
    }

    pub fn all() -> impl Iterator<Item = Self> {
        (0..64).map(Self)
    }
}

impl fmt::Display for Square {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}{}",
            char::from(b'a' + self.file()),
            char::from(b'1' + self.rank())
        )
    }
}

impl FromStr for Square {
    type Err = &'static str;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let bytes = value.as_bytes();
        if bytes.len() != 2
            || !(b'a'..=b'h').contains(&bytes[0])
            || !(b'1'..=b'8').contains(&bytes[1])
        {
            return Err("square must be a1..h8");
        }
        Ok(Self::from_file_rank(bytes[0] - b'a', bytes[1] - b'1'))
    }
}
