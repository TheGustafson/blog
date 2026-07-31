use crate::{Move, MoveList, Square};
use std::fmt;

const FILE_A: u64 = 0x0101_0101_0101_0101;
const FILE_H: u64 = 0x8080_8080_8080_8080;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Side {
    Black,
    White,
}

impl Side {
    pub const fn other(self) -> Self {
        match self {
            Self::Black => Self::White,
            Self::White => Self::Black,
        }
    }

    pub const fn as_char(self) -> char {
        match self {
            Self::Black => 'B',
            Self::White => 'W',
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
    Draw { black: u8, white: u8 },
    Win { winner: Side, black: u8, white: u8 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoveError {
    GameOver,
    IllegalSquare(Square),
    PassNotAllowed,
    MustPass,
}

impl fmt::Display for MoveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::GameOver => write!(f, "the game is already over"),
            Self::IllegalSquare(square) => {
                write!(f, "placing at {square} captures no opponent discs")
            }
            Self::PassNotAllowed => write!(f, "pass is only legal when no placement is legal"),
            Self::MustPass => write!(f, "the side to move has no placement and must pass"),
        }
    }
}

impl std::error::Error for MoveError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Undo {
    mv: Move,
    side: Side,
    flipped: u64,
}

impl Undo {
    pub const fn mv(self) -> Move {
        self.mv
    }

    pub const fn side(self) -> Side {
        self.side
    }

    pub const fn flipped(self) -> u64 {
        self.flipped
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Direction {
    North,
    South,
    East,
    West,
    NorthEast,
    NorthWest,
    SouthEast,
    SouthWest,
}

const DIRECTIONS: [Direction; 8] = [
    Direction::North,
    Direction::South,
    Direction::East,
    Direction::West,
    Direction::NorthEast,
    Direction::NorthWest,
    Direction::SouthEast,
    Direction::SouthWest,
];

/// Two disjoint bitboards and the side whose transition comes next.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Position {
    black: u64,
    white: u64,
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
            black: (1u64 << 28) | (1u64 << 35),
            white: (1u64 << 27) | (1u64 << 36),
            side_to_move: Side::Black,
        }
    }

