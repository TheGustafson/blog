use crate::Move;
use std::fmt;

pub(crate) const FULL: u16 = 0x01ff;
pub(crate) const LINES: [u16; 8] = [
    0b000_000_111,
    0b000_111_000,
    0b111_000_000,
    0b001_001_001,
    0b010_010_010,
    0b100_100_100,
    0b100_010_001,
    0b001_010_100,
];

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Player {
    X,
    O,
}

impl Player {
    pub const fn other(self) -> Self {
        match self {
            Self::X => Self::O,
            Self::O => Self::X,
        }
    }

    pub(crate) const fn index(self) -> usize {
        match self {
            Self::X => 0,
            Self::O => 1,
        }
    }
}

impl fmt::Display for Player {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::X => "X",
            Self::O => "O",
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MiniResult {
    Open,
    Draw,
    Win(Player),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GameResult {
    Ongoing,
    Draw,
    Win(Player),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MoveError {
    GameOver,
    WrongBoard { expected: u8 },
    ClosedBoard,
    Occupied,
}

impl fmt::Display for MoveError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::GameOver => formatter.write_str("the game is over"),
            Self::WrongBoard { expected } => {
                write!(
                    formatter,
                    "the move must be played in mini-board {expected}"
                )
            }
            Self::ClosedBoard => formatter.write_str("that mini-board is closed"),
            Self::Occupied => formatter.write_str("that cell is occupied"),
        }
    }
}

impl std::error::Error for MoveError {}

#[derive(Clone, Copy, Debug)]
pub struct MoveList {
    moves: [Move; 81],
    len: u8,
}

impl MoveList {
    const fn new() -> Self {
        Self {
            moves: [Move::new(0, 0); 81],
            len: 0,
        }
    }

    fn push(&mut self, mv: Move) {
        self.moves[usize::from(self.len)] = mv;
        self.len += 1;
    }

    pub const fn len(&self) -> usize {
        self.len as usize
    }

    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = Move> + '_ {
        self.moves[..self.len()].iter().copied()
    }

    pub fn contains(&self, mv: Move) -> bool {
        self.iter().any(|candidate| candidate == mv)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PositionStateError {
    CellsOutsideBoard { board: u8 },
    OverlappingCells { board: u8 },
    BothPlayersWonBoard { board: u8 },
    ImpossibleTurnCounts,
    BothPlayersWonGame,
    ActiveBoardClosed,
    ActiveBoardAfterGame,
}

impl fmt::Display for PositionStateError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CellsOutsideBoard { board } => {
                write!(formatter, "mini-board {board} contains out-of-range cells")
            }
            Self::OverlappingCells { board } => {
                write!(formatter, "X and O overlap in mini-board {board}")
            }
            Self::BothPlayersWonBoard { board } => {
                write!(formatter, "both players have won mini-board {board}")
            }
            Self::ImpossibleTurnCounts => {
                formatter.write_str("piece counts do not match the side to move")
            }
            Self::BothPlayersWonGame => formatter.write_str("both players have won the game"),
            Self::ActiveBoardClosed => formatter.write_str("the active mini-board is closed"),
            Self::ActiveBoardAfterGame => {
                formatter.write_str("a finished game cannot have an active mini-board")
            }
        }
    }
}

impl std::error::Error for PositionStateError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// The complete rule state needed to generate the next legal moves.
pub struct Position {
    x_cells: [u16; 9],
    o_cells: [u16; 9],
    macro_x: u16,
    macro_o: u16,
    macro_drawn: u16,
    active_board: Option<u8>,
    side_to_move: Player,
    ply: u8,
}

impl Default for Position {
    fn default() -> Self {
        Self::start()
    }
}

impl Position {
    pub const fn start() -> Self {
        Self {
            x_cells: [0; 9],
            o_cells: [0; 9],
            macro_x: 0,
            macro_o: 0,
            macro_drawn: 0,
            active_board: None,
            side_to_move: Player::X,
            ply: 0,
        }
    }

