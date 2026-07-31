use crate::{Move, Position};

/// Counts legal move sequences to exactly `depth` plies.
pub fn perft(position: &mut Position, depth: u8) -> u64 {
    if depth == 0 {
        return 1;
    }
    let moves = position.legal_moves();
    if depth == 1 {
        return moves.len() as u64;
    }
    let mut nodes = 0;
    for mv in moves {
        let undo = position
            .make_unchecked(mv)
            .expect("legal move must be makeable");
        nodes += perft(position, depth - 1);
        position.unmake_move(undo);
    }
    nodes
}

/// Returns the per-root-move breakdown of [`perft`].
pub fn divide(position: &mut Position, depth: u8) -> Vec<(Move, u64)> {
    if depth == 0 {
        return Vec::new();
    }
    let mut rows = Vec::new();
    for mv in position.legal_moves() {
        let undo = position
            .make_unchecked(mv)
            .expect("legal move must be makeable");
        rows.push((mv, perft(position, depth - 1)));
        position.unmake_move(undo);
    }
    rows
}