    pub fn from_bits(black: u64, white: u64, side_to_move: Side) -> Result<Self, &'static str> {
        if black & white != 0 {
            return Err("black and white bitboards overlap");
        }
        Ok(Self {
            black,
            white,
            side_to_move,
        })
    }

    /// Parse the 64-cell board plus side token used by Cassio's engine
    /// protocol. `X`, `B`, or `*` denotes black; `O` or `W` denotes white.
    pub fn from_cassio(board: &str, side: &str) -> Result<Self, &'static str> {
        if !board.is_ascii() || board.len() != 64 {
            return Err("Cassio board must contain exactly 64 ASCII cells");
        }
        let mut black = 0u64;
        let mut white = 0u64;
        for (index, cell) in board.bytes().enumerate() {
            let bit = 1u64 << index;
            match cell.to_ascii_lowercase() {
                b'x' | b'b' | b'*' => black |= bit,
                b'o' | b'w' => white |= bit,
                b'-' | b'.' => {}
                _ => return Err("Cassio board cells must be X, O, -, or ."),
            }
        }
        let side_to_move = match side.as_bytes() {
            [b'X' | b'x' | b'B' | b'b' | b'*'] => Side::Black,
            [b'O' | b'o' | b'W' | b'w'] => Side::White,
            _ => return Err("Cassio side must be X, O, B, or W"),
        };
        Self::from_bits(black, white, side_to_move)
    }

    /// Serialize a Cassio position as 64 cells followed immediately by `X`
    /// or `O` for the side to move.
    pub fn to_cassio(self) -> String {
        let mut value = String::with_capacity(65);
        for index in 0..64u8 {
            value.push(match self.side_at(Square::new(index)) {
                Some(Side::Black) => 'X',
                Some(Side::White) => 'O',
                None => '-',
            });
        }
        value.push(match self.side_to_move {
            Side::Black => 'X',
            Side::White => 'O',
        });
        value
    }

    pub const fn bits(self, side: Side) -> u64 {
        match side {
            Side::Black => self.black,
            Side::White => self.white,
        }
    }

    pub const fn occupied(self) -> u64 {
        self.black | self.white
    }

    pub const fn side_to_move(self) -> Side {
        self.side_to_move
    }

    pub const fn empty_count(self) -> u8 {
        (!self.occupied()).count_ones() as u8
    }

    pub const fn occupied_count(self) -> u8 {
        self.occupied().count_ones() as u8
    }

    pub const fn disc_count(self, side: Side) -> u8 {
        self.bits(side).count_ones() as u8
    }

    pub fn side_at(self, square: Square) -> Option<Side> {
        let bit = 1u64 << square.index();
        if self.black & bit != 0 {
            Some(Side::Black)
        } else if self.white & bit != 0 {
            Some(Side::White)
        } else {
            None
        }
    }

    pub fn legal_placement_bits(self) -> u64 {
        legal_bits(
            self.bits(self.side_to_move),
            self.bits(self.side_to_move.other()),
        )
    }

    pub fn legal_placements_for(self, side: Side) -> u64 {
        legal_bits(self.bits(side), self.bits(side.other()))
    }

    pub fn frontier_bits(self, side: Side) -> u64 {
        self.bits(side) & adjacent(!self.occupied())
    }

    pub fn potential_mobility_bits(self, side: Side) -> u64 {
        !self.occupied() & adjacent(self.bits(side.other()))
    }

    /// Returns every legal placement, or a single pass when one is required.
    pub fn legal_moves(self) -> MoveList {
        let mut moves = MoveList::default();
        let mut bits = self.legal_placement_bits();
        while bits != 0 {
            let index = bits.trailing_zeros() as u8;
            moves.push(Move::Place(Square::new(index)));
            bits &= bits - 1;
        }
        if moves.is_empty() && self.legal_placements_for(self.side_to_move.other()) != 0 {
            moves.push(Move::Pass);
        }
        moves
    }

    pub fn flips_for(self, square: Square) -> u64 {
        let move_bit = 1u64 << square.index();
        if self.occupied() & move_bit != 0 {
            return 0;
        }
        let own = self.bits(self.side_to_move);
        let opponent = self.bits(self.side_to_move.other());
        let mut flips = 0;
        for direction in DIRECTIONS {
            let mut captured = 0;
            let mut ray = shift(move_bit, direction) & opponent;
            while ray != 0 {
                captured |= ray;
                let next = shift(ray, direction);
                if next & own != 0 {
                    flips |= captured;
                    break;
                }
                ray = next & opponent;
            }
        }
        flips
    }

    pub fn result(self) -> GameResult {
        if self.legal_placement_bits() != 0
            || self.legal_placements_for(self.side_to_move.other()) != 0
        {
            return GameResult::Ongoing;
        }
        let black = self.disc_count(Side::Black);
        let white = self.disc_count(Side::White);
        match black.cmp(&white) {
            std::cmp::Ordering::Greater => GameResult::Win {
                winner: Side::Black,
                black,
                white,
            },
            std::cmp::Ordering::Less => GameResult::Win {
                winner: Side::White,
                black,
                white,
            },
            std::cmp::Ordering::Equal => GameResult::Draw { black, white },
        }
    }

    /// Plays one placement or forced pass and returns exact undo data.
    pub fn make_move(&mut self, mv: Move) -> Result<Undo, MoveError> {
        if self.result() != GameResult::Ongoing {
            return Err(MoveError::GameOver);
        }
        let side = self.side_to_move;
        let flips = match mv {
            Move::Pass => {
                if self.legal_placement_bits() != 0 {
                    return Err(MoveError::PassNotAllowed);
                }
                if self.legal_placements_for(side.other()) == 0 {
                    return Err(MoveError::GameOver);
                }
                0
            }
            Move::Place(square) => {
                if self.legal_placement_bits() == 0 {
                    return Err(MoveError::MustPass);
                }
                let flips = self.flips_for(square);
                if flips == 0 {
                    return Err(MoveError::IllegalSquare(square));
                }
                let placed = 1u64 << square.index();
                match side {
                    Side::Black => {
                        self.black |= placed | flips;
                        self.white &= !flips;
                    }
                    Side::White => {
                        self.white |= placed | flips;
                        self.black &= !flips;
                    }
                }
                flips
            }
        };
        self.side_to_move = side.other();
        debug_assert!(self.is_consistent());
        Ok(Undo {
            mv,
            side,
            flipped: flips,
        })
    }

    /// Restores the position that existed before `undo` was produced.
    pub fn unmake_move(&mut self, undo: Undo) {
        debug_assert_eq!(self.side_to_move, undo.side.other());
        self.side_to_move = undo.side;
        if let Move::Place(square) = undo.mv {
            let placed = 1u64 << square.index();
            match undo.side {
                Side::Black => {
                    self.black &= !(placed | undo.flipped);
                    self.white |= undo.flipped;
                }
                Side::White => {
                    self.white &= !(placed | undo.flipped);
                    self.black |= undo.flipped;
                }
            }
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
        let mut black = 0;
        let mut white = 0;
        for square in Square::all() {
            let target = square.mirrored();
            let target_bit = 1u64 << target.index();
            match self.side_at(square) {
                Some(Side::Black) => black |= target_bit,
                Some(Side::White) => white |= target_bit,
                None => {}
            }
        }
        Self {
            black,
            white,
            side_to_move: self.side_to_move,
        }
    }

    fn is_consistent(self) -> bool {
        self.black & self.white == 0
    }
}

fn legal_bits(own: u64, opponent: u64) -> u64 {
    let empty = !(own | opponent);
    let mut legal = 0;
    for direction in DIRECTIONS {
        let mut run = shift(own, direction) & opponent;
        // At most six opponent discs can lie between the anchor and target.
        for _ in 0..5 {
            run |= shift(run, direction) & opponent;
        }
        legal |= shift(run, direction) & empty;
    }
    legal
}

fn adjacent(bits: u64) -> u64 {
    DIRECTIONS
        .into_iter()
        .fold(0, |neighbors, direction| neighbors | shift(bits, direction))
}

const fn shift(bits: u64, direction: Direction) -> u64 {
    match direction {
        Direction::North => bits << 8,
        Direction::South => bits >> 8,
        Direction::East => (bits & !FILE_H) << 1,
        Direction::West => (bits & !FILE_A) >> 1,
        Direction::NorthEast => (bits & !FILE_H) << 9,
        Direction::NorthWest => (bits & !FILE_A) << 7,
        Direction::SouthEast => (bits & !FILE_H) >> 7,
        Direction::SouthWest => (bits & !FILE_A) >> 9,
    }
}