    pub fn from_moves(moves: &[Move]) -> Result<Self, MoveError> {
        let mut position = Self::start();
        for &mv in moves {
            position = position.play(mv)?;
        }
        Ok(position)
    }

    /// Imports a position while deriving and validating every macro-board mask.
    pub fn from_cells(
        x_cells: [u16; 9],
        o_cells: [u16; 9],
        active_board: Option<u8>,
        side_to_move: Player,
    ) -> Result<Self, PositionStateError> {
        for board in 0..9 {
            let index = board as usize;
            if (x_cells[index] | o_cells[index]) & !FULL != 0 {
                return Err(PositionStateError::CellsOutsideBoard { board });
            }
        }
        let x_count: u32 = x_cells.iter().map(|mask| mask.count_ones()).sum();
        let o_count: u32 = o_cells.iter().map(|mask| mask.count_ones()).sum();
        let counts_match = match side_to_move {
            Player::X => x_count == o_count,
            Player::O => x_count == o_count + 1,
        };
        if !counts_match {
            return Err(PositionStateError::ImpossibleTurnCounts);
        }

        let mut position = Self {
            x_cells,
            o_cells,
            macro_x: 0,
            macro_o: 0,
            macro_drawn: 0,
            active_board,
            side_to_move,
            ply: (x_count + o_count) as u8,
        };
        for board in 0..9 {
            let index = board as usize;
            if x_cells[index] & o_cells[index] != 0 {
                return Err(PositionStateError::OverlappingCells { board });
            }
            let x_won = has_line(x_cells[index]);
            let o_won = has_line(o_cells[index]);
            if x_won && o_won {
                return Err(PositionStateError::BothPlayersWonBoard { board });
            }
            let bit = 1 << board;
            if x_won {
                position.macro_x |= bit;
            } else if o_won {
                position.macro_o |= bit;
            } else if x_cells[index] | o_cells[index] == FULL {
                position.macro_drawn |= bit;
            }
        }
        if has_line(position.macro_x) && has_line(position.macro_o) {
            return Err(PositionStateError::BothPlayersWonGame);
        }
        if let Some(board) = active_board {
            if position.result() != GameResult::Ongoing {
                return Err(PositionStateError::ActiveBoardAfterGame);
            }
            if board >= 9 || position.mini_result(board) != MiniResult::Open {
                return Err(PositionStateError::ActiveBoardClosed);
            }
        }
        Ok(position)
    }

    pub const fn side_to_move(self) -> Player {
        self.side_to_move
    }

    pub const fn active_board(self) -> Option<u8> {
        self.active_board
    }

    pub const fn ply(self) -> u8 {
        self.ply
    }

    pub const fn macro_masks(self) -> (u16, u16, u16) {
        (self.macro_x, self.macro_o, self.macro_drawn)
    }

    pub const fn mini_masks(self, board: u8) -> (u16, u16) {
        (self.x_cells[board as usize], self.o_cells[board as usize])
    }

    pub const fn occupied(self, board: u8) -> u16 {
        let index = board as usize;
        self.x_cells[index] | self.o_cells[index]
    }

    pub fn player_at(self, mv: Move) -> Option<Player> {
        let bit = 1 << mv.cell();
        let board = mv.board() as usize;
        if self.x_cells[board] & bit != 0 {
            Some(Player::X)
        } else if self.o_cells[board] & bit != 0 {
            Some(Player::O)
        } else {
            None
        }
    }

    pub const fn closed_boards(self) -> u16 {
        self.macro_x | self.macro_o | self.macro_drawn
    }

    pub fn mini_result(self, board: u8) -> MiniResult {
        let bit = 1 << board;
        if self.macro_x & bit != 0 {
            MiniResult::Win(Player::X)
        } else if self.macro_o & bit != 0 {
            MiniResult::Win(Player::O)
        } else if self.macro_drawn & bit != 0 {
            MiniResult::Draw
        } else {
            MiniResult::Open
        }
    }

