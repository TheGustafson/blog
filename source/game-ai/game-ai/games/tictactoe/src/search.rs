use crate::mv::Move;
use crate::position::{GameResult, Position};
use crate::tablebase::Tablebase;
use std::fmt;
use std::str::FromStr;

/// Exact game-theoretic result from the current side's perspective.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Loss = -1,
    Draw = 0,
    Win = 1,
}

impl Outcome {
    pub const fn negate(self) -> Self {
        match self {
            Self::Loss => Self::Win,
            Self::Draw => Self::Draw,
            Self::Win => Self::Loss,
        }
    }

    pub const fn as_i8(self) -> i8 {
        self as i8
    }
}

impl fmt::Display for Outcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Loss => write!(f, "loss"),
            Self::Draw => write!(f, "draw"),
            Self::Win => write!(f, "win"),
        }
    }
}

/// Exact solving method used by [`search`].
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum Algorithm {
    Plain,
    Memo,
    Symmetry,
    #[default]
    Tablebase,
}

impl Algorithm {
    pub const ALL: [Self; 4] = [Self::Plain, Self::Memo, Self::Symmetry, Self::Tablebase];

    pub const fn name(self) -> &'static str {
        match self {
            Self::Plain => "plain",
            Self::Memo => "memo",
            Self::Symmetry => "symmetry",
            Self::Tablebase => "tablebase",
        }
    }
}

