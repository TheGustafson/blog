use std::fmt;
use std::str::FromStr;

pub const MIN_SIZE: u8 = 9;
pub const MAX_SIZE: u8 = 24;
pub const STRIDE: u16 = MAX_SIZE as u16;
pub const MAX_CELLS: usize = MAX_SIZE as usize * MAX_SIZE as usize;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct BoardSize(u8);

impl BoardSize {
    pub fn new(size: u8) -> Result<Self, BoardSizeError> {
        if (MIN_SIZE..=MAX_SIZE).contains(&size) {
            Ok(Self(size))
        } else {
            Err(BoardSizeError(size))
        }
    }

    pub const fn get(self) -> u8 {
        self.0
    }

    #[cfg(test)]
    pub(crate) const fn for_test(size: u8) -> Self {
        assert!(size > 0 && size <= MAX_SIZE);
        Self(size)
    }

    pub const fn cell_count(self) -> u16 {
        self.0 as u16 * self.0 as u16
    }

    pub(crate) const fn contains(self, cell: Cell) -> bool {
        cell.file() < self.0 && cell.rank() < self.0
    }
}

impl Default for BoardSize {
    fn default() -> Self {
        Self(13)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BoardSizeError(u8);

impl fmt::Display for BoardSizeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "board size must be from {MIN_SIZE} through {MAX_SIZE}"
        )
    }
}

impl std::error::Error for BoardSizeError {}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Cell(u16);

impl Cell {
    pub const fn new(file: u8, rank: u8) -> Option<Self> {
        if file < MAX_SIZE && rank < MAX_SIZE {
            Some(Self::from_coords(file, rank))
        } else {
            None
        }
    }

    pub(crate) const fn from_coords(file: u8, rank: u8) -> Self {
        Self(rank as u16 * STRIDE + file as u16)
    }

    pub(crate) const fn from_index(index: u16) -> Self {
        Self(index)
    }

    pub(crate) fn from_dense(index: u16, size: BoardSize) -> Self {
        let width = u16::from(size.get());
        Self::from_coords((index % width) as u8, (index / width) as u8)
    }

    pub const fn file(self) -> u8 {
        (self.0 % STRIDE) as u8
    }

    pub const fn rank(self) -> u8 {
        (self.0 / STRIDE) as u8
    }

    pub const fn index(self) -> u16 {
        self.0
    }
}

impl fmt::Display for Cell {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let file = char::from(b'a' + self.file());
        write!(formatter, "{file}{}", self.rank() + 1)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Move {
    Place(Cell),
    Swap,
}

impl Move {
    pub const fn place(file: u8, rank: u8) -> Option<Self> {
        match Cell::new(file, rank) {
            Some(cell) => Some(Self::Place(cell)),
            None => None,
        }
    }

    pub const fn cell(self) -> Option<Cell> {
        match self {
            Self::Place(cell) => Some(cell),
            Self::Swap => None,
        }
    }

    pub(crate) const fn order_key(self) -> u16 {
        match self {
            Self::Place(cell) => cell.index(),
            Self::Swap => u16::MAX,
        }
    }
}

impl fmt::Display for Move {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Place(cell) => cell.fmt(formatter),
            Self::Swap => formatter.write_str("swap"),
        }
    }
}

impl FromStr for Move {
    type Err = ParseMoveError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if value.eq_ignore_ascii_case("swap") {
            return Ok(Self::Swap);
        }
        let bytes = value.as_bytes();
        if !(2..=3).contains(&bytes.len()) || !bytes[0].is_ascii_alphabetic() {
            return Err(ParseMoveError);
        }
        let file = bytes[0].to_ascii_lowercase().wrapping_sub(b'a');
        if file >= MAX_SIZE {
            return Err(ParseMoveError);
        }
        let rank = value[1..].parse::<u8>().map_err(|_| ParseMoveError)?;
        if !(1..=MAX_SIZE).contains(&rank) {
            return Err(ParseMoveError);
        }
        Ok(Self::Place(Cell::from_coords(file, rank - 1)))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ParseMoveError;

impl fmt::Display for ParseMoveError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("expected a cell from a1 through x24, or swap")
    }
}

impl std::error::Error for ParseMoveError {}
