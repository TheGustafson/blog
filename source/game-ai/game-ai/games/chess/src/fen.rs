use crate::{CastlingRights, Color, Piece, PieceKind, Position, Square};
use std::fmt;

impl Position {
    /// Parses and validates all six fields of a FEN position.
    pub fn from_fen(fen: &str) -> Result<Self, &'static str> {
        let fields: Vec<_> = fen.split_whitespace().collect();
        if fields.len() != 6 {
            return Err("FEN must contain six fields");
        }
        let mut position = Self::empty();
        let ranks: Vec<_> = fields[0].split('/').collect();
        if ranks.len() != 8 {
            return Err("FEN placement must contain eight ranks");
        }
        for (fen_rank, text) in ranks.iter().enumerate() {
            let rank = 7 - fen_rank as u8;
            let mut file = 0u8;
            for value in text.chars() {
                if let Some(empty) = value.to_digit(10) {
                    if empty == 0 || empty > 8 {
                        return Err("FEN empty run must be 1..8");
                    }
                    file = file
                        .checked_add(empty as u8)
                        .ok_or("FEN rank is too wide")?;
                } else {
                    let piece =
                        Piece::from_fen_char(value).ok_or("FEN contains an unknown piece")?;
                    if file >= 8 {
                        return Err("FEN rank is too wide");
                    }
                    position.put_piece(Square::from_file_rank(file, rank), piece);
                    file += 1;
                }
            }
            if file != 8 {
                return Err("each FEN rank must describe eight squares");
            }
        }
        position.side_to_move = match fields[1] {
            "w" => Color::White,
            "b" => Color::Black,
            _ => return Err("FEN side must be w or b"),
        };
        let mut castling = 0;
        if fields[2] != "-" {
            for value in fields[2].chars() {
                let flag = match value {
                    'K' => CastlingRights::WHITE_KING,
                    'Q' => CastlingRights::WHITE_QUEEN,
                    'k' => CastlingRights::BLACK_KING,
                    'q' => CastlingRights::BLACK_QUEEN,
                    _ => return Err("FEN castling field contains an unknown right"),
                };
                if castling & flag != 0 {
                    return Err("FEN castling field contains a duplicate right");
                }
                castling |= flag;
            }
        }
        position.castling = CastlingRights::new(castling);
        position.en_passant = if fields[3] == "-" {
            None
        } else {
            let square: Square = fields[3].parse()?;
            if square.rank() != 2 && square.rank() != 5 {
                return Err("FEN en-passant square must be on rank 3 or 6");
            }
            Some(square)
        };
        position.halfmove_clock = fields[4]
            .parse()
            .map_err(|_| "FEN halfmove clock must be an integer")?;
        position.fullmove_number = fields[5]
            .parse()
            .map_err(|_| "FEN fullmove number must be an integer")?;
        if position.fullmove_number == 0 {
            return Err("FEN fullmove number starts at one");
        }
        for color in Color::ALL {
            if position.pieces(color, PieceKind::King).count_ones() != 1 {
                return Err("FEN must contain exactly one king per side");
            }
            let pawn_count = position.pieces(color, PieceKind::Pawn).count_ones();
            if pawn_count > 8 {
                return Err("FEN cannot contain more than eight pawns per side");
            }
            if position.occupancy(color).count_ones() > 16 {
                return Err("FEN cannot contain more than sixteen pieces per side");
            }
            let promoted_pieces = [
                (PieceKind::Queen, 1),
                (PieceKind::Rook, 2),
                (PieceKind::Bishop, 2),
                (PieceKind::Knight, 2),
            ]
            .into_iter()
            .map(|(kind, original)| {
                position
                    .pieces(color, kind)
                    .count_ones()
                    .saturating_sub(original)
            })
            .sum::<u32>();
            if promoted_pieces > 8 - pawn_count {
                return Err("FEN promoted material requires enough missing pawns");
            }
        }
        const BACK_RANKS: u64 = 0xff | (0xff << 56);
        let pawns = position.pieces(Color::White, PieceKind::Pawn)
            | position.pieces(Color::Black, PieceKind::Pawn);
        if pawns & BACK_RANKS != 0 {
            return Err("FEN cannot place a pawn on the first or eighth rank");
        }
        for (flag, color, rook_file) in [
            (CastlingRights::WHITE_KING, Color::White, 7),
            (CastlingRights::WHITE_QUEEN, Color::White, 0),
            (CastlingRights::BLACK_KING, Color::Black, 7),
            (CastlingRights::BLACK_QUEEN, Color::Black, 0),
        ] {
            if position.castling.has(flag) {
                let rank = color.home_rank();
                if position.piece_at(Square::from_file_rank(4, rank))
                    != Some(Piece::new(color, PieceKind::King))
                    || position.piece_at(Square::from_file_rank(rook_file, rank))
                        != Some(Piece::new(color, PieceKind::Rook))
                {
                    return Err("FEN castling right requires its king and rook");
                }
            }
        }
        if let Some(target) = position.en_passant {
            let expected_rank = match position.side_to_move {
                Color::White => 5,
                Color::Black => 2,
            };
            if target.rank() != expected_rank {
                return Err("FEN en-passant rank does not match the side to move");
            }
            if position.piece_at(target).is_some() {
                return Err("FEN en-passant target must be empty");
            }
            let captured = target
                .offset(0, -position.side_to_move.pawn_step())
                .ok_or("FEN en-passant capture square is outside the board")?;
            if position.piece_at(captured)
                != Some(Piece::new(position.side_to_move.other(), PieceKind::Pawn))
            {
                return Err("FEN en-passant target has no capturable pawn");
            }
            let origin = target
                .offset(0, position.side_to_move.pawn_step())
                .ok_or("FEN en-passant origin square is outside the board")?;
            if position.piece_at(origin).is_some() {
                return Err("FEN en-passant pawn origin must be empty");
            }
            if position.halfmove_clock != 0 {
                return Err("FEN en-passant target requires a zero halfmove clock");
            }
        }
        if position.in_check(position.side_to_move.other()) {
            return Err("FEN leaves the side that just moved in check");
        }
        position.key = crate::zobrist::compute(&position);
        position
            .is_fen_consistent()
            .then_some(position)
            .ok_or("FEN produced an inconsistent position")
    }

    /// Serializes all rule-relevant state as FEN.
    pub fn fen(&self) -> String {
        self.to_string()
    }

    fn is_fen_consistent(&self) -> bool {
        crate::zobrist::compute(self) == self.key
    }
}