impl fmt::Display for Algorithm {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

impl FromStr for Algorithm {
    type Err = &'static str;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.to_ascii_lowercase().as_str() {
            "plain" | "negamax" => Ok(Self::Plain),
            "memo" | "memoized" => Ok(Self::Memo),
            "symmetry" | "canonical" => Ok(Self::Symmetry),
            "tablebase" | "perfect" => Ok(Self::Tablebase),
            _ => Err("algorithm must be plain, memo, symmetry, or tablebase"),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SearchStats {
    pub nodes: u64,
    pub cache_hits: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    pub mv: Move,
    pub outcome: Outcome,
    pub distance: u8,
    pub nodes: u64,
}

/// Complete exact-search result for a position.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchReport {
    pub algorithm: Algorithm,
    pub best_move: Option<Move>,
    pub outcome: Outcome,
    pub distance: u8,
    pub stats: SearchStats,
    pub candidates: Vec<Candidate>,
    pub pv: Vec<Move>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Solved {
    pub outcome: Outcome,
    pub distance: u8,
}

impl Solved {
    fn from_child(child: Self) -> Self {
        Self {
            outcome: child.outcome.negate(),
            distance: child.distance + 1,
        }
    }
}

/// Solves `position` exactly with the selected `algorithm`.
///
/// Every algorithm returns the same game-theoretic answer; they differ only
/// in the amount of repeated work they avoid.
pub fn search(position: Position, algorithm: Algorithm, tablebase: &Tablebase) -> SearchReport {
    if position.result() != GameResult::Ongoing {
        return SearchReport {
            algorithm,
            best_move: None,
            outcome: terminal_value(position),
            distance: 0,
            stats: SearchStats {
                nodes: 1,
                cache_hits: 0,
            },
            candidates: Vec::new(),
            pv: Vec::new(),
        };
    }

    let mut stats = SearchStats::default();
    let mut cache = vec![None; Position::KEY_SPACE];
    let mut candidates = Vec::new();
    let mut best: Option<(Move, Solved)> = None;

    for mv in position.legal_moves() {
        let before = stats.nodes;
        let mut child = position;
        child.make_move(mv).expect("legal move must apply");
        let solved = match algorithm {
            Algorithm::Plain => Solved::from_child(negamax_plain(&mut child, &mut stats)),
            Algorithm::Memo => {
                Solved::from_child(negamax_cached(&mut child, &mut cache, false, &mut stats))
            }
            Algorithm::Symmetry => {
                Solved::from_child(negamax_cached(&mut child, &mut cache, true, &mut stats))
            }
            Algorithm::Tablebase => {
                stats.nodes += 1;
                Solved::from_child(tablebase.value(child))
            }
        };
        candidates.push(Candidate {
            mv,
            outcome: solved.outcome,
            distance: solved.distance,
            nodes: stats.nodes - before,
        });
        if best.is_none_or(|(_, incumbent)| is_better(solved, incumbent)) {
            best = Some((mv, solved));
        }
    }

    let (best_move, solved) = best.expect("ongoing position has a legal move");
    let pv = extract_pv(position, algorithm, tablebase);
    SearchReport {
        algorithm,
        best_move: Some(best_move),
        outcome: solved.outcome,
        distance: solved.distance,
        stats,
        candidates,
        pv,
    }
}

/// Counts leaf positions reached at exactly `depth` plies or at game end.
pub fn perft(position: &mut Position, depth: u8) -> u64 {
    if depth == 0 || position.result() != GameResult::Ongoing {
        return 1;
    }
    let moves: Vec<_> = position.legal_moves().collect();
    moves
        .into_iter()
        .map(|mv| {
            position.make_move(mv).expect("generated move is legal");
            let nodes = perft(position, depth - 1);
            position.unmake_move(mv);
            nodes
        })
        .sum()
}

fn negamax_plain(position: &mut Position, stats: &mut SearchStats) -> Solved {
    stats.nodes += 1;
    if position.result() != GameResult::Ongoing {
        return Solved {
            outcome: terminal_value(*position),
            distance: 0,
        };
    }

    let moves: Vec<_> = position.legal_moves().collect();
    let mut best: Option<Solved> = None;
    for mv in moves {
        position.make_move(mv).expect("generated move is legal");
        let candidate = Solved::from_child(negamax_plain(position, stats));
        position.unmake_move(mv);
        if best.is_none_or(|incumbent| is_better(candidate, incumbent)) {
            best = Some(candidate);
        }
    }
    best.expect("ongoing position has a legal move")
}

fn negamax_cached(
    position: &mut Position,
    cache: &mut [Option<Solved>],
    use_symmetry: bool,
    stats: &mut SearchStats,
) -> Solved {
    stats.nodes += 1;
    let key = if use_symmetry {
        position.canonical_key()
    } else {
        position.key()
    };
    if let Some(solved) = cache[key] {
        stats.cache_hits += 1;
        return solved;
    }
    if position.result() != GameResult::Ongoing {
        let solved = Solved {
            outcome: terminal_value(*position),
            distance: 0,
        };
        cache[key] = Some(solved);
        return solved;
    }

    let moves: Vec<_> = position.legal_moves().collect();
    let mut best: Option<Solved> = None;
    for mv in moves {
        position.make_move(mv).expect("generated move is legal");
        let candidate = Solved::from_child(negamax_cached(position, cache, use_symmetry, stats));
        position.unmake_move(mv);
        if best.is_none_or(|incumbent| is_better(candidate, incumbent)) {
            best = Some(candidate);
        }
    }
    let solved = best.expect("ongoing position has a legal move");
    cache[key] = Some(solved);
    solved
}

fn terminal_value(position: Position) -> Outcome {
    match position.result() {
        GameResult::Draw => Outcome::Draw,
        GameResult::Win(winner) if winner == position.side_to_move() => Outcome::Win,
        GameResult::Win(_) => Outcome::Loss,
        GameResult::Ongoing => unreachable!("terminal_value called on an ongoing position"),
    }
}

fn is_better(candidate: Solved, incumbent: Solved) -> bool {
    match candidate.outcome.as_i8().cmp(&incumbent.outcome.as_i8()) {
        std::cmp::Ordering::Greater => true,
        std::cmp::Ordering::Less => false,
        std::cmp::Ordering::Equal => match candidate.outcome {
            Outcome::Win => candidate.distance < incumbent.distance,
            Outcome::Draw => candidate.distance < incumbent.distance,
            Outcome::Loss => candidate.distance > incumbent.distance,
        },
    }
}

fn extract_pv(mut position: Position, algorithm: Algorithm, tablebase: &Tablebase) -> Vec<Move> {
    let mut pv = Vec::new();
    while position.result() == GameResult::Ongoing {
        let mut best: Option<(Move, Solved)> = None;
        for mv in position.legal_moves() {
            let mut child = position;
            child.make_move(mv).expect("legal move must apply");
            let child_value = match algorithm {
                Algorithm::Tablebase => tablebase.value(child),
                _ => {
                    let mut ignored = SearchStats::default();
                    let mut cache = vec![None; Position::KEY_SPACE];
                    match algorithm {
                        Algorithm::Plain => negamax_plain(&mut child, &mut ignored),
                        Algorithm::Memo => {
                            negamax_cached(&mut child, &mut cache, false, &mut ignored)
                        }
                        Algorithm::Symmetry => {
                            negamax_cached(&mut child, &mut cache, true, &mut ignored)
                        }
                        Algorithm::Tablebase => unreachable!(),
                    }
                }
            };
            let solved = Solved::from_child(child_value);
            if best.is_none_or(|(_, incumbent)| is_better(solved, incumbent)) {
                best = Some((mv, solved));
            }
        }
        let Some((mv, _)) = best else {
            break;
        };
        position.make_move(mv).expect("PV move must apply");
        pv.push(mv);
    }
    pv
}
