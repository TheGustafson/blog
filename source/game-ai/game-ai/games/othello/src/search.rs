use crate::{
    Evaluation, EvaluationProfile, GameResult, Move, MoveList, Position, Square, evaluate,
};

const INFINITY: i32 = 40_000;
const TERMINAL: i32 = 30_000;

/// Search score with exact results kept outside the heuristic band.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Score(i32);

impl Score {
    pub const fn raw(self) -> i32 {
        self.0
    }

    pub const fn kind(self) -> ScoreKind {
        if self.0 >= TERMINAL {
            ScoreKind::Win {
                margin: (self.0 - TERMINAL) as u8,
            }
        } else if self.0 <= -TERMINAL {
            ScoreKind::Loss {
                margin: (-self.0 - TERMINAL) as u8,
            }
        } else {
            ScoreKind::Estimate(self.0)
        }
    }

    const fn negated(self) -> Self {
        Self(-self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScoreKind {
    Win { margin: u8 },
    Draw,
    Loss { margin: u8 },
    Estimate(i32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SearchConfig {
    pub depth: u8,
    pub evaluator: EvaluationProfile,
    pub exact_endgame_empties: u8,
}

impl SearchConfig {
    pub const fn fixed_depth(depth: u8, evaluator: EvaluationProfile) -> Self {
        Self {
            depth,
            evaluator,
            exact_endgame_empties: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SearchStats {
    pub nodes: u64,
    pub leaves: u64,
    pub cutoffs: u64,
    pub passes: u64,
    pub exact_nodes: u64,
    pub max_ply: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    pub mv: Move,
    pub score: Score,
    pub nodes: u64,
    pub cutoffs: u64,
    pub flipped: u64,
}

/// Complete result of one search.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchReport {
    pub config: SearchConfig,
    pub best_move: Option<Move>,
    pub score: Score,
    pub exact: bool,
    pub principal_variation: Vec<Move>,
    pub candidates: Vec<Candidate>,
    pub evaluation: Evaluation,
    pub stats: SearchStats,
}

#[derive(Debug, Clone, Copy)]
struct Line {
    moves: [Move; 64],
    len: u8,
}

impl Default for Line {
    fn default() -> Self {
        Self {
            moves: [Move::Pass; 64],
            len: 0,
        }
    }
}

impl Line {
    fn prepend(mv: Move, child: Self) -> Self {
        let mut line = Self::default();
        line.moves[0] = mv;
        let child_len = usize::from(child.len).min(63);
        if child_len > 0 {
            line.moves[1..=child_len].copy_from_slice(&child.moves[..child_len]);
        }
        line.len = child_len as u8 + 1;
        line
    }

    fn to_vec(self) -> Vec<Move> {
        self.moves[..usize::from(self.len)].to_vec()
    }
}

#[derive(Debug, Clone, Copy)]
struct NodeResult {
    score: Score,
    line: Line,
}

struct Searcher<F: Fn() -> bool> {
    config: SearchConfig,
    stats: SearchStats,
    should_stop: F,
    stopped: bool,
}

impl<F: Fn() -> bool> Searcher<F> {
    fn stop_requested(&mut self) -> bool {
        if self.stopped || (self.should_stop)() {
            self.stopped = true;
        }
        self.stopped
    }

    fn node(
        &mut self,
        position: &mut Position,
        depth: u8,
        ply: u8,
        mut alpha: Score,
        beta: Score,
    ) -> Option<NodeResult> {
        if self.stop_requested() {
            return None;
        }
        self.stats.nodes += 1;
        self.stats.max_ply = self.stats.max_ply.max(ply);
        if position.empty_count() <= self.config.exact_endgame_empties {
            self.stats.exact_nodes += 1;
        }
        if let Some(score) = terminal_score(*position) {
            self.stats.leaves += 1;
            return Some(NodeResult {
                score,
                line: Line::default(),
            });
        }
        let exact = position.empty_count() <= self.config.exact_endgame_empties;
        if depth == 0 && !exact {
            self.stats.leaves += 1;
            return Some(NodeResult {
                score: Score(evaluate(*position, self.config.evaluator).total),
                line: Line::default(),
            });
        }

        let mut best = Score(-INFINITY);
        let mut best_move = None;
        let mut best_line = Line::default();
        for mv in ordered_moves(*position) {
            if mv == Move::Pass {
                self.stats.passes += 1;
            }
            let undo = position
                .make_move(mv)
                .expect("generated moves must be legal");
            let next_depth = if mv == Move::Pass {
                depth
            } else {
                depth.saturating_sub(1)
            };
            let child = self.node(
                position,
                next_depth,
                ply + 1,
                beta.negated(),
                alpha.negated(),
            );
            position.unmake_move(undo);
            let child = child?;
            let score = child.score.negated();
            if score > best || (score == best && prefer(mv, best_move)) {
                best = score;
                best_move = Some(mv);
                best_line = Line::prepend(mv, child.line);
            }
            alpha = alpha.max(score);
            if alpha >= beta {
                self.stats.cutoffs += 1;
                break;
            }
        }
        Some(NodeResult {
            score: best,
            line: best_line,
        })
    }
}

/// Searches `position` to the configured depth.
pub fn search(position: Position, config: SearchConfig) -> SearchReport {
    search_until(position, config, || false)
        .expect("a search with a constant false stop signal must complete")
}

/// Search a fixed depth while polling a cooperative stop signal at node entry.
/// A stopped partial result is discarded.
pub fn search_until<F: Fn() -> bool>(
    position: Position,
    config: SearchConfig,
    should_stop: F,
) -> Option<SearchReport> {
    let exact = position.empty_count() <= config.exact_endgame_empties;
    let evaluation = evaluate(position, config.evaluator);
    let mut searcher = Searcher {
        config,
        stats: SearchStats {
            nodes: 1,
            ..SearchStats::default()
        },
        should_stop,
        stopped: false,
    };
    if searcher.stop_requested() {
        return None;
    }
    if let Some(score) = terminal_score(position) {
        searcher.stats.leaves = 1;
        return Some(SearchReport {
            config,
            best_move: None,
            score,
            exact: true,
            principal_variation: Vec::new(),
            candidates: Vec::new(),
            evaluation,
            stats: searcher.stats,
        });
    }
    if config.depth == 0 && !exact {
        searcher.stats.leaves = 1;
        return Some(SearchReport {
            config,
            best_move: None,
            score: Score(evaluation.total),
            exact: false,
            principal_variation: Vec::new(),
            candidates: Vec::new(),
            evaluation,
            stats: searcher.stats,
        });
    }

    let mut best = Score(-INFINITY);
    let mut best_move = None;
    let mut best_line = Line::default();
    let mut candidates = Vec::new();
    for mv in ordered_moves(position) {
        if searcher.stop_requested() {
            return None;
        }
        if mv == Move::Pass {
            searcher.stats.passes += 1;
        }
        let before_nodes = searcher.stats.nodes;
        let before_cutoffs = searcher.stats.cutoffs;
        let flipped = mv.square().map_or(0, |square| position.flips_for(square));
        let mut child_position = position;
        child_position
            .make_move(mv)
            .expect("generated moves must be legal");
        let next_depth = if mv == Move::Pass {
            config.depth
        } else {
            config.depth.saturating_sub(1)
        };
        let child = searcher.node(
            &mut child_position,
            next_depth,
            1,
            Score(-INFINITY),
            Score(INFINITY),
        )?;
        let score = child.score.negated();
        candidates.push(Candidate {
            mv,
            score,
            nodes: searcher.stats.nodes - before_nodes,
            cutoffs: searcher.stats.cutoffs - before_cutoffs,
            flipped,
        });
        if score > best || (score == best && prefer(mv, best_move)) {
            best = score;
            best_move = Some(mv);
            best_line = Line::prepend(mv, child.line);
        }
    }

    Some(SearchReport {
        config,
        best_move,
        score: best,
        exact,
        principal_variation: best_line.to_vec(),
        candidates,
        evaluation,
        stats: searcher.stats,
    })
}

fn terminal_score(position: Position) -> Option<Score> {
    match position.result() {
        GameResult::Ongoing => None,
        GameResult::Draw { .. } => Some(Score(0)),
        GameResult::Win {
            winner,
            black,
            white,
        } => {
            let margin = i32::from(black.abs_diff(white));
            if winner == position.side_to_move() {
                Some(Score(TERMINAL + margin))
            } else {
                Some(Score(-TERMINAL - margin))
            }
        }
    }
}

fn ordered_moves(position: Position) -> MoveList {
    const CORNERS: [u8; 4] = [0, 7, 56, 63];
    let legal = position.legal_moves();
    if legal.as_slice() == [Move::Pass] {
        return legal;
    }
    let mut ordered = MoveList::default();
    for index in CORNERS {
        let mv = Move::Place(Square::new(index));
        if legal.as_slice().contains(&mv) {
            ordered.push(mv);
        }
    }
    for mv in legal {
        if !ordered.as_slice().contains(&mv) {
            ordered.push(mv);
        }
    }
    ordered
}

fn prefer(candidate: Move, current: Option<Move>) -> bool {
    current.is_none_or(|mv| candidate < mv)
}
