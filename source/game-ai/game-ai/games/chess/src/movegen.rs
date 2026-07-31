use crate::attacks::{self, BISHOP_DIRECTIONS, ROOK_DIRECTIONS};
use crate::{CastlingRights, Color, Move, MoveKind, MoveList, Piece, PieceKind, Position, Square};

pub(crate) fn pseudo_legal(position: &Position) -> MoveList {
    let mut moves = MoveList::default();
    let us = position.side_to_move();
    let own = position.occupancy(us);
    let enemy_king = position.pieces(us.other(), PieceKind::King);
    let available = !own & !enemy_king;

    pawns(position, &mut moves);
    pieces(
        position,
        &mut moves,
        PieceKind::Knight,
        available,
        |square, _| attacks::knight(square),
    );
    pieces(
        position,
        &mut moves,
        PieceKind::Bishop,
        available,
        |square, occupied| attacks::slider(square, occupied, &BISHOP_DIRECTIONS),
    );
    pieces(
        position,
        &mut moves,
        PieceKind::Rook,
        available,
        |square, occupied| attacks::slider(square, occupied, &ROOK_DIRECTIONS),
    );
    pieces(
        position,
        &mut moves,
        PieceKind::Queen,
        available,
        |square, occupied| {
            attacks::slider(square, occupied, &BISHOP_DIRECTIONS)
                | attacks::slider(square, occupied, &ROOK_DIRECTIONS)
        },
    );
    pieces(
        position,
        &mut moves,
        PieceKind::King,
        available,
        |square, _| attacks::king(square),
    );
    castles(position, &mut moves);
    moves
}

fn pawns(position: &Position, moves: &mut MoveList) {
    let color = position.side_to_move();
    let enemy = position.occupancy(color.other());
    for from in attacks::bits(position.pieces(color, PieceKind::Pawn)) {
        if let Some(to) = from.offset(0, color.pawn_step()) {
            if position.piece_at(to).is_none() {
                push_pawn_move(moves, from, to, color);
                if from.rank() == color.pawn_rank() {
                    if let Some(double) = from.offset(0, color.pawn_step() * 2) {
                        if position.piece_at(double).is_none() {
                            moves.push(Move::normal(from, double));
                        }
                    }
                }
            }
        }
        for file_delta in [-1, 1] {
            let Some(to) = from.offset(file_delta, color.pawn_step()) else {
                continue;
            };
            if enemy & attacks::bit(to) != 0 {
                push_pawn_move(moves, from, to, color);
            } else if position.en_passant() == Some(to) {
                moves.push(Move::new(from, to, MoveKind::EnPassant));
            }
        }
    }
}

fn push_pawn_move(moves: &mut MoveList, from: Square, to: Square, color: Color) {
    if to.rank() == color.promotion_rank() {
        for kind in PieceKind::PROMOTIONS {
            moves.push(Move::new(from, to, MoveKind::Promotion(kind)));
        }
    } else {
        moves.push(Move::normal(from, to));
    }
}

fn pieces(
    position: &Position,
    moves: &mut MoveList,
    kind: PieceKind,
    available: u64,
    attacks_for: impl Fn(Square, u64) -> u64,
) {
    let color = position.side_to_move();
    for from in attacks::bits(position.pieces(color, kind)) {
        let targets = attacks_for(from, position.occupied()) & available;
        for to in attacks::bits(targets) {
            moves.push(Move::normal(from, to));
        }
    }
}

fn castles(position: &Position, moves: &mut MoveList) {
    let color = position.side_to_move();
    let rank = color.home_rank();
    let king_from = Square::from_file_rank(4, rank);
    if position.piece_at(king_from) != Some(Piece::new(color, PieceKind::King))
        || position.in_check(color)
    {
        return;
    }
    let opponent = color.other();
    let (king_flag, queen_flag) = match color {
        Color::White => (CastlingRights::WHITE_KING, CastlingRights::WHITE_QUEEN),
        Color::Black => (CastlingRights::BLACK_KING, CastlingRights::BLACK_QUEEN),
    };
    if position.castling_rights().has(king_flag) {
        let f = Square::from_file_rank(5, rank);
        let g = Square::from_file_rank(6, rank);
        let rook = Square::from_file_rank(7, rank);
        if position.piece_at(f).is_none()
            && position.piece_at(g).is_none()
            && position.piece_at(rook) == Some(Piece::new(color, PieceKind::Rook))
            && !attacks::square_is_attacked(position, f, opponent)
            && !attacks::square_is_attacked(position, g, opponent)
        {
            moves.push(Move::new(king_from, g, MoveKind::CastleKingSide));
        }
    }
    if position.castling_rights().has(queen_flag) {
        let b = Square::from_file_rank(1, rank);
        let c = Square::from_file_rank(2, rank);
        let d = Square::from_file_rank(3, rank);
        let rook = Square::from_file_rank(0, rank);
        if position.piece_at(b).is_none()
            && position.piece_at(c).is_none()
            && position.piece_at(d).is_none()
            && position.piece_at(rook) == Some(Piece::new(color, PieceKind::Rook))
            && !attacks::square_is_attacked(position, d, opponent)
            && !attacks::square_is_attacked(position, c, opponent)
        {
            moves.push(Move::new(king_from, c, MoveKind::CastleQueenSide));
        }
    }
}
