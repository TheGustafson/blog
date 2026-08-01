use crate::{Column, GameResult, Move, MoveList, Position, Side};
use std::fmt;
use std::str::FromStr;

const INFINITY: i16 = 31_000;
const FORCED_SCORE: i16 = 30_000;
const MAX_GAME_PLIES: i16 = 42;
const HEURISTIC_LIMIT: i16 = 4_000;
const DEFAULT_TT_ENTRIES: usize = 1 << 16;

/// Controlled configurations used by search regression tests.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Algorithm {
    PlainNegamax,
    AlphaBeta,
    OrderedAlphaBeta,
    #[default]
    TranspositionTable,
}

impl Algorithm {
    pub const ALL: [Self; 4] = [
        Self::PlainNegamax,
        Self::AlphaBeta,
        Self::OrderedAlphaBeta,
        Self::TranspositionTable,
    ];

    pub const fn name(self) -> &'static str {
        match self {
            Self::PlainNegamax => "plain",
            Self::AlphaBeta => "alpha-beta",
            Self::OrderedAlphaBeta => "ordered",
            Self::TranspositionTable => "tt",
        }
    }

    const fn uses_alpha_beta(self) -> bool {
        !matches!(self, Self::PlainNegamax)
    }

    const fn uses_center_order(self) -> bool {
        matches!(self, Self::OrderedAlphaBeta | Self::TranspositionTable)
    }

    const fn uses_table(self) -> bool {
        matches!(self, Self::TranspositionTable)
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
            "plain" | "negamax" => Ok(Self::PlainNegamax),
            "alpha-beta" | "alphabeta" | "alpha_beta" => Ok(Self::AlphaBeta),
            "ordered" | "ordered-alpha-beta" | "ordered_alphabeta" => Ok(Self::OrderedAlphaBeta),
            "tt" | "transposition" | "transposition-table" => Ok(Self::TranspositionTable),
            _ => Err("algorithm must be plain, alpha-beta, ordered, or tt"),
        }
    }
}

/// A raw search score with terminal results deliberately outside the heuristic
/// band. The type prevents an evaluation estimate from masquerading as a
/// proven win or loss.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Score(i16);

impl Score {
    pub const fn raw(self) -> i16 {
        self.0
    }

