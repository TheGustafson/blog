use std::fmt;
use std::str::FromStr;

/// One square, with `a1` as the least-significant bit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Square(u8);

impl Square {
    pub const COUNT: usize = 64;

    /// Creates a square from its rank-first zero-based index.
    ///
    /// # Panics
    ///
    /// Panics when `index` is greater than 63.
    pub const fn new(index: u8) -> Self {
        assert!(index < 64, "square index must be 0..64");
        Self(index)
    }

    pub const fn index(self) -> usize {
        self.0 as usize
    }

    pub const fn file(self) -> u8 {
        self.0 % 8
    }

    pub const fn rank(self) -> u8 {
        self.0 / 8
    }

    pub const fn mirrored(self) -> Self {
        Self::new(self.rank() * 8 + (7 - self.file()))
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
        Ok(Self::new((bytes[1] - b'1') * 8 + (bytes[0] - b'a')))
    }
}

/// Othello pass is a real move because it changes the side to move.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Move {
    Place(Square),
    Pass,
}

impl Move {
    pub const fn square(self) -> Option<Square> {
        match self {
            Self::Place(square) => Some(square),
            Self::Pass => None,
        }
    }

    pub const fn mirrored(self) -> Self {
        match self {
            Self::Place(square) => Self::Place(square.mirrored()),
            Self::Pass => Self::Pass,
        }
    }
}

impl fmt::Display for Move {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Place(square) => square.fmt(f),
            Self::Pass => f.write_str("pass"),
        }
    }
}

impl FromStr for Move {
    type Err = &'static str;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if value.eq_ignore_ascii_case("pass") {
            Ok(Self::Pass)
        } else {
            value.parse().map(Self::Place)
        }
    }
}

/// Fixed-capacity list large enough for every Othello move.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MoveList {
    moves: [Move; Square::COUNT],
    len: u8,
}

impl Default for MoveList {
    fn default() -> Self {
        Self {
            moves: [Move::Pass; Square::COUNT],
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

    pub(crate) fn push(&mut self, mv: Move) {
        debug_assert!(self.len() < self.moves.len());
        self.moves[self.len()] = mv;
        self.len += 1;
    }
}

impl IntoIterator for MoveList {
    type Item = Move;
    type IntoIter = std::iter::Take<std::array::IntoIter<Move, { Square::COUNT }>>;

    fn into_iter(self) -> Self::IntoIter {
        self.moves.into_iter().take(self.len())
    }
}
