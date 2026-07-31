use crate::attacks;
use crate::zobrist;
use crate::{Color, Move, MoveKind, MoveList, Piece, PieceKind, Square};
use std::fmt;

pub(crate) const START_FEN: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";

/// Four-bit set of orthodox castling permissions.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CastlingRights(u8);

impl CastlingRights {
    pub const WHITE_KING: u8 = 1;
    pub const WHITE_QUEEN: u8 = 2;
    pub const BLACK_KING: u8 = 4;
    pub const BLACK_QUEEN: u8 = 8;

    pub const fn new(bits: u8) -> Self {
        Self(bits & 0x0f)
    }

    pub const fn bits(self) -> u8 {
        self.0
    }

    pub const fn has(self, flag: u8) -> bool {
        self.0 & flag != 0
    }

    pub(crate) fn remove(&mut self, flags: u8) {
        self.0 &= !flags;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameResult {
    Ongoing,
    Checkmate { winner: Color },
    Stalemate,
    FiftyMoveDraw,
    InsufficientMaterialDraw,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoveError {
    NoPiece(Square),
    WrongSide(Square),
    Illegal(Move),
}

impl fmt::Display for MoveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoPiece(square) => write!(f, "there is no piece on {square}"),
            Self::WrongSide(square) => write!(f, "the piece on {square} is not the side to move"),
            Self::Illegal(mv) => write!(f, "{mv} is not legal in this position"),
        }
    }
}

impl std::error::Error for MoveError {}

/// Opaque record containing every field changed by one move.
///
/// Pass it back to [`Position::unmake_move`] to restore the exact prior
/// position, including clocks, castling, en-passant state, and hash key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Undo {
    mv: Move,
    moved: Piece,
    captured: Option<(Piece, Square)>,
    castling: CastlingRights,
    en_passant: Option<Square>,
    halfmove_clock: u16,
    fullmove_number: u16,
    key: u64,
}

/// Complete, reversible chess position.
///
/// Piece placement is held in synchronized bitboards and a mailbox. The
/// position also owns all rule state and its incremental Zobrist key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Position {
    pub(crate) pieces: [[u64; 6]; 2],
    pub(crate) color_occupancy: [u64; 2],
    pub(crate) occupied: u64,
    pub(crate) mailbox: [Option<Piece>; 64],
    pub(crate) side_to_move: Color,
    pub(crate) castling: CastlingRights,
    pub(crate) en_passant: Option<Square>,
    pub(crate) halfmove_clock: u16,
    pub(crate) fullmove_number: u16,
    pub(crate) key: u64,
}

impl Default for Position {
    fn default() -> Self {
        Self::start()
    }
}

impl Position {
    pub fn start() -> Self {
        Self::from_fen(START_FEN).expect("the built-in starting FEN must be valid")
    }

    pub(crate) fn empty() -> Self {
        Self {
            pieces: [[0; 6]; 2],
            color_occupancy: [0; 2],
            occupied: 0,
            mailbox: [None; 64],
            side_to_move: Color::White,
            castling: CastlingRights::default(),
            en_passant: None,
            halfmove_clock: 0,
            fullmove_number: 1,
            key: 0,
        }
    }

    pub const fn side_to_move(&self) -> Color {
        self.side_to_move
    }

    pub const fn castling_rights(&self) -> CastlingRights {
        self.castling
    }

    pub const fn en_passant(&self) -> Option<Square> {
        self.en_passant
    }

    pub const fn halfmove_clock(&self) -> u16 {
        self.halfmove_clock
    }

    pub const fn fullmove_number(&self) -> u16 {
        self.fullmove_number
    }

    pub const fn key(&self) -> u64 {
        self.key
    }

    pub const fn occupied(&self) -> u64 {
        self.occupied
    }

    pub const fn occupancy(&self, color: Color) -> u64 {
        self.color_occupancy[color.index()]
    }

    pub const fn pieces(&self, color: Color, kind: PieceKind) -> u64 {
        self.pieces[color.index()][kind.index()]
    }

    pub fn piece_at(&self, square: Square) -> Option<Piece> {
        self.mailbox[square.index()]
    }

