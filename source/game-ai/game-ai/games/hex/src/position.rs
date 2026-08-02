use crate::board::{BoardSize, Cell, Move};
use crate::connectivity::{connection_path, has_connection, insert, occupied};
use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Color {
    Red,
    Blue,
}

impl Color {
    pub const fn other(self) -> Self {
        match self {
            Self::Red => Self::Blue,
            Self::Blue => Self::Red,
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Red => "red",
            Self::Blue => "blue",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Seat {
    One,
    Two,
}

impl Seat {
    pub const fn other(self) -> Self {
        match self {
            Self::One => Self::Two,
            Self::Two => Self::One,
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::One => "one",
            Self::Two => "two",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SwapRule {
    Enabled,
    Disabled,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GameResult {
    Ongoing,
    Win(Seat),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Position {
    size: BoardSize,
    red: [u64; 9],
    blue: [u64; 9],
    seat_to_move: Seat,
    colors_swapped: bool,
    swap_rule: SwapRule,
    swap_available: bool,
    actions: u16,
    stones: u16,
    last_placement: Option<Cell>,
    result: GameResult,
}

impl Position {
    pub const fn new(size: BoardSize, swap_rule: SwapRule) -> Self {
        Self {
            size,
            red: [0; 9],
            blue: [0; 9],
            seat_to_move: Seat::One,
            colors_swapped: false,
            swap_rule,
            swap_available: false,
            actions: 0,
            stones: 0,
            last_placement: None,
            result: GameResult::Ongoing,
        }
    }

    pub fn from_moves(
        size: BoardSize,
        swap_rule: SwapRule,
        moves: &[Move],
    ) -> Result<Self, MoveError> {
        moves
            .iter()
            .try_fold(Self::new(size, swap_rule), |position, &mv| {
                position.play(mv)
            })
    }

    pub const fn size(self) -> BoardSize {
        self.size
    }

    pub const fn seat_to_move(self) -> Seat {
        self.seat_to_move
    }

    pub const fn color_to_move(self) -> Color {
        self.color_for_seat(self.seat_to_move)
    }

    pub const fn color_for_seat(self, seat: Seat) -> Color {
        match (seat, self.colors_swapped) {
            (Seat::One, false) | (Seat::Two, true) => Color::Red,
            (Seat::Two, false) | (Seat::One, true) => Color::Blue,
        }
    }

    pub const fn seat_for_color(self, color: Color) -> Seat {
        match (color, self.colors_swapped) {
            (Color::Red, false) | (Color::Blue, true) => Seat::One,
            (Color::Blue, false) | (Color::Red, true) => Seat::Two,
        }
    }

    pub const fn colors_swapped(self) -> bool {
        self.colors_swapped
    }

    pub const fn swap_rule(self) -> SwapRule {
        self.swap_rule
    }

    pub const fn swap_available(self) -> bool {
        self.swap_available
    }

    pub const fn actions(self) -> u16 {
        self.actions
    }

    pub const fn stones(self) -> u16 {
        self.stones
    }

    pub const fn last_placement(self) -> Option<Cell> {
        self.last_placement
    }

    pub const fn result(self) -> GameResult {
        self.result
    }

    pub fn color_at(self, cell: Cell) -> Option<Color> {
        if !self.size.contains(cell) {
            return None;
        }
        if occupied(&self.red, cell) {
            Some(Color::Red)
        } else if occupied(&self.blue, cell) {
            Some(Color::Blue)
        } else {
            None
        }
    }

    pub fn is_legal(self, mv: Move) -> bool {
        if self.result != GameResult::Ongoing {
            return false;
        }
        match mv {
            Move::Swap => self.swap_available,
            Move::Place(cell) => self.size.contains(cell) && self.color_at(cell).is_none(),
        }
    }

    pub fn legal_moves(self) -> Vec<Move> {
        if self.result != GameResult::Ongoing {
            return Vec::new();
        }
        let mut moves = Vec::with_capacity(self.empty_count() + usize::from(self.swap_available));
        if self.swap_available {
            moves.push(Move::Swap);
        }
        for dense in 0..self.size.cell_count() {
            let cell = Cell::from_dense(dense, self.size);
            if self.color_at(cell).is_none() {
                moves.push(Move::Place(cell));
            }
        }
        moves
    }

    pub fn play(mut self, mv: Move) -> Result<Self, MoveError> {
        if self.result != GameResult::Ongoing {
            return Err(MoveError::GameOver);
        }
        match mv {
            Move::Swap if self.swap_available => {
                self.colors_swapped = !self.colors_swapped;
                self.swap_available = false;
                self.last_placement = None;
                self.seat_to_move = self.seat_to_move.other();
                self.actions += 1;
                Ok(self)
            }
            Move::Swap => Err(MoveError::SwapUnavailable),
            Move::Place(cell) if !self.size.contains(cell) => Err(MoveError::OutsideBoard),
            Move::Place(cell) if self.color_at(cell).is_some() => Err(MoveError::Occupied),
            Move::Place(cell) => {
                let mover = self.seat_to_move;
                let color = self.color_for_seat(mover);
                insert(self.bits_mut(color), cell);
                self.stones += 1;
                self.last_placement = Some(cell);
                self.actions += 1;
                self.swap_available =
                    self.stones == 1 && self.actions == 1 && self.swap_rule == SwapRule::Enabled;
                self.seat_to_move = self.seat_to_move.other();
                if has_connection(self.bits(color), self.size, color) {
                    self.result = GameResult::Win(mover);
                    self.swap_available = false;
                }
                Ok(self)
            }
        }
    }

    pub fn winning_path(self) -> Vec<Cell> {
        let GameResult::Win(seat) = self.result else {
            return Vec::new();
        };
        let color = self.color_for_seat(seat);
        connection_path(self.bits(color), self.size, color).unwrap_or_default()
    }

    pub(crate) fn empty_count(self) -> usize {
        usize::from(self.size.cell_count() - self.stones)
    }

    pub(crate) fn empty_cells(self) -> Vec<Cell> {
        (0..self.size.cell_count())
            .map(|dense| Cell::from_dense(dense, self.size))
            .filter(|&cell| self.color_at(cell).is_none())
            .collect()
    }

    pub(crate) fn connects_after(self, color: Color, cell: Cell) -> bool {
        if self.color_at(cell).is_some() || !self.size.contains(cell) {
            return false;
        }
        let mut bits = *self.bits(color);
        insert(&mut bits, cell);
        has_connection(&bits, self.size, color)
    }

    pub(crate) fn stone_count(self, color: Color) -> u32 {
        self.bits(color).iter().map(|word| word.count_ones()).sum()
    }

    pub(crate) fn rollout_decline_swap(&mut self) {
        self.swap_available = false;
    }

    pub(crate) fn rollout_place(&mut self, cell: Cell) {
        let color = self.color_to_move();
        insert(self.bits_mut(color), cell);
        self.stones += 1;
        self.last_placement = Some(cell);
        self.actions += 1;
        self.swap_available = false;
        self.seat_to_move = self.seat_to_move.other();
    }

    pub(crate) fn winner_on_full_board(self) -> Seat {
        if has_connection(&self.red, self.size, Color::Red) {
            self.seat_for_color(Color::Red)
        } else {
            debug_assert!(has_connection(&self.blue, self.size, Color::Blue));
            self.seat_for_color(Color::Blue)
        }
    }

    fn bits(&self, color: Color) -> &[u64; 9] {
        match color {
            Color::Red => &self.red,
            Color::Blue => &self.blue,
        }
    }

    fn bits_mut(&mut self, color: Color) -> &mut [u64; 9] {
        match color {
            Color::Red => &mut self.red,
            Color::Blue => &mut self.blue,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MoveError {
    GameOver,
    OutsideBoard,
    Occupied,
    SwapUnavailable,
}

impl fmt::Display for MoveError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::GameOver => "the game is over",
            Self::OutsideBoard => "the cell is outside this board",
            Self::Occupied => "the cell is occupied",
            Self::SwapUnavailable => "the swap move is not available",
        })
    }
}

impl std::error::Error for MoveError {}