    pub fn result(self) -> GameResult {
        if has_line(self.macro_x) {
            GameResult::Win(Player::X)
        } else if has_line(self.macro_o) {
            GameResult::Win(Player::O)
        } else if self.closed_boards() == FULL {
            GameResult::Draw
        } else {
            GameResult::Ongoing
        }
    }

    pub fn macro_winning_line(self) -> Option<u16> {
        match self.result() {
            GameResult::Win(Player::X) => winning_line(self.macro_x),
            GameResult::Win(Player::O) => winning_line(self.macro_o),
            _ => None,
        }
    }

    pub fn legal_moves(self) -> MoveList {
        let mut moves = MoveList::new();
        if self.result() != GameResult::Ongoing {
            return moves;
        }
        if let Some(board) = self.active_board {
            self.add_board_moves(board, &mut moves);
        } else {
            for board in 0..9 {
                if self.mini_result(board) == MiniResult::Open {
                    self.add_board_moves(board, &mut moves);
                }
            }
        }
        moves
    }

    fn add_board_moves(self, board: u8, moves: &mut MoveList) {
        let mut empty = FULL & !self.occupied(board);
        while empty != 0 {
            let cell = empty.trailing_zeros() as u8;
            moves.push(Move::new(board, cell));
            empty &= empty - 1;
        }
    }

    /// Plays a move and applies forced routing or wildcard routing atomically.
    pub fn play(self, mv: Move) -> Result<Self, MoveError> {
        if self.result() != GameResult::Ongoing {
            return Err(MoveError::GameOver);
        }
        if let Some(expected) = self.active_board {
            if mv.board() != expected {
                return Err(MoveError::WrongBoard { expected });
            }
        }
        if self.mini_result(mv.board()) != MiniResult::Open {
            return Err(MoveError::ClosedBoard);
        }
        let board = mv.board() as usize;
        let cell_bit = 1 << mv.cell();
        if self.occupied(mv.board()) & cell_bit != 0 {
            return Err(MoveError::Occupied);
        }

        let mut next = self;
        match self.side_to_move {
            Player::X => next.x_cells[board] |= cell_bit,
            Player::O => next.o_cells[board] |= cell_bit,
        }
        let board_bit = 1 << mv.board();
        let played = match self.side_to_move {
            Player::X => next.x_cells[board],
            Player::O => next.o_cells[board],
        };
        if has_line(played) {
            match self.side_to_move {
                Player::X => next.macro_x |= board_bit,
                Player::O => next.macro_o |= board_bit,
            }
        } else if next.occupied(mv.board()) == FULL {
            next.macro_drawn |= board_bit;
        }

        next.side_to_move = self.side_to_move.other();
        next.ply += 1;
        next.active_board = if next.result() == GameResult::Ongoing
            && next.mini_result(mv.cell()) == MiniResult::Open
        {
            Some(mv.cell())
        } else {
            None
        };
        Ok(next)
    }

    pub fn hash(self) -> u64 {
        let mut hash = if self.side_to_move == Player::O {
            splitmix64(0x9b8d_f181_3c2a_6d75)
        } else {
            0
        };
        for board in 0..9 {
            for player in [Player::X, Player::O] {
                let mut cells = match player {
                    Player::X => self.x_cells[board],
                    Player::O => self.o_cells[board],
                };
                while cells != 0 {
                    let cell = cells.trailing_zeros() as u64;
                    let key = 1 + board as u64 * 18 + player.index() as u64 * 9 + cell;
                    hash ^= splitmix64(key);
                    cells &= cells - 1;
                }
            }
        }
        let active_key = self.active_board.map_or(9, u64::from);
        hash ^ splitmix64(0xa4cf_91e0_5d62_733b ^ active_key)
    }
}

pub(crate) fn has_line(mask: u16) -> bool {
    LINES.iter().any(|line| mask & line == *line)
}

pub(crate) fn winning_line(mask: u16) -> Option<u16> {
    LINES.into_iter().find(|line| mask & line == *line)
}

const fn splitmix64(mut value: u64) -> u64 {
    value = value.wrapping_add(0x9e37_79b9_7f4a_7c15);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}
