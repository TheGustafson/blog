use crate::mv::{Move, Square};
use std::fmt;

const BOARD_MASK: u16 = 0x01ff;
const WIN_MASKS: [u16; 8] = [
    0b000_000_111,
    0b000_111_000,
    0b111_000_000,
    0b001_001_001,
    0b010_010_010,
    0b100_100_100,
    0b100_010_001,
    0b001_010_100,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Side {
    X,
    O,
}

impl Side {
    pub const fn other(self) -> Self {
        match self {
            Self::X => Self::O,
            Self::O => Self::X,
        }
    }

    pub const fn as_char(self) -> char {
        match self {
            Self::X => 'X',
            Self::O => 'O',
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
    Occupied(Square),
}

impl fmt::Display for MoveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::GameOver => write!(f, "the game is already over"),
            Self::Occupied(square) => write!(f, "square {square} is occupied"),
        }
    }
}

impl std::error::Error for MoveError {}

/// The whole game state: one nine-bit board per side and the side to move.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Position {
    x: u16,
    o: u16,
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
            x: 0,
            o: 0,
            side_to_move: Side::X,
        }
    }

    pub const fn side_to_move(self) -> Side {
        self.side_to_move
    }

    pub const fn bits(self, side: Side) -> u16 {
        match side {
            Side::X => self.x,
            Side::O => self.o,
        }
    }

    pub const fn occupied(self) -> u16 {
        self.x | self.o
    }

    pub fn side_at(self, square: Square) -> Option<Side> {
        let bit = 1u16 << square.index();
        if self.x & bit != 0 {
            Some(Side::X)
        } else if self.o & bit != 0 {
            Some(Side::O)
        } else {
            None
        }
    }

    pub fn is_winning_move(self, side: Side, mv: Move) -> bool {
        let bit = 1u16 << mv.square().index();
        self.occupied() & bit == 0 && Self::has_line(self.bits(side) | bit)
    }

    /// Iterates over legal moves in deterministic center-corner-edge order.
    pub fn legal_moves(self) -> impl Iterator<Item = Move> {
        const ORDER: [u8; 9] = [4, 0, 2, 6, 8, 1, 3, 5, 7];
        let empty = !self.occupied() & BOARD_MASK;
        ORDER
            .into_iter()
            .filter(move |index| empty & (1 << index) != 0)
            .map(|index| Move::new(Square::new(index)))
    }

    pub fn result(self) -> GameResult {
        if Self::has_line(self.x) {
            GameResult::Win(Side::X)
        } else if Self::has_line(self.o) {
            GameResult::Win(Side::O)
        } else if self.occupied() == BOARD_MASK {
            GameResult::Draw
        } else {
            GameResult::Ongoing
        }
    }

    pub fn winning_squares(self) -> Vec<Square> {
        let winning_bits = match self.result() {
            GameResult::Win(side) => self.bits(side),
            GameResult::Ongoing | GameResult::Draw => return Vec::new(),
        };
        let Some(mask) = WIN_MASKS
            .into_iter()
            .find(|mask| winning_bits & mask == *mask)
        else {
            return Vec::new();
        };
        Square::all()
            .filter(|square| mask & (1u16 << square.index()) != 0)
            .collect()
    }

    pub fn make_move(&mut self, mv: Move) -> Result<(), MoveError> {
        if self.result() != GameResult::Ongoing {
            return Err(MoveError::GameOver);
        }
        let bit = 1u16 << mv.square().index();
        if self.occupied() & bit != 0 {
            return Err(MoveError::Occupied(mv.square()));
        }
        match self.side_to_move {
            Side::X => self.x |= bit,
            Side::O => self.o |= bit,
        }
        self.side_to_move = self.side_to_move.other();
        debug_assert!(self.is_consistent());
        Ok(())
    }

    /// Undo the most recently made move.
    ///
    /// The caller supplies the move from its own search/history stack. There
    /// are no captures, so undo is one bit clear plus a side flip.
    pub fn unmake_move(&mut self, mv: Move) {
        self.side_to_move = self.side_to_move.other();
        let bit = !(1u16 << mv.square().index());
        match self.side_to_move {
            Side::X => self.x &= bit,
            Side::O => self.o &= bit,
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

    pub fn key(self) -> usize {
        let mut board = 0usize;
        let mut place = 1usize;
        for square in Square::all() {
            let digit = match self.side_at(square) {
                None => 0,
                Some(Side::X) => 1,
                Some(Side::O) => 2,
            };
            board += digit * place;
            place *= 3;
        }
        board + usize::from(self.side_to_move == Side::O) * Self::BOARD_STATES
    }

    pub fn canonical_key(self) -> usize {
        (0..8)
            .map(|transform| self.transformed(transform).key())
            .min()
            .expect("the symmetry group is non-empty")
    }

    pub const BOARD_STATES: usize = 19_683; // 3^9
    pub const KEY_SPACE: usize = Self::BOARD_STATES * 2;

    fn transformed(self, transform: u8) -> Self {
        let mut x = 0u16;
        let mut o = 0u16;
        for square in Square::all() {
            let target = transform_square(square, transform);
            let target_bit = 1u16 << target.index();
            match self.side_at(square) {
                Some(Side::X) => x |= target_bit,
                Some(Side::O) => o |= target_bit,
                None => {}
            }
        }
        Self {
            x,
            o,
            side_to_move: self.side_to_move,
        }
    }

    fn has_line(bits: u16) -> bool {
        for mask in WIN_MASKS {
            if bits & mask == mask {
                return true;
            }
        }
        false
    }

    fn is_consistent(self) -> bool {
        if self.x & self.o != 0 || self.occupied() & !BOARD_MASK != 0 {
            return false;
        }
        let x_count = self.x.count_ones();
        let o_count = self.o.count_ones();
        match self.side_to_move {
            Side::X => x_count == o_count,
            Side::O => x_count == o_count + 1,
        }
    }
}

fn transform_square(square: Square, transform: u8) -> Square {
    let (x, y) = (square.file(), square.rank());
    let (tx, ty) = match transform {
        0 => (x, y),
        1 => (2 - y, x),
        2 => (2 - x, 2 - y),
        3 => (y, 2 - x),
        4 => (2 - x, y),
        5 => (2 - y, 2 - x),
        6 => (x, 2 - y),
        7 => (y, x),
        _ => unreachable!("tic-tac-toe has exactly eight board symmetries"),
    };
    Square::new(ty * 3 + tx)
}
