use crate::board::{BoardSize, Cell, MAX_CELLS};
use crate::position::Color;

pub(crate) fn has_connection(bits: &[u64; 9], size: BoardSize, color: Color) -> bool {
    connection_path(bits, size, color).is_some()
}

pub(crate) fn connection_path(bits: &[u64; 9], size: BoardSize, color: Color) -> Option<Vec<Cell>> {
    let width = size.get();
    let mut seen = [false; MAX_CELLS];
    let mut previous = [u16::MAX; MAX_CELLS];
    let mut queue = [Cell::from_coords(0, 0); MAX_CELLS];
    let mut head = 0;
    let mut tail = 0;

    for edge in 0..width {
        let cell = match color {
            Color::Red => Cell::from_coords(edge, 0),
            Color::Blue => Cell::from_coords(0, edge),
        };
        if occupied(bits, cell) {
            seen[cell.index() as usize] = true;
            queue[tail] = cell;
            tail += 1;
        }
    }

    while head < tail {
        let cell = queue[head];
        head += 1;
        let reached = match color {
            Color::Red => cell.rank() + 1 == width,
            Color::Blue => cell.file() + 1 == width,
        };
        if reached {
            return Some(reconstruct(cell, &previous));
        }
        for neighbor in neighbors(cell, size).into_iter().flatten() {
            let index = neighbor.index() as usize;
            if occupied(bits, neighbor) && !seen[index] {
                seen[index] = true;
                previous[index] = cell.index();
                queue[tail] = neighbor;
                tail += 1;
            }
        }
    }
    None
}

fn reconstruct(mut cell: Cell, previous: &[u16; MAX_CELLS]) -> Vec<Cell> {
    let mut path = vec![cell];
    while previous[cell.index() as usize] != u16::MAX {
        let index = previous[cell.index() as usize];
        cell = Cell::from_index(index);
        path.push(cell);
    }
    path.reverse();
    path
}

pub(crate) fn neighbors(cell: Cell, size: BoardSize) -> [Option<Cell>; 6] {
    let file = i16::from(cell.file());
    let rank = i16::from(cell.rank());
    let limit = i16::from(size.get());
    [(1, 0), (-1, 0), (0, 1), (0, -1), (1, -1), (-1, 1)].map(|(file_delta, rank_delta)| {
        let next_file = file + file_delta;
        let next_rank = rank + rank_delta;
        (next_file >= 0 && next_rank >= 0 && next_file < limit && next_rank < limit)
            .then(|| Cell::from_coords(next_file as u8, next_rank as u8))
    })
}

pub(crate) fn occupied(bits: &[u64; 9], cell: Cell) -> bool {
    let index = cell.index() as usize;
    bits[index / 64] & (1_u64 << (index % 64)) != 0
}

pub(crate) fn insert(bits: &mut [u64; 9], cell: Cell) {
    let index = cell.index() as usize;
    bits[index / 64] |= 1_u64 << (index % 64);
}
