use std::fmt;
use std::str::FromStr;

/// One of Connect Four's seven columns, indexed left to right.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Column(u8);

impl Column {
    pub const COUNT: usize = 7;

    /// Creates a column from its zero-based left-to-right index.
    ///
    /// # Panics
    ///
    /// Panics when `index` is greater than six.
    pub const fn new(index: u8) -> Self {
        assert!(index < Self::COUNT as u8, "column index must be 0..7");
        Self(index)
    }

    pub const fn index(self) -> usize {
        self.0 as usize
    }

    pub const fn mirrored(self) -> Self {
        Self::new(Self::COUNT as u8 - 1 - self.0)
    }

    pub fn all() -> impl Iterator<Item = Self> {
        (0..Self::COUNT as u8).map(Self)
    }
}

impl fmt::Display for Column {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", char::from(b'a' + self.0))
    }
}

impl FromStr for Column {
    type Err = &'static str;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let bytes = value.as_bytes();
        if bytes.len() != 1 {
            return Err("column must be a..g or 1..7");
        }
        match bytes[0] {
            b'a'..=b'g' => Ok(Self::new(bytes[0] - b'a')),
            b'A'..=b'G' => Ok(Self::new(bytes[0] - b'A')),
            b'1'..=b'7' => Ok(Self::new(bytes[0] - b'1')),
            _ => Err("column must be a..g or 1..7"),
        }
    }
}

/// A move drops one disc into a column.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Move(Column);

impl Move {
    pub const fn new(column: Column) -> Self {
        Self(column)
    }

    pub const fn column(self) -> Column {
        self.0
    }

    pub const fn mirrored(self) -> Self {
        Self(self.0.mirrored())
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

/// A physical board cell. Rank zero is the bottom row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Cell {
    column: Column,
    row: u8,
}

impl Cell {
    pub const ROWS: usize = 6;

    /// Creates a physical cell.
    ///
    /// # Panics
    ///
    /// Panics when `row` is greater than five.
    pub const fn new(column: Column, row: u8) -> Self {
        assert!(row < Self::ROWS as u8, "row index must be 0..6");
        Self { column, row }
    }

    pub const fn column(self) -> Column {
        self.column
    }

    pub const fn row(self) -> usize {
        self.row as usize
    }

    pub const fn bit_index(self) -> usize {
        self.column.index() * 7 + self.row()
    }

    pub const fn mirrored(self) -> Self {
        Self::new(self.column.mirrored(), self.row)
    }
}

impl fmt::Display for Cell {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", self.column, self.row + 1)
    }
}

/// At most seven moves exist, so search never needs a heap-backed move list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MoveList {
    moves: [Move; Column::COUNT],
    len: u8,
}

impl Default for MoveList {
    fn default() -> Self {
        Self {
            moves: [Move::new(Column::new(0)); Column::COUNT],
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
        debug_assert!(self.len() < Column::COUNT);
        self.moves[self.len()] = mv;
        self.len += 1;
    }
}

impl IntoIterator for MoveList {
    type Item = Move;
    type IntoIter = std::iter::Take<std::array::IntoIter<Move, { Column::COUNT }>>;

    fn into_iter(self) -> Self::IntoIter {
        self.moves.into_iter().take(self.len())
    }
}