impl fmt::Display for Position {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for rank in (0..8).rev() {
            if rank != 7 {
                f.write_str("/")?;
            }
            let mut empty = 0;
            for file in 0..8 {
                let square = Square::from_file_rank(file, rank);
                if let Some(piece) = self.piece_at(square) {
                    if empty > 0 {
                        write!(f, "{empty}")?;
                        empty = 0;
                    }
                    write!(f, "{}", piece.fen_char())?;
                } else {
                    empty += 1;
                }
            }
            if empty > 0 {
                write!(f, "{empty}")?;
            }
        }
        write!(
            f,
            " {} ",
            if self.side_to_move() == Color::White {
                "w"
            } else {
                "b"
            }
        )?;
        let rights = self.castling_rights();
        if rights.bits() == 0 {
            f.write_str("-")?;
        } else {
            for (flag, symbol) in [
                (CastlingRights::WHITE_KING, 'K'),
                (CastlingRights::WHITE_QUEEN, 'Q'),
                (CastlingRights::BLACK_KING, 'k'),
                (CastlingRights::BLACK_QUEEN, 'q'),
            ] {
                if rights.has(flag) {
                    write!(f, "{symbol}")?;
                }
            }
        }
        write!(
            f,
            " {} {} {}",
            self.en_passant()
                .map_or_else(|| "-".to_owned(), |square| square.to_string()),
            self.halfmove_clock(),
            self.fullmove_number()
        )
    }
}

#[test]
fn built_in_start_fen_is_canonical() {
    assert_eq!(Position::start().fen(), crate::position::START_FEN);
}
