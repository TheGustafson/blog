use crate::mv::Move;
use crate::position::{GameResult, Position};
use crate::search::{Outcome, Solved};
use std::collections::HashSet;

const UNKNOWN: i8 = 2;
const NO_MOVE: u8 = u8::MAX;

#[derive(Debug, Clone, Copy)]
struct Entry {
    outcome: i8,
    distance: u8,
    best: u8,
}

impl Entry {
    const UNKNOWN: Self = Self {
        outcome: UNKNOWN,
        distance: 0,
        best: NO_MOVE,
    };
}

/// An exact table for every position reachable from the empty board.
#[derive(Debug, Clone)]
pub struct Tablebase {
    entries: Box<[Entry]>,
    reachable: usize,
    canonical: usize,
}

impl Default for Tablebase {
    fn default() -> Self {
        Self::build()
    }
}

impl Tablebase {
    pub fn build() -> Self {
        let mut tablebase = Self {
            entries: vec![Entry::UNKNOWN; Position::KEY_SPACE].into_boxed_slice(),
            reachable: 0,
            canonical: 0,
        };
        let mut canonical_keys = HashSet::new();
        tablebase.solve(Position::start(), &mut canonical_keys);
        tablebase.canonical = canonical_keys.len();
        tablebase
    }

    pub fn reachable_positions(&self) -> usize {
        self.reachable
    }

    pub fn canonical_positions(&self) -> usize {
        self.canonical
    }

    pub(crate) fn value(&self, position: Position) -> Solved {
        let entry = self.entries[position.key()];
        if entry.outcome == UNKNOWN {
            // The public protocol reaches positions by replaying legal moves
            // from start, so this is an invariant rather than user input.
            panic!("position is not reachable from the empty board");
        }
        Solved {
            outcome: decode_outcome(entry.outcome),
            distance: entry.distance,
        }
    }

    /// Returns the optimal move, or `None` when `position` is terminal.
    ///
    /// # Panics
    ///
    /// Panics if `position` is not reachable by legal play from the empty
    /// board.
    pub fn best_move(&self, position: Position) -> Option<Move> {
        let entry = self.entries[position.key()];
        assert!(
            entry.outcome != UNKNOWN,
            "position is not reachable from the empty board"
        );
        (entry.best != NO_MOVE).then(|| Move::new(crate::Square::new(entry.best)))
    }

    fn solve(&mut self, mut position: Position, canonical_keys: &mut HashSet<usize>) -> Solved {
        let key = position.key();
        let cached = self.entries[key];
        if cached.outcome != UNKNOWN {
            return Solved {
                outcome: decode_outcome(cached.outcome),
                distance: cached.distance,
            };
        }
        self.reachable += 1;
        canonical_keys.insert(position.canonical_key());

        if position.result() != GameResult::Ongoing {
            let outcome = match position.result() {
                GameResult::Draw => Outcome::Draw,
                GameResult::Win(winner) if winner == position.side_to_move() => Outcome::Win,
                GameResult::Win(_) => Outcome::Loss,
                GameResult::Ongoing => unreachable!(),
            };
            self.entries[key] = Entry {
                outcome: outcome.as_i8(),
                distance: 0,
                best: NO_MOVE,
            };
            return Solved {
                outcome,
                distance: 0,
            };
        }

        let moves: Vec<_> = position.legal_moves().collect();
        let mut best: Option<(Move, Solved)> = None;
        for mv in moves {
            position.make_move(mv).expect("generated move is legal");
            let child = self.solve(position, canonical_keys);
            position.unmake_move(mv);
            let candidate = Solved {
                outcome: child.outcome.negate(),
                distance: child.distance.saturating_add(1),
            };
            if best.is_none_or(|(_, incumbent)| better(candidate, incumbent)) {
                best = Some((mv, candidate));
            }
        }

        let (best_move, solved) = best.expect("ongoing position has legal moves");
        self.entries[key] = Entry {
            outcome: solved.outcome.as_i8(),
            distance: solved.distance,
            best: best_move.square().index() as u8,
        };
        solved
    }
}

fn better(candidate: Solved, incumbent: Solved) -> bool {
    if candidate.outcome.as_i8() != incumbent.outcome.as_i8() {
        return candidate.outcome.as_i8() > incumbent.outcome.as_i8();
    }
    match candidate.outcome {
        Outcome::Win | Outcome::Draw => candidate.distance < incumbent.distance,
        Outcome::Loss => candidate.distance > incumbent.distance,
    }
}

fn decode_outcome(value: i8) -> Outcome {
    match value {
        -1 => Outcome::Loss,
        0 => Outcome::Draw,
        1 => Outcome::Win,
        _ => panic!("invalid tablebase outcome"),
    }
}