    pub const fn kind(self) -> ScoreKind {
        if self.0 >= FORCED_SCORE - MAX_GAME_PLIES {
            ScoreKind::ForcedWin {
                plies: (FORCED_SCORE - self.0) as u8,
            }
        } else if self.0 <= -FORCED_SCORE + MAX_GAME_PLIES {
            ScoreKind::ForcedLoss {
                plies: (FORCED_SCORE + self.0) as u8,
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
    ForcedWin { plies: u8 },
    ForcedLoss { plies: u8 },
    Estimate(i16),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SearchLimits {
    pub depth: u8,
    pub nodes: Option<u64>,
}

impl SearchLimits {
    pub const fn depth(depth: u8) -> Self {
        Self { depth, nodes: None }
    }

    pub const fn with_nodes(depth: u8, nodes: u64) -> Self {
        Self {
            depth,
            nodes: Some(nodes),
        }
    }
}

/// A named depth and cumulative node budget for interactive play.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SearchPreset {
    pub name: &'static str,
    pub limits: SearchLimits,
}

/// The built-in strength ladder, ordered from least to most search work.
pub const SEARCH_PRESETS: [SearchPreset; 6] = [
    SearchPreset {
        name: "beginner",
        limits: SearchLimits::with_nodes(3, 250),
    },
    SearchPreset {
        name: "easy",
        limits: SearchLimits::with_nodes(5, 2_000),
    },
    SearchPreset {
        name: "medium",
        limits: SearchLimits::with_nodes(7, 10_000),
    },
    SearchPreset {
        name: "hard",
        limits: SearchLimits::with_nodes(9, 50_000),
    },
    SearchPreset {
        name: "expert",
        limits: SearchLimits::with_nodes(12, 200_000),
    },
    SearchPreset {
        name: "maximum",
        limits: SearchLimits::with_nodes(42, 750_000),
    },
];

/// Finds a built-in strength preset by its lowercase name.
pub fn search_preset(name: &str) -> Option<SearchPreset> {
    SEARCH_PRESETS
        .iter()
        .copied()
        .find(|preset| preset.name == name)
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SearchStats {
    pub nodes: u64,
    pub leaves: u64,
    pub cutoffs: u64,
    pub tt_probes: u64,
    pub tt_hits: u64,
    pub tt_stores: u64,
    pub max_ply: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RootBranch {
    pub mv: Move,
    pub score: Score,
    pub nodes: u64,
    pub cutoffs: u64,
    pub completed: bool,
}

/// Complete result of one fixed-depth search attempt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchReport {
    pub algorithm: Algorithm,
    pub requested_depth: u8,
    pub completed: bool,
    pub best_move: Option<Move>,
    pub score: Score,
    pub principal_variation: Vec<Move>,
    pub root_branches: Vec<RootBranch>,
    pub stats: SearchStats,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IterationSummary {
    pub depth: u8,
    pub completed: bool,
    pub best_move: Option<Move>,
    pub score: Score,
    pub nodes: u64,
    pub cutoffs: u64,
    pub tt_hits: u64,
}

/// Result of iterative deepening under one cumulative budget.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IterativeSearchReport {
    pub result: SearchReport,
    pub requested_depth: u8,
    pub completed_depth: u8,
    pub completed: bool,
    pub total_nodes: u64,
    pub iterations: Vec<IterationSummary>,
}

#[derive(Debug, Clone, Copy)]
struct Line {
    moves: [Move; 42],
    len: u8,
}

impl Default for Line {
    fn default() -> Self {
        Self {
            moves: [Move::new(Column::new(0)); 42],
            len: 0,
        }
    }
}

impl Line {
    fn prepend(mv: Move, child: Self) -> Self {
        let mut line = Self::default();
        line.moves[0] = mv;
        let child_len = usize::from(child.len).min(line.moves.len() - 1);
        if child_len > 0 {
            line.moves[1..=child_len].copy_from_slice(&child.moves[..child_len]);
        }
        line.len = child_len as u8 + 1;
        line
    }

    fn to_vec(self) -> Vec<Move> {
        self.moves[..usize::from(self.len)].to_vec()
    }

    fn push(&mut self, mv: Move) -> bool {
        let index = usize::from(self.len);
        if index == self.moves.len() {
            return false;
        }
        self.moves[index] = mv;
        self.len += 1;
        true
    }
}

#[derive(Debug, Clone, Copy)]
struct NodeResult {
    score: Score,
    line: Line,
    stopped: bool,
}

impl NodeResult {
    const fn leaf(score: Score) -> Self {
        Self {
            score,
            line: Line {
                moves: [Move::new(Column::new(0)); 42],
                len: 0,
            },
            stopped: false,
        }
    }

    const fn stopped(score: Score) -> Self {
        Self {
            score,
            line: Line {
                moves: [Move::new(Column::new(0)); 42],
                len: 0,
            },
            stopped: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Bound {
    Exact,
    Lower,
    Upper,
}

#[derive(Debug, Clone, Copy)]
struct TableEntry {
    key: u64,
    depth: u8,
    score: Score,
    bound: Bound,
    best_move: Option<Move>,
}

struct TranspositionTable {
    entries: Vec<Option<TableEntry>>,
}

impl TranspositionTable {
    fn new(entries: usize) -> Self {
        debug_assert!(entries.is_power_of_two());
        Self {
            entries: vec![None; entries],
        }
    }

    fn probe(&self, key: u64) -> Option<TableEntry> {
        self.entries[self.index(key)].filter(|entry| entry.key == key)
    }

    fn store(&mut self, entry: TableEntry) -> bool {
        let index = self.index(entry.key);
        let replace =
            self.entries[index].is_none_or(|old| old.key != entry.key || entry.depth >= old.depth);
        if replace {
            self.entries[index] = Some(entry);
        }
        replace
    }

    fn index(&self, key: u64) -> usize {
        let mixed = key ^ (key >> 23) ^ (key >> 41);
        mixed as usize & (self.entries.len() - 1)
    }
}

struct Searcher {
    algorithm: Algorithm,
    limits: SearchLimits,
    stats: SearchStats,
    table: Option<TranspositionTable>,
}

impl Searcher {
    fn new(algorithm: Algorithm, limits: SearchLimits) -> Self {
        Self {
            algorithm,
            limits,
            stats: SearchStats::default(),
            table: algorithm
                .uses_table()
                .then(|| TranspositionTable::new(DEFAULT_TT_ENTRIES)),
        }
    }

    fn enter_node(&mut self, ply: u8) -> bool {
        if self
            .limits
            .nodes
            .is_some_and(|limit| self.stats.nodes >= limit)
        {
            return false;
        }
        self.stats.nodes += 1;
        self.stats.max_ply = self.stats.max_ply.max(ply);
        true
    }

    fn begin_iteration(&mut self, limits: SearchLimits) {
        self.limits = limits;
        self.stats = SearchStats::default();
    }

    fn table_line(&self, mut position: Position, depth: u8) -> Line {
        let Some(table) = &self.table else {
            return Line::default();
        };
        let mut line = Line::default();
        for _ in 0..depth {
            if position.result() != GameResult::Ongoing {
                break;
            }
            let Some(mv) = table
                .probe(position.key())
                .and_then(|entry| entry.best_move)
            else {
                break;
            };
            if !position.can_play(mv.column()) || !line.push(mv) {
                break;
            }
            position
                .make_move(mv)
                .expect("a stored table move must remain legal");
        }
        line
    }

    fn node(
        &mut self,
        position: &mut Position,
        depth: u8,
        ply: u8,
        mut alpha: Score,
        mut beta: Score,
    ) -> NodeResult {
        if !self.enter_node(ply) {
            return NodeResult::stopped(evaluate(*position));
        }
        if let Some(score) = terminal_score(*position, ply) {
            self.stats.leaves += 1;
            return NodeResult::leaf(score);
        }
        if depth == 0 {
            self.stats.leaves += 1;
            return NodeResult::leaf(evaluate(*position));
        }

        if !self.algorithm.uses_alpha_beta() {
            return self.plain_node(position, depth, ply);
        }

        let original_alpha = alpha;
        let original_beta = beta;
        let mut table_move = None;
        if self.algorithm.uses_table() {
            self.stats.tt_probes += 1;
            if let Some(entry) = self
                .table
                .as_ref()
                .and_then(|table| table.probe(position.key()))
            {
                self.stats.tt_hits += 1;
                table_move = entry.best_move;
                if entry.depth >= depth {
                    match entry.bound {
                        Bound::Exact => {
                            return NodeResult {
                                score: entry.score,
                                line: self.table_line(*position, depth),
                                stopped: false,
                            };
                        }
                        Bound::Lower => alpha = alpha.max(entry.score),
                        Bound::Upper => beta = beta.min(entry.score),
                    }
                    if alpha >= beta {
                        self.stats.cutoffs += 1;
                        return NodeResult {
                            score: entry.score,
                            line: self.table_line(*position, depth),
                            stopped: false,
                        };
                    }
                }
            }
        }

        let mut best = Score(-INFINITY);
        let mut best_move = None;
        let mut best_line = Line::default();
        for mv in ordered_moves(*position, self.algorithm, table_move) {
            let undo = position
                .make_move(mv)
                .expect("generated moves must be legal");
            let child = self.node(
                position,
                depth - 1,
                ply + 1,
                beta.negated(),
                alpha.negated(),
            );
            position.unmake_move(undo);
            let score = child.score.negated();
            if score > best || (score == best && prefer(mv, best_move)) {
                best = score;
                best_move = Some(mv);
                best_line = Line::prepend(mv, child.line);
            }
            if child.stopped {
                return NodeResult {
                    score: best,
                    line: best_line,
                    stopped: true,
                };
            }
            alpha = alpha.max(score);
            if alpha >= beta {
                self.stats.cutoffs += 1;
                break;
            }
        }

        if self.algorithm.uses_table() {
            let bound = if best <= original_alpha {
                Bound::Upper
            } else if best >= original_beta {
                Bound::Lower
            } else {
                Bound::Exact
            };
            let entry = TableEntry {
                key: position.key(),
                depth,
                score: best,
                bound,
                best_move,
            };
            if self
                .table
                .as_mut()
                .expect("table algorithm owns a table")
                .store(entry)
            {
                self.stats.tt_stores += 1;
            }
        }

        NodeResult {
            score: best,
            line: best_line,
            stopped: false,
        }
    }

    fn plain_node(&mut self, position: &mut Position, depth: u8, ply: u8) -> NodeResult {
        let mut best = Score(-INFINITY);
        let mut best_move = None;
        let mut best_line = Line::default();
        for mv in ordered_moves(*position, self.algorithm, None) {
            let undo = position
                .make_move(mv)
                .expect("generated moves must be legal");
            let child = self.node(
                position,
                depth - 1,
                ply + 1,
                Score(-INFINITY),
                Score(INFINITY),
            );
            position.unmake_move(undo);
            let score = child.score.negated();
            if score > best || (score == best && prefer(mv, best_move)) {
                best = score;
                best_move = Some(mv);
                best_line = Line::prepend(mv, child.line);
            }
            if child.stopped {
                return NodeResult {
                    score: best,
                    line: best_line,
                    stopped: true,
                };
            }
        }
        NodeResult {
            score: best,
            line: best_line,
            stopped: false,
        }
    }
}

/// Search one fixed depth. All algorithms use the same terminal scores and
/// frontier evaluator, so changing `algorithm` isolates search efficiency.
pub fn search(position: Position, algorithm: Algorithm, limits: SearchLimits) -> SearchReport {
    let mut searcher = Searcher::new(algorithm, limits);
    search_with(&mut searcher, position, limits, None)
}

/// Iteratively deepen while preserving the table and the last completed root
/// move. A cumulative node budget is split exactly across iterations. If an
/// iteration is interrupted, callers receive the prior complete result.
pub fn iterative_search(
    position: Position,
    algorithm: Algorithm,
    limits: SearchLimits,
) -> IterativeSearchReport {
    if limits.depth == 0 {
        let result = search(position, algorithm, limits);
        return IterativeSearchReport {
            requested_depth: 0,
            completed_depth: 0,
            completed: result.completed,
            total_nodes: result.stats.nodes,
            iterations: Vec::new(),
            result,
        };
    }

    let mut searcher = Searcher::new(algorithm, SearchLimits::depth(1));
    let mut iterations = Vec::with_capacity(usize::from(limits.depth));
    let mut total_nodes = 0u64;
    let mut completed_depth = 0u8;
    let mut root_hint = None;
    let mut best_complete = None;
    let mut last_attempt = None;

    for depth in 1..=limits.depth {
        let remaining = limits
            .nodes
            .map(|budget| budget.saturating_sub(total_nodes));
        if remaining == Some(0) {
            break;
        }
        let iteration_limits = SearchLimits {
            depth,
            nodes: remaining,
        };
        let report = search_with(&mut searcher, position, iteration_limits, root_hint);
        total_nodes += report.stats.nodes;
        iterations.push(IterationSummary {
            depth,
            completed: report.completed,
            best_move: report.best_move,
            score: report.score,
            nodes: report.stats.nodes,
            cutoffs: report.stats.cutoffs,
            tt_hits: report.stats.tt_hits,
        });
        if report.completed {
            completed_depth = depth;
            root_hint = report.best_move;
            best_complete = Some(report);
        } else {
            last_attempt = Some(report);
            break;
        }
    }

    let result = best_complete.or(last_attempt).unwrap_or_else(|| {
        search_with(
            &mut searcher,
            position,
            SearchLimits {
                depth: 1,
                nodes: Some(0),
            },
            None,
        )
    });
    IterativeSearchReport {
        requested_depth: limits.depth,
        completed_depth,
        completed: completed_depth == limits.depth,
        total_nodes,
        iterations,
        result,
    }
}

fn search_with(
    searcher: &mut Searcher,
    position: Position,
    limits: SearchLimits,
    root_hint: Option<Move>,
) -> SearchReport {
    searcher.begin_iteration(limits);
    let algorithm = searcher.algorithm;
    let mut position = position;
    if !searcher.enter_node(0) {
        return empty_report(position, algorithm, limits, false, searcher.stats);
    }
    if let Some(score) = terminal_score(position, 0) {
        searcher.stats.leaves = 1;
        return SearchReport {
            algorithm,
            requested_depth: limits.depth,
            completed: true,
            best_move: None,
            score,
            principal_variation: Vec::new(),
            root_branches: Vec::new(),
            stats: searcher.stats,
        };
    }
    if limits.depth == 0 {
        searcher.stats.leaves = 1;
        return empty_report(position, algorithm, limits, true, searcher.stats);
    }

    let mut best = Score(-INFINITY);
    let mut best_move = None;
    let mut best_line = Line::default();
    let mut root_branches = Vec::with_capacity(Column::COUNT);
    let mut completed = true;

    for mv in ordered_moves(position, algorithm, root_hint) {
        let nodes_before = searcher.stats.nodes;
        let cutoffs_before = searcher.stats.cutoffs;
        let undo = position
            .make_move(mv)
            .expect("generated moves must be legal");
        // Give every root move a full window. Besides producing an honest
        // score-above-each-column readout, this keeps the selected move's
        // stable tie-break independent of the order used inside each branch.
        let child = searcher.node(
            &mut position,
            limits.depth - 1,
            1,
            Score(-INFINITY),
            Score(INFINITY),
        );
        position.unmake_move(undo);
        let score = child.score.negated();
        root_branches.push(RootBranch {
            mv,
            score,
            nodes: searcher.stats.nodes - nodes_before,
            cutoffs: searcher.stats.cutoffs - cutoffs_before,
            completed: !child.stopped,
        });
        if score > best || (score == best && prefer(mv, best_move)) {
            best = score;
            best_move = Some(mv);
            best_line = Line::prepend(mv, child.line);
        }
        if child.stopped {
            completed = false;
            break;
        }
    }

    SearchReport {
        algorithm,
        requested_depth: limits.depth,
        completed,
        best_move,
        score: if best_move.is_some() {
            best
        } else {
            evaluate(position)
        },
        principal_variation: best_line.to_vec(),
        root_branches,
        stats: searcher.stats,
    }
}

fn empty_report(
    position: Position,
    algorithm: Algorithm,
    limits: SearchLimits,
    completed: bool,
    stats: SearchStats,
) -> SearchReport {
    SearchReport {
        algorithm,
        requested_depth: limits.depth,
        completed,
        best_move: None,
        score: evaluate(position),
        principal_variation: Vec::new(),
        root_branches: Vec::new(),
        stats,
    }
}

fn ordered_moves(position: Position, algorithm: Algorithm, first: Option<Move>) -> MoveList {
    let mut moves = MoveList::default();
    if position.result() != GameResult::Ongoing {
        return moves;
    }
    if first.is_some_and(|mv| position.can_play(mv.column())) {
        moves.push(first.expect("checked above"));
    }
    let columns = if algorithm.uses_center_order() {
        [3, 2, 4, 1, 5, 0, 6]
    } else {
        [0, 1, 2, 3, 4, 5, 6]
    };
    for index in columns {
        let mv = Move::new(Column::new(index));
        if Some(mv) != first && position.can_play(mv.column()) {
            moves.push(mv);
        }
    }
    moves
}

fn prefer(candidate: Move, current: Option<Move>) -> bool {
    const PRIORITY: [u8; Column::COUNT] = [5, 3, 1, 0, 2, 4, 6];
    current.is_none_or(|mv| PRIORITY[candidate.column().index()] < PRIORITY[mv.column().index()])
}

fn terminal_score(position: Position, ply: u8) -> Option<Score> {
    match position.result() {
        GameResult::Ongoing => None,
        GameResult::Draw => Some(Score(0)),
        GameResult::Win(winner) => {
            let distance = i16::from(ply);
            if winner == position.side_to_move() {
                Some(Score(FORCED_SCORE - distance))
            } else {
                Some(Score(-FORCED_SCORE + distance))
            }
        }
    }
}

/// Count open groups of four from the side-to-move perspective. The exact
/// weights are intentionally modest so search depth remains the visible
/// difference between opponents.
fn evaluate(position: Position) -> Score {
    let us = position.side_to_move();
    let them = us.other();
    let mut value = center_count(position, us) * 3 - center_count(position, them) * 3;
    for column in 0..Column::COUNT as i8 {
        for row in 0..crate::position::HEIGHT as i8 {
            for (dc, dr) in [(1i8, 0i8), (0, 1), (1, 1), (1, -1)] {
                let end_column = column + dc * 3;
                let end_row = row + dr * 3;
                if !(0..Column::COUNT as i8).contains(&end_column)
                    || !(0..crate::position::HEIGHT as i8).contains(&end_row)
                {
                    continue;
                }
                let mut ours = 0;
                let mut theirs = 0;
                for step in 0..4 {
                    let cell = crate::Cell::new(
                        Column::new((column + dc * step) as u8),
                        (row + dr * step) as u8,
                    );
                    match position.side_at(cell) {
                        Some(side) if side == us => ours += 1,
                        Some(_) => theirs += 1,
                        None => {}
                    }
                }
                if theirs == 0 {
                    value += window_weight(ours);
                } else if ours == 0 {
                    value -= window_weight(theirs);
                }
            }
        }
    }
    Score(value.clamp(-HEURISTIC_LIMIT, HEURISTIC_LIMIT))
}

fn center_count(position: Position, side: Side) -> i16 {
    let center = Column::new(3);
    (0..crate::position::HEIGHT as u8)
        .filter(|row| position.side_at(crate::Cell::new(center, *row)) == Some(side))
        .count() as i16
}

const fn window_weight(discs: i16) -> i16 {
    match discs {
        1 => 1,
        2 => 8,
        3 => 40,
        _ => 0,
    }
}