    pub fn king_square(&self, color: Color) -> Option<Square> {
        let king = self.pieces(color, PieceKind::King);
        (king != 0).then(|| Square::new(king.trailing_zeros() as u8))
    }

    pub fn in_check(&self, color: Color) -> bool {
        self.king_square(color)
            .is_some_and(|king| attacks::square_is_attacked(self, king, color.other()))
    }

    /// Generates every legal move in deterministic order.
    ///
    /// Move generation temporarily makes pseudo-legal moves to reject those
    /// that leave the moving side's king in check.
    pub fn legal_moves(&mut self) -> MoveList {
        let pseudo = crate::movegen::pseudo_legal(self);
        let mover = self.side_to_move;
        let mut legal = MoveList::default();
        for mv in pseudo {
            let undo = self
                .make_unchecked(mv)
                .expect("generated pseudo-legal move must have a moving piece");
            let leaves_king_safe = !self.in_check(mover);
            self.unmake_move(undo);
            if leaves_king_safe {
                legal.push(mv);
            }
        }
        legal
    }

    pub fn find_move(&mut self, notation: &str) -> Result<Move, &'static str> {
        self.legal_moves()
            .into_iter()
            .find(|mv| mv.to_string() == notation)
            .ok_or("move is not legal in this position")
    }

    pub fn make_move(&mut self, mv: Move) -> Result<Undo, MoveError> {
        let piece = self
            .piece_at(mv.from())
            .ok_or(MoveError::NoPiece(mv.from()))?;
        if piece.color != self.side_to_move {
            return Err(MoveError::WrongSide(mv.from()));
        }
        if !self.legal_moves().as_slice().contains(&mv) {
            return Err(MoveError::Illegal(mv));
        }
        self.make_unchecked(mv)
    }

    pub fn play_uci(&mut self, notation: &str) -> Result<Undo, &'static str> {
        let mv = self.find_move(notation)?;
        self.make_unchecked(mv)
            .map_err(|_| "move could not be made")
    }

    /// Restores the exact position that existed before `undo`.
    pub fn unmake_move(&mut self, undo: Undo) {
        self.side_to_move = undo.moved.color;
        let destination_piece = self
            .piece_at(undo.mv.to())
            .expect("made move must leave a piece on its destination");
        self.remove_piece(undo.mv.to(), destination_piece);

        match undo.mv.kind() {
            MoveKind::CastleKingSide => {
                let rank = undo.moved.color.home_rank();
                let rook_to = Square::from_file_rank(5, rank);
                let rook_from = Square::from_file_rank(7, rank);
                let rook = self
                    .piece_at(rook_to)
                    .expect("castling must leave its rook on f-file");
                self.remove_piece(rook_to, rook);
                self.put_piece(rook_from, rook);
            }
            MoveKind::CastleQueenSide => {
                let rank = undo.moved.color.home_rank();
                let rook_to = Square::from_file_rank(3, rank);
                let rook_from = Square::from_file_rank(0, rank);
                let rook = self
                    .piece_at(rook_to)
                    .expect("castling must leave its rook on d-file");
                self.remove_piece(rook_to, rook);
                self.put_piece(rook_from, rook);
            }
            MoveKind::Normal | MoveKind::EnPassant | MoveKind::Promotion(_) => {}
        }

        self.put_piece(undo.mv.from(), undo.moved);
        if let Some((captured, square)) = undo.captured {
            self.put_piece(square, captured);
        }
        self.castling = undo.castling;
        self.en_passant = undo.en_passant;
        self.halfmove_clock = undo.halfmove_clock;
        self.fullmove_number = undo.fullmove_number;
        self.key = undo.key;
        debug_assert!(self.is_consistent());
    }

    /// Returns the current rules-level result.
    ///
    /// Threefold repetition depends on history outside a single position and
    /// is therefore handled by the search/session layer.
    pub fn result(&mut self) -> GameResult {
        if self.legal_moves().is_empty() {
            if self.in_check(self.side_to_move) {
                GameResult::Checkmate {
                    winner: self.side_to_move.other(),
                }
            } else {
                GameResult::Stalemate
            }
        } else if self.halfmove_clock >= 100 {
            GameResult::FiftyMoveDraw
        } else if self.has_insufficient_material() {
            GameResult::InsufficientMaterialDraw
        } else {
            GameResult::Ongoing
        }
    }

    /// Returns true only for material sets from which no legal sequence can
    /// end in checkmate. This intentionally avoids broad "minor pieces only"
    /// shortcuts: two knights, bishop-and-knight, and opposite-colored bishops
    /// can all participate in a helpmate.
    pub fn has_insufficient_material(&self) -> bool {
        let pawns =
            self.pieces(Color::White, PieceKind::Pawn) | self.pieces(Color::Black, PieceKind::Pawn);
        let rooks =
            self.pieces(Color::White, PieceKind::Rook) | self.pieces(Color::Black, PieceKind::Rook);
        let queens = self.pieces(Color::White, PieceKind::Queen)
            | self.pieces(Color::Black, PieceKind::Queen);
        if pawns | rooks | queens != 0 {
            return false;
        }

        let knights = self.pieces(Color::White, PieceKind::Knight)
            | self.pieces(Color::Black, PieceKind::Knight);
        let bishops = self.pieces(Color::White, PieceKind::Bishop)
            | self.pieces(Color::Black, PieceKind::Bishop);
        if bishops == 0 {
            return knights.count_ones() <= 1;
        }
        if knights != 0 {
            return false;
        }

        let mut remaining = bishops;
        let first = remaining.trailing_zeros() as u8;
        let first_color = ((first & 7) + (first >> 3)) & 1;
        remaining &= remaining - 1;
        while remaining != 0 {
            let square = remaining.trailing_zeros() as u8;
            let color = ((square & 7) + (square >> 3)) & 1;
            if color != first_color {
                return false;
            }
            remaining &= remaining - 1;
        }
        true
    }

    pub fn assert_consistent(&self) {
        assert!(
            self.is_consistent(),
            "chess position representations drifted"
        );
    }

    pub(crate) fn make_unchecked(&mut self, mv: Move) -> Result<Undo, MoveError> {
        let moved = self
            .piece_at(mv.from())
            .ok_or(MoveError::NoPiece(mv.from()))?;
        if moved.color != self.side_to_move {
            return Err(MoveError::WrongSide(mv.from()));
        }
        let previous = Undo {
            mv,
            moved,
            captured: None,
            castling: self.castling,
            en_passant: self.en_passant,
            halfmove_clock: self.halfmove_clock,
            fullmove_number: self.fullmove_number,
            key: self.key,
        };

        self.key ^= zobrist::castling(self.castling.bits());
        if let Some(file) = self.hashed_en_passant_file() {
            self.key ^= zobrist::en_passant(file);
        }
        self.en_passant = None;
        self.remove_piece(mv.from(), moved);

        let capture_square = if mv.kind() == MoveKind::EnPassant {
            mv.to()
                .offset(0, -moved.color.pawn_step())
                .expect("en-passant destination must have a square behind it")
        } else {
            mv.to()
        };
        let captured = self.piece_at(capture_square);
        if let Some(piece) = captured {
            self.remove_piece(capture_square, piece);
        }

        let placed = match mv.kind() {
            MoveKind::Promotion(kind) => Piece::new(moved.color, kind),
            MoveKind::Normal
            | MoveKind::CastleKingSide
            | MoveKind::CastleQueenSide
            | MoveKind::EnPassant => moved,
        };
        self.put_piece(mv.to(), placed);

        match mv.kind() {
            MoveKind::CastleKingSide => {
                let rank = moved.color.home_rank();
                self.move_piece(
                    Square::from_file_rank(7, rank),
                    Square::from_file_rank(5, rank),
                );
            }
            MoveKind::CastleQueenSide => {
                let rank = moved.color.home_rank();
                self.move_piece(
                    Square::from_file_rank(0, rank),
                    Square::from_file_rank(3, rank),
                );
            }
            MoveKind::Normal | MoveKind::EnPassant | MoveKind::Promotion(_) => {}
        }

        self.update_castling_rights(
            moved,
            mv.from(),
            captured.map(|piece| (piece, capture_square)),
        );
        if moved.kind == PieceKind::Pawn && mv.from().rank().abs_diff(mv.to().rank()) == 2 {
            self.en_passant = mv.from().offset(0, moved.color.pawn_step());
        }
        self.halfmove_clock = if moved.kind == PieceKind::Pawn || captured.is_some() {
            0
        } else {
            self.halfmove_clock.saturating_add(1)
        };
        if moved.color == Color::Black {
            self.fullmove_number = self.fullmove_number.saturating_add(1);
        }
        self.side_to_move = moved.color.other();
        self.key ^= zobrist::side();
        self.key ^= zobrist::castling(self.castling.bits());
        if let Some(file) = self.hashed_en_passant_file() {
            self.key ^= zobrist::en_passant(file);
        }
        debug_assert!(self.is_consistent());
        Ok(Undo {
            captured: captured.map(|piece| (piece, capture_square)),
            ..previous
        })
    }

    pub(crate) fn put_piece(&mut self, square: Square, piece: Piece) {
        debug_assert!(self.mailbox[square.index()].is_none());
        let bit = attacks::bit(square);
        self.pieces[piece.color.index()][piece.kind.index()] |= bit;
        self.color_occupancy[piece.color.index()] |= bit;
        self.occupied |= bit;
        self.mailbox[square.index()] = Some(piece);
        self.key ^= zobrist::piece(piece, square);
    }

    fn remove_piece(&mut self, square: Square, piece: Piece) {
        debug_assert_eq!(self.mailbox[square.index()], Some(piece));
        let bit = attacks::bit(square);
        self.pieces[piece.color.index()][piece.kind.index()] &= !bit;
        self.color_occupancy[piece.color.index()] &= !bit;
        self.occupied &= !bit;
        self.mailbox[square.index()] = None;
        self.key ^= zobrist::piece(piece, square);
    }

    fn move_piece(&mut self, from: Square, to: Square) {
        let piece = self
            .piece_at(from)
            .expect("moving a piece internally requires a source piece");
        self.remove_piece(from, piece);
        self.put_piece(to, piece);
    }

    fn update_castling_rights(
        &mut self,
        moved: Piece,
        from: Square,
        captured: Option<(Piece, Square)>,
    ) {
        if moved.kind == PieceKind::King {
            self.castling.remove(match moved.color {
                Color::White => CastlingRights::WHITE_KING | CastlingRights::WHITE_QUEEN,
                Color::Black => CastlingRights::BLACK_KING | CastlingRights::BLACK_QUEEN,
            });
        }
        if moved.kind == PieceKind::Rook {
            self.revoke_rook_square(from);
        }
        if let Some((piece, square)) = captured {
            if piece.kind == PieceKind::Rook {
                self.revoke_rook_square(square);
            }
        }
    }

    fn revoke_rook_square(&mut self, square: Square) {
        let flag = match (square.file(), square.rank()) {
            (7, 0) => CastlingRights::WHITE_KING,
            (0, 0) => CastlingRights::WHITE_QUEEN,
            (7, 7) => CastlingRights::BLACK_KING,
            (0, 7) => CastlingRights::BLACK_QUEEN,
            _ => 0,
        };
        self.castling.remove(flag);
    }

    pub(crate) fn hashed_en_passant_file(&self) -> Option<u8> {
        let target = self.en_passant?;
        let source_rank_delta = -self.side_to_move.pawn_step();
        [-1, 1].into_iter().find_map(|file_delta| {
            let source = target.offset(file_delta, source_rank_delta)?;
            (self.piece_at(source) == Some(Piece::new(self.side_to_move, PieceKind::Pawn)))
                .then_some(target.file())
        })
    }

    fn is_consistent(&self) -> bool {
        let mut pieces = [[0u64; 6]; 2];
        let mut colors = [0u64; 2];
        let mut occupied = 0;
        for square in Square::all() {
            if let Some(piece) = self.mailbox[square.index()] {
                let bit = attacks::bit(square);
                pieces[piece.color.index()][piece.kind.index()] |= bit;
                colors[piece.color.index()] |= bit;
                occupied |= bit;
            }
        }
        pieces == self.pieces
            && colors == self.color_occupancy
            && occupied == self.occupied
            && colors[0] & colors[1] == 0
            && zobrist::compute(self) == self.key
    }
}
