use crate::{Color, PieceKind, Position, Square};

const KNIGHT_DELTAS: [(i8, i8); 8] = [
    (1, 2),
    (2, 1),
    (2, -1),
    (1, -2),
    (-1, -2),
    (-2, -1),
    (-2, 1),
    (-1, 2),
];
const KING_DELTAS: [(i8, i8); 8] = [
    (0, 1),
    (1, 1),
    (1, 0),
    (1, -1),
    (0, -1),
    (-1, -1),
    (-1, 0),
    (-1, 1),
];
pub(crate) const BISHOP_DIRECTIONS: [(i8, i8); 4] = [(1, 1), (1, -1), (-1, -1), (-1, 1)];
pub(crate) const ROOK_DIRECTIONS: [(i8, i8); 4] = [(0, 1), (1, 0), (0, -1), (-1, 0)];

pub(crate) fn knight(square: Square) -> u64 {
    jumps(square, &KNIGHT_DELTAS)
}

pub(crate) fn king(square: Square) -> u64 {
    jumps(square, &KING_DELTAS)
}

pub(crate) fn pawn(square: Square, color: Color) -> u64 {
    let rank_delta = color.pawn_step();
    [(-1, rank_delta), (1, rank_delta)]
        .into_iter()
        .filter_map(|(file, rank)| square.offset(file, rank))
        .fold(0, |bits, target| bits | bit(target))
}

pub(crate) fn slider(square: Square, occupied: u64, directions: &[(i8, i8)]) -> u64 {
    let mut attacks = 0;
    for &(file_delta, rank_delta) in directions {
        let mut cursor = square;
        while let Some(target) = cursor.offset(file_delta, rank_delta) {
            attacks |= bit(target);
            if occupied & bit(target) != 0 {
                break;
            }
            cursor = target;
        }
    }
    attacks
}

pub(crate) fn square_is_attacked(position: &Position, square: Square, by: Color) -> bool {
    let square_bit = bit(square);
    for from in bits(position.pieces(by, PieceKind::Pawn)) {
        if pawn(from, by) & square_bit != 0 {
            return true;
        }
    }
    if knight(square) & position.pieces(by, PieceKind::Knight) != 0 {
        return true;
    }
    if king(square) & position.pieces(by, PieceKind::King) != 0 {
        return true;
    }
    let bishops = position.pieces(by, PieceKind::Bishop) | position.pieces(by, PieceKind::Queen);
    if slider(square, position.occupied(), &BISHOP_DIRECTIONS) & bishops != 0 {
        return true;
    }
    let rooks = position.pieces(by, PieceKind::Rook) | position.pieces(by, PieceKind::Queen);
    slider(square, position.occupied(), &ROOK_DIRECTIONS) & rooks != 0
}

pub(crate) const fn bit(square: Square) -> u64 {
    1u64 << square.index()
}

pub(crate) fn bits(mut value: u64) -> impl Iterator<Item = Square> {
    std::iter::from_fn(move || {
        if value == 0 {
            None
        } else {
            let square = Square::new(value.trailing_zeros() as u8);
            value &= value - 1;
            Some(square)
        }
    })
}

fn jumps(square: Square, deltas: &[(i8, i8)]) -> u64 {
    deltas
        .iter()
        .filter_map(|&(file, rank)| square.offset(file, rank))
        .fold(0, |bits, target| bits | bit(target))
}
