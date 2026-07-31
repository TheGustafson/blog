use crate::{Color, Piece, Square};

const SEED: u64 = 0x6a09_e667_f3bc_c909;

pub(crate) const fn piece(piece: Piece, square: Square) -> u64 {
    let piece_index = piece.color.index() * 6 + piece.kind.index();
    splitmix64(SEED ^ ((piece_index as u64) << 7) ^ square.index() as u64)
}

pub(crate) const fn side() -> u64 {
    splitmix64(SEED ^ 0x1000)
}

pub(crate) const fn castling(rights: u8) -> u64 {
    splitmix64(SEED ^ 0x2000 ^ rights as u64)
}

pub(crate) const fn en_passant(file: u8) -> u64 {
    splitmix64(SEED ^ 0x3000 ^ file as u64)
}

const fn splitmix64(mut value: u64) -> u64 {
    value = value.wrapping_add(0x9e37_79b9_7f4a_7c15);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

pub(crate) fn compute(position: &crate::Position) -> u64 {
    let mut key = castling(position.castling_rights().bits());
    if position.side_to_move() == Color::Black {
        key ^= side();
    }
    if let Some(file) = position.hashed_en_passant_file() {
        key ^= en_passant(file);
    }
    for square in Square::all() {
        if let Some(piece_value) = position.piece_at(square) {
            key ^= piece(piece_value, square);
        }
    }
    key
}
