use std::fmt;
use std::str::FromStr;

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
/// One cell on the global 9×9 board, stored as a mini-board and local cell.
pub struct Move(u8);

impl Move {
    pub const fn new(board: u8, cell: u8) -> Self {
        assert!(board < 9 && cell < 9, "board and cell must be in 0..9");
        Self(board * 9 + cell)
    }

    pub const fn from_global_index(index: u8) -> Self {
        assert!(index < 81, "global index must be in 0..81");
        let row = index / 9;
        let column = index % 9;
        let board = (row / 3) * 3 + column / 3;
        let cell = (row % 3) * 3 + column % 3;
        Self::new(board, cell)
    }

    pub const fn board(self) -> u8 {
        self.0 / 9
    }

    pub const fn cell(self) -> u8 {
        self.0 % 9
    }

    pub const fn global_index(self) -> u8 {
        let board_row = self.board() / 3;
        let board_column = self.board() % 3;
        let cell_row = self.cell() / 3;
        let cell_column = self.cell() % 3;
        (board_row * 3 + cell_row) * 9 + board_column * 3 + cell_column
    }
}

impl fmt::Display for Move {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let index = self.global_index();
        let file = char::from(b'a' + index % 9);
        let rank = char::from(b'9' - index / 9);
        write!(formatter, "{file}{rank}")
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ParseMoveError;

impl fmt::Display for ParseMoveError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("moves must be coordinates from a1 through i9")
    }
}

impl std::error::Error for ParseMoveError {}

impl FromStr for Move {
    type Err = ParseMoveError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let bytes = value.as_bytes();
        if bytes.len() != 2
            || !(b'a'..=b'i').contains(&bytes[0].to_ascii_lowercase())
            || !(b'1'..=b'9').contains(&bytes[1])
        {
            return Err(ParseMoveError);
        }
        let column = bytes[0].to_ascii_lowercase() - b'a';
        let row = b'9' - bytes[1];
        Ok(Self::from_global_index(row * 9 + column))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coordinates_round_trip_every_square() {
        for index in 0..81 {
            let mv = Move::from_global_index(index);
            assert_eq!(mv.to_string().parse(), Ok(mv));
            assert_eq!(mv.global_index(), index);
        }
    }

    #[test]
    fn board_and_cell_mapping_matches_the_visual_grid() {
        assert_eq!("a9".parse(), Ok(Move::new(0, 0)));
        assert_eq!("e5".parse(), Ok(Move::new(4, 4)));
        assert_eq!("i1".parse(), Ok(Move::new(8, 8)));
        assert_eq!("d7".parse(), Ok(Move::new(1, 6)));
    }
}
