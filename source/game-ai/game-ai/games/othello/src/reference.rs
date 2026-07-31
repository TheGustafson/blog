use crate::{GameResult, Move, MoveError, MoveList, Side, Square};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReferenceUndo {
    mv: Move,
    side: Side,
    flipped: Vec<Square>,
}

/// Array-backed ray-walking implementation used for differential tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReferencePosition {
    cells: [Option<Side>; Square::COUNT],
    side_to_move: Side,
}

impl Default for ReferencePosition {
    fn default() -> Self {
        Self::start()
    }
}

impl ReferencePosition {
    pub const fn start() -> Self {
        let mut cells = [None; Square::COUNT];
        cells[28] = Some(Side::Black);
        cells[35] = Some(Side::Black);
        cells[27] = Some(Side::White);
        cells[36] = Some(Side::White);
        Self {
            cells,
            side_to_move: Side::Black,
        }
    }

    pub fn from_bits(black: u64, white: u64, side_to_move: Side) -> Result<Self, &'static str> {
        if black & white != 0 {
            return Err("black and white bitboards overlap");
        }
        let mut position = Self {
            cells: [None; Square::COUNT],
            side_to_move,
        };
        for square in Square::all() {
            let bit = 1u64 << square.index();
            position.cells[square.index()] = if black & bit != 0 {
                Some(Side::Black)
            } else if white & bit != 0 {
                Some(Side::White)
            } else {
                None
            };
        }
        Ok(position)
    }

    pub const fn side_to_move(self) -> Side {
        self.side_to_move
    }

    pub const fn side_at(self, square: Square) -> Option<Side> {
        self.cells[square.index()]
    }

    pub fn legal_moves(self) -> MoveList {
        let mut moves = MoveList::default();
        for square in Square::all() {
            if !self.flips_for(square, self.side_to_move).is_empty() {
                moves.push(Move::Place(square));
            }
        }
        if moves.is_empty() && self.has_placement(self.side_to_move.other()) {
            moves.push(Move::Pass);
        }
        moves
    }

    pub fn result(self) -> GameResult {
        if self.has_placement(self.side_to_move) || self.has_placement(self.side_to_move.other()) {
            return GameResult::Ongoing;
        }
        let black = self
            .cells
            .iter()
            .filter(|cell| **cell == Some(Side::Black))
            .count() as u8;
        let white = self
            .cells
            .iter()
            .filter(|cell| **cell == Some(Side::White))
            .count() as u8;
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

    pub fn make_move(&mut self, mv: Move) -> Result<ReferenceUndo, MoveError> {
        if self.result() != GameResult::Ongoing {
            return Err(MoveError::GameOver);
        }
        let side = self.side_to_move;
        let flipped = match mv {
            Move::Pass => {
                if self.has_placement(side) {
                    return Err(MoveError::PassNotAllowed);
                }
                if !self.has_placement(side.other()) {
                    return Err(MoveError::GameOver);
                }
                Vec::new()
            }
            Move::Place(square) => {
                if !self.has_placement(side) {
                    return Err(MoveError::MustPass);
                }
                let flipped = self.flips_for(square, side);
                if flipped.is_empty() {
                    return Err(MoveError::IllegalSquare(square));
                }
                self.cells[square.index()] = Some(side);
                for flipped_square in &flipped {
                    self.cells[flipped_square.index()] = Some(side);
                }
                flipped
            }
        };
        self.side_to_move = side.other();
        Ok(ReferenceUndo { mv, side, flipped })
    }

    pub fn unmake_move(&mut self, undo: ReferenceUndo) {
        debug_assert_eq!(self.side_to_move, undo.side.other());
        self.side_to_move = undo.side;
        if let Move::Place(square) = undo.mv {
            self.cells[square.index()] = None;
            for flipped_square in undo.flipped {
                self.cells[flipped_square.index()] = Some(undo.side.other());
            }
        }
    }

    fn has_placement(self, side: Side) -> bool {
        Square::all().any(|square| !self.flips_for(square, side).is_empty())
    }

    fn flips_for(self, square: Square, side: Side) -> Vec<Square> {
        if self.side_at(square).is_some() {
            return Vec::new();
        }
        let mut flips = Vec::new();
        for (df, dr) in [
            (-1i8, -1i8),
            (0, -1),
            (1, -1),
            (-1, 0),
            (1, 0),
            (-1, 1),
            (0, 1),
            (1, 1),
        ] {
            let mut ray = Vec::new();
            let mut file = square.file() as i8 + df;
            let mut rank = square.rank() as i8 + dr;
            while (0..8).contains(&file) && (0..8).contains(&rank) {
                let target = Square::new((rank * 8 + file) as u8);
                match self.side_at(target) {
                    Some(found) if found == side.other() => ray.push(target),
                    Some(found) if found == side => {
                        if !ray.is_empty() {
                            flips.extend(ray);
                        }
                        break;
                    }
                    Some(_) | None => break,
                }
                file += df;
                rank += dr;
            }
        }
        flips
    }
}
