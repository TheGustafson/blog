use crate::mv::{Cell, Column, Move, MoveList};
use std::fmt;

pub const WIDTH: usize = Column::COUNT;
pub const HEIGHT: usize = Cell::ROWS;
const STRIDE: usize = HEIGHT + 1;
const COLUMN_BITS: u64 = (1u64 << HEIGHT) - 1;

const fn board_mask() -> u64 {
    let mut mask = 0;
    let mut column = 0;
    while column < WIDTH {
        mask |= COLUMN_BITS << (column * STRIDE);
        column += 1;
    }
    mask
}

const fn bottom_mask() -> u64 {
    let mut mask = 0;
    let mut column = 0;
    while column < WIDTH {
        mask |= 1u64 << (column * STRIDE);
        column += 1;
    }
    mask
}

pub(crate) const BOARD_MASK: u64 = board_mask();
pub(crate) const BOTTOM_MASK: u64 = bottom_mask();

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Side {
    Red,
    Yellow,
}

impl Side {
    pub const fn other(self) -> Self {
        match self {
            Self::Red => Self::Yellow,
            Self::Yellow => Self::Red,
        }
    }

    pub const fn as_char(self) -> char {
        match self {
            Self::Red => 'R',
            Self::Yellow => 'Y',
        }
    }
}

impl fmt::Display for Side {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_char())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameResult {
    Ongoing,
    Draw,
    Win(Side),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoveError {
    GameOver,
    Full(Column),
}

impl fmt::Display for MoveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::GameOver => write!(f, "the game is already over"),
            Self::Full(column) => write!(f, "column {column} is full"),
        }
    }
}

impl std::error::Error for MoveError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Undo {
    mv: Move,
    side: Side,
    bit: u64,
}

impl Undo {
    pub const fn mv(self) -> Move {
        self.mv
    }

    pub const fn side(self) -> Side {
        self.side
    }

    pub const fn cell(self) -> Cell {
        let row = (self.bit.trailing_zeros() as usize % STRIDE) as u8;
        Cell::new(self.mv.column(), row)
    }
}

/// Two 42-bit player boards inside Connect Four's 7×7 sentinel layout.
///
/// Each column occupies seven adjacent bits: six playable cells and one zero
/// sentinel. The spare row makes horizontal and diagonal shifts unable to wrap
/// from one column into the next.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Position {
    red: u64,
    yellow: u64,
    side_to_move: Side,
}

impl Default for Position {
    fn default() -> Self {
        Self::start()
    }
}

impl Position {
    pub const fn start() -> Self {
        Self {
            red: 0,
            yellow: 0,
            side_to_move: Side::Red,
        }
    }

    pub const fn side_to_move(self) -> Side {
        self.side_to_move
    }

    pub const fn bits(self, side: Side) -> u64 {
        match side {
            Side::Red => self.red,
            Side::Yellow => self.yellow,
        }
    }

    pub const fn occupied(self) -> u64 {
        self.red | self.yellow
    }

    pub const fn ply(self) -> u8 {
        self.occupied().count_ones() as u8
    }

    /// Pascal Pons' compact, collision-free key for legal Connect Four boards.
    ///
    /// The side-to-move board is a subset of occupancy; adding the bottom mask
    /// encodes each column height in its sentinel bit.
    pub const fn key(self) -> u64 {
        self.bits(self.side_to_move) + self.occupied() + BOTTOM_MASK
    }

    pub fn side_at(self, cell: Cell) -> Option<Side> {
        let bit = 1u64 << cell.bit_index();
        if self.red & bit != 0 {
            Some(Side::Red)
        } else if self.yellow & bit != 0 {
            Some(Side::Yellow)
        } else {
            None
        }
    }

    /// One set bit per non-full column, computed with one addition.
    pub const fn playable_bits(self) -> u64 {
        self.occupied().wrapping_add(BOTTOM_MASK) & BOARD_MASK
    }

    pub const fn can_play(self, column: Column) -> bool {
        self.playable_bits() & Self::column_mask(column) != 0
    }

    /// Returns legal moves in deterministic center-first order.
    pub fn legal_moves(self) -> MoveList {
        const ORDER: [u8; WIDTH] = [3, 2, 4, 1, 5, 0, 6];
        let mut moves = MoveList::default();
        if self.result() != GameResult::Ongoing {
            return moves;
        }
        for index in ORDER {
            let column = Column::new(index);
            if self.can_play(column) {
                moves.push(Move::new(column));
            }
        }
        moves
    }

    pub fn is_winning_move(self, side: Side, mv: Move) -> bool {
        let bit = self.playable_bits() & Self::column_mask(mv.column());
        bit != 0 && Self::has_four(self.bits(side) | bit)
    }

