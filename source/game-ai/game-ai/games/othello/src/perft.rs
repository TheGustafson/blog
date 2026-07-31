use crate::Position;
use crate::reference::ReferencePosition;

/// Counts legal move sequences to exactly `depth` plies with the bitboard core.
pub fn perft(position: &mut Position, depth: u8) -> u64 {
    if depth == 0 {
        return 1;
    }
    let mut nodes = 0;
    for mv in position.legal_moves() {
        let undo = position
            .make_move(mv)
            .expect("generated moves must be legal");
        nodes += perft(position, depth - 1);
        position.unmake_move(undo);
    }
    nodes
}

/// Runs the same traversal through the array-backed reference implementation.
pub fn reference_perft(position: &mut ReferencePosition, depth: u8) -> u64 {
    if depth == 0 {
        return 1;
    }
    let mut nodes = 0;
    for mv in position.legal_moves() {
        let undo = position
            .make_move(mv)
            .expect("generated moves must be legal");
        nodes += reference_perft(position, depth - 1);
        position.unmake_move(undo);
    }
    nodes
}
