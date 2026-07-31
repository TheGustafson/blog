use std::fmt;
use std::str::FromStr;

/// One of the nine squares, numbered rank-first from `a1` to `c3`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Square(u8);

impl Square {
    pub const COUNT: usize = 9;

    /// Creates a square from its rank-first zero-based index.
    ///
    /// # Panics
    ///
    /// Panics when `index` is greater than eight.
    pub const fn new(index: u8) -> Self {
        assert!(index < Self::COUNT as u8, "square index must be 0..9");
        Self(index)
    }

    pub const fn index(self) -> usize {
        self.0 as usize
    }

    pub const fn file(self) -> u8 {
        self.0 % 3
    }

    pub const fn rank(self) -> u8 {
        self.0 / 3
    }

    pub fn all() -> impl Iterator<Item = Self> {
        (0..Self::COUNT as u8).map(Self)
    }
}

impl fmt::Display for Square {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let file = char::from(b'a' + self.file());
        let rank = char::from(b'1' + self.rank());
        write!(f, "{file}{rank}")
    }
}

impl FromStr for Square {
    type Err = &'static str;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let bytes = value.as_bytes();
        if bytes.len() != 2
            || !(b'a'..=b'c').contains(&bytes[0])
            || !(b'1'..=b'3').contains(&bytes[1])
        {
            return Err("square must be a1..c3");
        }
        Ok(Self::new((bytes[1] - b'1') * 3 + (bytes[0] - b'a')))
    }
}

/// A tic-tac-toe move is exactly one square.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Move(Square);

impl Move {
    pub const fn new(square: Square) -> Self {
        Self(square)
    }

    pub const fn square(self) -> Square {
        self.0
    }
}

impl fmt::Display for Move {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl FromStr for Move {
    type Err = &'static str;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        value.parse().map(Self)
    }
}