    pub fn result(self) -> GameResult {
        if Self::has_four(self.red) {
            GameResult::Win(Side::Red)
        } else if Self::has_four(self.yellow) {
            GameResult::Win(Side::Yellow)
        } else if self.occupied() == BOARD_MASK {
            GameResult::Draw
        } else {
            GameResult::Ongoing
        }
    }

    pub fn winning_cells(self) -> Vec<Cell> {
        let winning_bits = match self.result() {
            GameResult::Win(side) => self.bits(side),
            GameResult::Ongoing | GameResult::Draw => return Vec::new(),
        };
        let directions = [(1i8, 0i8), (0, 1), (1, 1), (1, -1)];
        let mut cells = Vec::with_capacity(7);
        for column in 0..WIDTH as i8 {
            for row in 0..HEIGHT as i8 {
                for (dc, dr) in directions {
                    let end_column = column + dc * 3;
                    let end_row = row + dr * 3;
                    if !(0..WIDTH as i8).contains(&end_column)
                        || !(0..HEIGHT as i8).contains(&end_row)
                    {
                        continue;
                    }
                    let line = (0..4).map(|step| {
                        Cell::new(
                            Column::new((column + dc * step) as u8),
                            (row + dr * step) as u8,
                        )
                    });
                    let line: Vec<Cell> = line.collect();
                    if line
                        .iter()
                        .all(|cell| winning_bits & (1u64 << cell.bit_index()) != 0)
                    {
                        for cell in line {
                            if !cells.contains(&cell) {
                                cells.push(cell);
                            }
                        }
                    }
                }
            }
        }
        cells.sort_unstable();
        cells
    }

    pub fn make_move(&mut self, mv: Move) -> Result<Undo, MoveError> {
        if self.result() != GameResult::Ongoing {
            return Err(MoveError::GameOver);
        }
        let bit = self.playable_bits() & Self::column_mask(mv.column());
        if bit == 0 {
            return Err(MoveError::Full(mv.column()));
        }
        let side = self.side_to_move;
        match side {
            Side::Red => self.red |= bit,
            Side::Yellow => self.yellow |= bit,
        }
        self.side_to_move = side.other();
        debug_assert!(self.is_consistent());
        Ok(Undo { mv, side, bit })
    }

    pub fn unmake_move(&mut self, undo: Undo) {
        debug_assert_eq!(self.side_to_move, undo.side.other());
        debug_assert_ne!(self.bits(undo.side) & undo.bit, 0);
        self.side_to_move = undo.side;
        match undo.side {
            Side::Red => self.red &= !undo.bit,
            Side::Yellow => self.yellow &= !undo.bit,
        }
        debug_assert!(self.is_consistent());
    }

    pub fn from_moves(moves: &[Move]) -> Result<Self, MoveError> {
        let mut position = Self::start();
        for &mv in moves {
            position.make_move(mv)?;
        }
        Ok(position)
    }

    pub fn mirrored(self) -> Self {
        let mut mirrored = Self::start();
        mirrored.side_to_move = self.side_to_move;
        for column in Column::all() {
            let shift = column.index() * STRIDE;
            let target_shift = column.mirrored().index() * STRIDE;
            let red = (self.red >> shift) & COLUMN_BITS;
            let yellow = (self.yellow >> shift) & COLUMN_BITS;
            mirrored.red |= red << target_shift;
            mirrored.yellow |= yellow << target_shift;
        }
        mirrored
    }

    pub const fn has_four(bits: u64) -> bool {
        let vertical = bits & (bits >> 1);
        if vertical & (vertical >> 2) != 0 {
            return true;
        }
        let horizontal = bits & (bits >> STRIDE);
        if horizontal & (horizontal >> (2 * STRIDE)) != 0 {
            return true;
        }
        let rising = bits & (bits >> (STRIDE + 1));
        if rising & (rising >> (2 * (STRIDE + 1))) != 0 {
            return true;
        }
        let falling = bits & (bits >> (STRIDE - 1));
        falling & (falling >> (2 * (STRIDE - 1))) != 0
    }

    const fn column_mask(column: Column) -> u64 {
        COLUMN_BITS << (column.index() * STRIDE)
    }

    fn is_consistent(self) -> bool {
        if self.red & self.yellow != 0 || self.occupied() & !BOARD_MASK != 0 {
            return false;
        }
        let red_count = self.red.count_ones();
        let yellow_count = self.yellow.count_ones();
        let turn_is_consistent = match self.side_to_move {
            Side::Red => red_count == yellow_count,
            Side::Yellow => red_count == yellow_count + 1,
        };
        if !turn_is_consistent {
            return false;
        }
        for column in Column::all() {
            let occupied = (self.occupied() >> (column.index() * STRIDE)) & COLUMN_BITS;
            if occupied != (1u64 << occupied.count_ones()) - 1 {
                return false;
            }
        }
        !(Self::has_four(self.red) && Self::has_four(self.yellow))
    }
}
