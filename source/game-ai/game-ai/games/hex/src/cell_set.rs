use crate::board::{Cell, MAX_CELLS};

const WORDS: usize = MAX_CELLS / 64;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct CellSet([u64; WORDS]);

impl CellSet {
    pub(crate) fn insert(&mut self, cell: Cell) {
        let index = usize::from(cell.index());
        self.0[index / 64] |= 1_u64 << (index % 64);
    }

    pub(crate) fn contains(self, cell: Cell) -> bool {
        let index = usize::from(cell.index());
        self.0[index / 64] & (1_u64 << (index % 64)) != 0
    }

    pub(crate) fn is_empty(self) -> bool {
        self.0.iter().all(|word| *word == 0)
    }

    pub(crate) fn count(self) -> u32 {
        self.0.iter().map(|word| word.count_ones()).sum()
    }

    pub(crate) fn intersects(self, other: Self) -> bool {
        self.0
            .iter()
            .zip(other.0)
            .any(|(left, right)| left & right != 0)
    }

    pub(crate) fn intersection(self, other: Self) -> Self {
        let mut intersection = Self::default();
        for ((output, left), right) in intersection.0.iter_mut().zip(self.0).zip(other.0) {
            *output = left & right;
        }
        intersection
    }

    pub(crate) fn union(self, other: Self) -> Self {
        let mut union = Self::default();
        for ((output, left), right) in union.0.iter_mut().zip(self.0).zip(other.0) {
            *output = left | right;
        }
        union
    }

    pub(crate) fn is_subset_of(self, other: Self) -> bool {
        self.0
            .iter()
            .zip(other.0)
            .all(|(left, right)| left & !right == 0)
    }
}
