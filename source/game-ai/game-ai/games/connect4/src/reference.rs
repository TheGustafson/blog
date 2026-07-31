use crate::mv::{Cell, Column, Move, MoveList};
use crate::position::{GameResult, HEIGHT, MoveError, Side, WIDTH};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReferenceUndo {
    mv: Move,
    row: u8,
    side: Side,
}

/// An obvious implementation used as a differential oracle for the bitboard.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReferencePosition {
    cells: [Option<Side>; WIDTH * HEIGHT],
    heights: [u8; WIDTH],
    side_to_move: Side,
}

impl Default for ReferencePosition {
    fn default() -> Self {
        Self::start()
    }
}

impl ReferencePosition {
    pub const fn start() -> Self {
        Self {
            cells: [None; WIDTH * HEIGHT],
            heights: [0; WIDTH],
            side_to_move: Side::Red,
        }
    }

    pub const fn side_to_move(self) -> Side {
        self.side_to_move
    }

    pub fn side_at(self, cell: Cell) -> Option<Side> {
        self.cells[Self::index(cell.column(), cell.row())]
    }

    pub fn can_play(self, column: Column) -> bool {
        self.heights[column.index()] < HEIGHT as u8
    }

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

    pub fn result(self) -> GameResult {
        for column in 0..WIDTH as i8 {
            for row in 0..HEIGHT as i8 {
                let Some(side) = self.side_at(Cell::new(Column::new(column as u8), row as u8))
                else {
                    continue;
                };
                for (dc, dr) in [(1i8, 0i8), (0, 1), (1, 1), (1, -1)] {
                    let end_column = column + dc * 3;
                    let end_row = row + dr * 3;
                    if !(0..WIDTH as i8).contains(&end_column)
                        || !(0..HEIGHT as i8).contains(&end_row)
                    {
                        continue;
                    }
                    if (1..4).all(|step| {
                        self.side_at(Cell::new(
                            Column::new((column + dc * step) as u8),
                            (row + dr * step) as u8,
                        )) == Some(side)
                    }) {
                        return GameResult::Win(side);
                    }
                }
            }
        }
        if self.heights.iter().all(|height| *height == HEIGHT as u8) {
            GameResult::Draw
        } else {
            GameResult::Ongoing
        }
    }

    pub fn make_move(&mut self, mv: Move) -> Result<ReferenceUndo, MoveError> {
        if self.result() != GameResult::Ongoing {
            return Err(MoveError::GameOver);
        }
        let column = mv.column();
        if !self.can_play(column) {
            return Err(MoveError::Full(column));
        }
        let row = self.heights[column.index()];
        let side = self.side_to_move;
        self.cells[Self::index(column, row as usize)] = Some(side);
        self.heights[column.index()] += 1;
        self.side_to_move = side.other();
        Ok(ReferenceUndo { mv, row, side })
    }

    pub fn unmake_move(&mut self, undo: ReferenceUndo) {
        debug_assert_eq!(self.side_to_move, undo.side.other());
        debug_assert_eq!(
            self.cells[Self::index(undo.mv.column(), undo.row as usize)],
            Some(undo.side)
        );
        self.side_to_move = undo.side;
        self.heights[undo.mv.column().index()] -= 1;
        self.cells[Self::index(undo.mv.column(), undo.row as usize)] = None;
    }

    const fn index(column: Column, row: usize) -> usize {
        column.index() * HEIGHT + row
    }
}
