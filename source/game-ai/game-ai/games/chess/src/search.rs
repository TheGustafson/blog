use crate::{
    Evaluation, EvaluationProfile, FeatureDelta, Move, MoveKind, MoveList, NnueAccumulator,
    PieceKind, Position, builtin_nnue, evaluate,
};
use std::cmp::Ordering;
#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;

const INFINITY: i32 = 31_000;
const MATE: i32 = 30_000;
const MATE_BAND: i32 = 29_000;
const DEFAULT_TT_ENTRIES: usize = 1 << 16;
const MAX_LINE_PLIES: usize = 128;

/// A search score whose terminal provenance remains visible to the caller.
#[derive(Debug, Clone, Copy)]
pub struct Score {
    raw: i32,
    exact_draw: bool,
}

impl Score {
    pub const fn raw(self) -> i32 {
        self.raw
    }

    pub const fn kind(self) -> ScoreKind {
        if self.exact_draw {
            ScoreKind::Draw
        } else if self.raw >= MATE_BAND {
            ScoreKind::MateIn {
                plies: (MATE - self.raw) as u16,
            }
        } else if self.raw <= -MATE_BAND {
            ScoreKind::MatedIn {
                plies: (MATE + self.raw) as u16,
            }
        } else {
            ScoreKind::Centipawns(self.raw)
        }
    }

    const fn centipawns(value: i32) -> Self {
        Self {
            raw: value,
            exact_draw: false,
        }
    }

    const fn draw() -> Self {
        Self {
            raw: 0,
            exact_draw: true,
        }
    }

    const fn mated(ply: u8) -> Self {
        Self {
            raw: -MATE + ply as i32,
            exact_draw: false,
        }
    }

    const fn negated(self) -> Self {
        Self {
            raw: -self.raw,
            exact_draw: self.exact_draw,
        }
    }

    const fn max(self, other: Self) -> Self {
        if self.raw >= other.raw { self } else { other }
    }

    const fn to_table(self, ply: u8) -> Self {
        let raw = if self.raw >= MATE_BAND {
            self.raw + ply as i32
        } else if self.raw <= -MATE_BAND {
            self.raw - ply as i32
        } else {
            self.raw
        };
        Self { raw, ..self }
    }

    const fn from_table(score: Self, ply: u8) -> Self {
        let raw = if score.raw >= MATE_BAND {
            score.raw - ply as i32
        } else if score.raw <= -MATE_BAND {
            score.raw + ply as i32
        } else {
            score.raw
        };
        Self { raw, ..score }
    }
}

impl PartialEq for Score {
    fn eq(&self, other: &Self) -> bool {
        self.raw == other.raw
    }
}

impl Eq for Score {}

impl PartialOrd for Score {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Score {
    fn cmp(&self, other: &Self) -> Ordering {
        self.raw.cmp(&other.raw)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScoreKind {
    MateIn { plies: u16 },
    MatedIn { plies: u16 },
    Draw,
    Centipawns(i32),
}

/// Search limits and independently controllable teaching features.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SearchConfig {
    pub depth: u8,
    pub nodes: Option<u64>,
    pub time_millis: Option<u64>,
    pub evaluator: EvaluationProfile,
    pub incremental_nnue: bool,
    pub quiescence: bool,
    pub move_ordering: bool,
    pub transposition_table: bool,
}

impl SearchConfig {
    pub const fn classical(depth: u8, evaluator: EvaluationProfile) -> Self {
        Self {
            depth,
            nodes: None,
            time_millis: None,
            evaluator,
            incremental_nnue: true,
            quiescence: true,
            move_ordering: true,
            transposition_table: true,
        }
    }

    pub const fn with_nodes(mut self, nodes: u64) -> Self {
        self.nodes = Some(nodes);
        self
    }

    pub const fn with_time_millis(mut self, time_millis: u64) -> Self {
        self.time_millis = Some(time_millis);
        self
    }
}

/// A named set of search limits and features for interactive play.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SearchPreset {
    pub name: &'static str,
    pub config: SearchConfig,
}

/// The built-in strength ladder, ordered from least to most search work.
pub const SEARCH_PRESETS: [SearchPreset; 6] = [
    SearchPreset {
        name: "beginner",
        config: SearchConfig {
            depth: 2,
            nodes: Some(1_000),
            time_millis: Some(25),
            evaluator: EvaluationProfile::TinyNnue,
            incremental_nnue: true,
            quiescence: false,
            move_ordering: false,
            transposition_table: false,
        },
    },
    SearchPreset {
        name: "easy",
        config: SearchConfig {
            depth: 3,
            nodes: Some(5_000),
            time_millis: Some(50),
            evaluator: EvaluationProfile::TinyNnue,
            incremental_nnue: true,
            quiescence: false,
            move_ordering: true,
            transposition_table: true,
        },
    },
    SearchPreset {
        name: "medium",
        config: SearchConfig {
            depth: 4,
            nodes: Some(15_000),
            time_millis: Some(100),
            evaluator: EvaluationProfile::TinyNnue,
            incremental_nnue: true,
            quiescence: true,
            move_ordering: true,
            transposition_table: true,
        },
    },
    SearchPreset {
        name: "hard",
        config: SearchConfig {
            depth: 5,
            nodes: Some(40_000),
            time_millis: Some(250),
            evaluator: EvaluationProfile::TinyNnue,
            incremental_nnue: true,
            quiescence: true,
            move_ordering: true,
            transposition_table: true,
        },
    },
    SearchPreset {
        name: "expert",
        config: SearchConfig {
            depth: 7,
            nodes: Some(120_000),
            time_millis: Some(500),
            evaluator: EvaluationProfile::TinyNnue,
            incremental_nnue: true,
            quiescence: true,
            move_ordering: true,
            transposition_table: true,
        },
    },
    SearchPreset {
        name: "maximum",
        config: SearchConfig {
            depth: 64,
            nodes: Some(300_000),
            time_millis: Some(1_000),
            evaluator: EvaluationProfile::TinyNnue,
            incremental_nnue: true,
            quiescence: true,
            move_ordering: true,
            transposition_table: true,
        },
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
    pub qnodes: u64,
    pub leaves: u64,
    pub evaluations: u64,
    pub cutoffs: u64,
    pub tt_probes: u64,
    pub tt_hits: u64,
    pub tt_stores: u64,
    pub max_ply: u8,
    pub nnue_refreshes: u64,
    pub nnue_updates: u64,
    pub nnue_feature_changes: u64,
    pub nnue_accumulator_ops: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    pub mv: Move,
    pub score: Score,
    pub nodes: u64,
    pub qnodes: u64,
    pub cutoffs: u64,
    pub completed: bool,
}

/// Complete result of one fixed-depth search attempt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchReport {
    pub config: SearchConfig,
    pub completed: bool,
    pub best_move: Option<Move>,
    pub score: Score,
    pub principal_variation: Vec<Move>,
    pub candidates: Vec<Candidate>,
    pub evaluation: Evaluation,
    pub stats: SearchStats,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IterationSummary {
    pub depth: u8,
    pub completed: bool,
    pub best_move: Option<Move>,
    pub score: Score,
    pub nodes: u64,
    pub qnodes: u64,
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
    moves: [Move; MAX_LINE_PLIES],
    len: u8,
}

impl Default for Line {
    fn default() -> Self {
        Self {
            moves: [Move::PLACEHOLDER; MAX_LINE_PLIES],
            len: 0,
        }
    }
}

impl Line {
    fn prepend(mv: Move, child: Self) -> Self {
        let mut line = Self::default();
        line.moves[0] = mv;
        let child_len = usize::from(child.len).min(MAX_LINE_PLIES - 1);
        if child_len > 0 {
            line.moves[1..=child_len].copy_from_slice(&child.moves[..child_len]);
        }
        line.len = child_len as u8 + 1;
        line
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

    fn to_vec(self) -> Vec<Move> {
        self.moves[..usize::from(self.len)].to_vec()
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
                moves: [Move::PLACEHOLDER; MAX_LINE_PLIES],
                len: 0,
            },
            stopped: false,
        }
    }

    const fn stopped(score: Score) -> Self {
        Self {
            score,
            line: Line {
                moves: [Move::PLACEHOLDER; MAX_LINE_PLIES],
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

struct Searcher<F>
where
    F: Fn() -> bool,
{
    config: SearchConfig,
    stats: SearchStats,
    node_limit: Option<u64>,
    table: Option<TranspositionTable>,
    history: Vec<u64>,
    nnue_accumulator: Option<NnueAccumulator>,
    #[cfg(not(target_arch = "wasm32"))]
    started: Instant,
    #[cfg(target_arch = "wasm32")]
    started_millis: f64,
    should_stop: F,
}

impl<F> Searcher<F>
where
    F: Fn() -> bool,
{
    fn new(config: SearchConfig, history: Vec<u64>, should_stop: F) -> Self {
        Self {
            config,
            stats: SearchStats::default(),
            node_limit: config.nodes,
            table: config
                .transposition_table
                .then(|| TranspositionTable::new(DEFAULT_TT_ENTRIES)),
            history,
            nnue_accumulator: None,
            #[cfg(not(target_arch = "wasm32"))]
            started: Instant::now(),
            #[cfg(target_arch = "wasm32")]
            started_millis: js_sys::Date::now(),
            should_stop,
        }
    }

    fn begin_iteration(&mut self, depth: u8, nodes: Option<u64>) {
        self.config.depth = depth;
        self.config.nodes = nodes;
        self.node_limit = nodes;
        self.stats = SearchStats::default();
    }

    fn enter_node(&mut self, ply: u8) -> bool {
        if self
            .node_limit
            .is_some_and(|limit| self.stats.nodes >= limit)
            || self.time_expired()
            || self.stop_requested()
        {
            return false;
        }
        self.stats.nodes += 1;
        self.stats.max_ply = self.stats.max_ply.max(ply);
        true
    }

    fn time_expired(&self) -> bool {
        let Some(limit) = self.config.time_millis else {
            return false;
        };
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.started.elapsed().as_millis() >= u128::from(limit)
        }
        #[cfg(target_arch = "wasm32")]
        {
            js_sys::Date::now() - self.started_millis >= limit as f64
        }
    }

    fn stop_requested(&self) -> bool {
        (self.should_stop)()
    }

    fn static_score(&mut self, position: &Position) -> Score {
        self.stats.leaves += 1;
        self.stats.evaluations += 1;
        let score = if self.config.evaluator == EvaluationProfile::TinyNnue
            && self.config.incremental_nnue
        {
            self.nnue_accumulator
                .as_ref()
                .expect("incremental NNUE search must prepare a root accumulator")
                .evaluate(position.side_to_move(), builtin_nnue())
        } else {
            if self.config.evaluator == EvaluationProfile::TinyNnue {
                self.stats.nnue_refreshes += 1;
                self.stats.nnue_accumulator_ops +=
                    u64::from(position.occupied().count_ones()) * 2 * crate::NNUE_HIDDEN as u64;
            }
            evaluate(position, self.config.evaluator).total
        };
        Score::centipawns(score)
    }

    fn prepare_root(&mut self, position: &Position) {
        self.nnue_accumulator = (self.config.evaluator == EvaluationProfile::TinyNnue
            && self.config.incremental_nnue)
            .then(|| NnueAccumulator::refresh(position, builtin_nnue()));
        if self.nnue_accumulator.is_some() {
            self.stats.nnue_refreshes += 1;
            self.stats.nnue_accumulator_ops +=
                u64::from(position.occupied().count_ones()) * 2 * crate::NNUE_HIDDEN as u64;
        }
    }

    fn push_nnue(&mut self, position: &Position, mv: Move) -> Option<FeatureDelta> {
        let accumulator = self.nnue_accumulator.as_mut()?;
        let delta = FeatureDelta::from_move(position, mv)
            .expect("a generated legal move must have a valid NNUE delta");
        let changes = delta.changes().len() as u64;
        accumulator.apply(builtin_nnue(), &delta);
        self.stats.nnue_updates += 1;
        self.stats.nnue_feature_changes += changes;
        self.stats.nnue_accumulator_ops += changes * 2 * crate::NNUE_HIDDEN as u64;
        Some(delta)
    }

    fn pop_nnue(&mut self, delta: Option<&FeatureDelta>) {
        let (Some(accumulator), Some(delta)) = (&mut self.nnue_accumulator, delta) else {
            return;
        };
        accumulator.revert(builtin_nnue(), delta);
        self.stats.nnue_accumulator_ops +=
            delta.changes().len() as u64 * 2 * crate::NNUE_HIDDEN as u64;
    }

    fn repeated(&self, key: u64) -> bool {
        self.history.iter().filter(|&&seen| seen == key).count() >= 3
    }

    fn table_line(&self, mut position: Position, depth: u8) -> Line {
        let Some(table) = &self.table else {
            return Line::default();
        };
        let mut line = Line::default();
        for _ in 0..depth {
            let legal = position.legal_moves();
            let Some(mv) = table
                .probe(position.key())
                .and_then(|entry| entry.best_move)
                .filter(|mv| legal.as_slice().contains(mv))
            else {
                break;
            };
            if !line.push(mv) {
                break;
            }
            position
                .make_unchecked(mv)
                .expect("a validated table move must be makeable");
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
            return NodeResult::stopped(self.static_score(position));
        }

        let legal = position.legal_moves();
        if let Some(score) = terminal_score(position, &legal, ply, self.repeated(position.key())) {
            self.stats.leaves += 1;
            return NodeResult::leaf(score);
        }
        if depth == 0 {
            return if self.config.quiescence {
                self.quiescence(position, legal, ply, alpha, beta, false)
            } else {
                NodeResult::leaf(self.static_score(position))
            };
        }

        let original_alpha = alpha;
        let original_beta = beta;
        let mut table_move = None;
        if self.config.transposition_table {
            self.stats.tt_probes += 1;
            if let Some(entry) = self
                .table
                .as_ref()
                .and_then(|table| table.probe(position.key()))
            {
                self.stats.tt_hits += 1;
                table_move = entry.best_move;
                if entry.depth >= depth {
                    let score = Score::from_table(entry.score, ply);
                    match entry.bound {
                        Bound::Exact => {
                            return NodeResult {
                                score,
                                line: self.table_line(position.clone(), depth),
                                stopped: false,
                            };
                        }
                        Bound::Lower => alpha = alpha.max(score),
                        Bound::Upper => beta = beta.min(score),
                    }
                    if alpha >= beta {
                        self.stats.cutoffs += 1;
                        return NodeResult {
                            score,
                            line: self.table_line(position.clone(), depth),
                            stopped: false,
                        };
                    }
                }
            }
        }

        let mut best = Score::centipawns(-INFINITY);
        let mut best_move = None;
        let mut best_line = Line::default();
        let first = self.config.move_ordering.then_some(table_move).flatten();
        for mv in ordered_moves(position, legal, first, self.config.move_ordering) {
            let nnue_delta = self.push_nnue(position, mv);
            let undo = position
                .make_unchecked(mv)
                .expect("generated legal move must be makeable");
            self.history.push(position.key());
            let child = self.node(
                position,
                depth - 1,
                ply + 1,
                beta.negated(),
                alpha.negated(),
            );
            self.history.pop();
            position.unmake_move(undo);
            self.pop_nnue(nnue_delta.as_ref());
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

        if self.config.transposition_table {
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
                score: best.to_table(ply),
                bound,
                best_move,
            };
            if self
                .table
                .as_mut()
                .expect("enabled table must exist")
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

    fn quiescence(
        &mut self,
        position: &mut Position,
        legal: MoveList,
        ply: u8,
        mut alpha: Score,
        beta: Score,
        enter: bool,
    ) -> NodeResult {
        if enter && !self.enter_node(ply) {
            return NodeResult::stopped(self.static_score(position));
        }
        self.stats.qnodes += 1;

        if let Some(score) = terminal_score(position, &legal, ply, self.repeated(position.key())) {
            self.stats.leaves += 1;
            return NodeResult::leaf(score);
        }

        let in_check = position.in_check(position.side_to_move());
        let mut best = if in_check {
            Score::centipawns(-INFINITY)
        } else {
            self.static_score(position)
        };
        if !in_check {
            if best >= beta {
                self.stats.cutoffs += 1;
                return NodeResult::leaf(best);
            }
            alpha = alpha.max(best);
        }

        let tactical = if in_check {
            legal
        } else {
            tactical_moves(position, legal)
        };
        if tactical.is_empty() {
            return NodeResult::leaf(best);
        }

        let mut best_move = None;
        let mut best_line = Line::default();
        for mv in ordered_moves(position, tactical, None, true) {
            let nnue_delta = self.push_nnue(position, mv);
            let undo = position
                .make_unchecked(mv)
                .expect("generated tactical move must be makeable");
            self.history.push(position.key());
            let child_legal = position.legal_moves();
            let child = self.quiescence(
                position,
                child_legal,
                ply.saturating_add(1),
                beta.negated(),
                alpha.negated(),
                true,
            );
            self.history.pop();
            position.unmake_move(undo);
            self.pop_nnue(nnue_delta.as_ref());
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
        NodeResult {
            score: best,
            line: best_line,
            stopped: false,
        }
    }
}

/// Search one fixed depth. Root candidates receive full windows so their
/// displayed scores are comparable rather than incidental alpha-beta bounds.
pub fn search(position: Position, config: SearchConfig) -> SearchReport {
    search_with_history(position, config, &[])
}

/// Search with the real game keys that precede `position`. The current key is
/// appended, allowing exact threefold detection in a session without putting
/// browser-specific history inside `Position`.
pub fn search_with_history(
    position: Position,
    config: SearchConfig,
    prior_keys: &[u64],
) -> SearchReport {
    let history = normalized_history(position.key(), prior_keys);
    let mut searcher = Searcher::new(config, history, || false);
    search_at_depth(&mut searcher, position, None)
}

/// Iteratively deepen while retaining the TT and previous root move. A node
/// budget is cumulative and exact; an interrupted iteration never replaces
/// the last complete result.
pub fn iterative_search(position: Position, config: SearchConfig) -> IterativeSearchReport {
    iterative_search_with_history(position, config, &[])
}

/// Iteratively deepens with the game keys preceding `position`.
///
/// `prior_keys` enables exact threefold detection without coupling a
/// position value to session history.
pub fn iterative_search_with_history(
    position: Position,
    config: SearchConfig,
    prior_keys: &[u64],
) -> IterativeSearchReport {
    iterative_search_with_history_until(position, config, prior_keys, || false)
}

pub(crate) fn iterative_search_with_history_until<F>(
    position: Position,
    config: SearchConfig,
    prior_keys: &[u64],
    should_stop: F,
) -> IterativeSearchReport
where
    F: Fn() -> bool,
{
    if config.depth == 0 {
        let history = normalized_history(position.key(), prior_keys);
        let mut searcher = Searcher::new(config, history, should_stop);
        let result = search_at_depth(&mut searcher, position, None);
        return IterativeSearchReport {
            requested_depth: 0,
            completed_depth: 0,
            completed: result.completed,
            total_nodes: result.stats.nodes,
            iterations: Vec::new(),
            result,
        };
    }

    let requested_depth = config.depth;
    let mut first = config;
    first.depth = 1;
    first.nodes = None;
    let mut searcher = Searcher::new(
        first,
        normalized_history(position.key(), prior_keys),
        should_stop,
    );
    let mut total_nodes = 0;
    let mut completed_depth = 0;
    let mut root_hint = None;
    let mut best_complete = None;
    let mut last_attempt = None;
    let mut iterations = Vec::with_capacity(usize::from(requested_depth));

    for depth in 1..=requested_depth {
        let remaining = config
            .nodes
            .map(|budget| budget.saturating_sub(total_nodes));
        if remaining == Some(0) {
            break;
        }
        if searcher.time_expired() || searcher.stop_requested() {
            break;
        }
        searcher.begin_iteration(depth, remaining);
        let report = search_at_depth(&mut searcher, position.clone(), root_hint);
        total_nodes += report.stats.nodes;
        iterations.push(IterationSummary {
            depth,
            completed: report.completed,
            best_move: report.best_move,
            score: report.score,
            nodes: report.stats.nodes,
            qnodes: report.stats.qnodes,
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
        searcher.begin_iteration(1, Some(0));
        search_at_depth(&mut searcher, position, None)
    });
    IterativeSearchReport {
        result,
        requested_depth,
        completed_depth,
        completed: completed_depth == requested_depth,
        total_nodes,
        iterations,
    }
}

fn search_at_depth<F>(
    searcher: &mut Searcher<F>,
    mut position: Position,
    root_hint: Option<Move>,
) -> SearchReport
where
    F: Fn() -> bool,
{
    searcher.prepare_root(&position);
    let config = searcher.config;
    let evaluation = evaluate(&position, config.evaluator);
    if !searcher.enter_node(0) {
        return SearchReport {
            config,
            completed: false,
            best_move: None,
            score: Score::centipawns(evaluation.total),
            principal_variation: Vec::new(),
            candidates: Vec::new(),
            evaluation,
            stats: searcher.stats,
        };
    }

    let legal = position.legal_moves();
    if let Some(score) = terminal_score(&position, &legal, 0, searcher.repeated(position.key())) {
        searcher.stats.leaves += 1;
        return SearchReport {
            config,
            completed: true,
            best_move: None,
            score,
            principal_variation: Vec::new(),
            candidates: Vec::new(),
            evaluation,
            stats: searcher.stats,
        };
    }
    if config.depth == 0 {
        searcher.stats.leaves += 1;
        searcher.stats.evaluations += 1;
        return SearchReport {
            config,
            completed: true,
            best_move: None,
            score: Score::centipawns(evaluation.total),
            principal_variation: Vec::new(),
            candidates: Vec::new(),
            evaluation,
            stats: searcher.stats,
        };
    }

    let mut best = Score::centipawns(-INFINITY);
    let mut best_move = None;
    let mut best_line = Line::default();
    let mut completed = true;
    let mut candidates = Vec::with_capacity(legal.len());
    let first = config.move_ordering.then_some(root_hint).flatten();
    for mv in ordered_moves(&position, legal, first, config.move_ordering) {
        let before = searcher.stats;
        let nnue_delta = searcher.push_nnue(&position, mv);
        let undo = position
            .make_unchecked(mv)
            .expect("generated legal root move must be makeable");
        searcher.history.push(position.key());
        let child = searcher.node(
            &mut position,
            config.depth - 1,
            1,
            Score::centipawns(-INFINITY),
            Score::centipawns(INFINITY),
        );
        searcher.history.pop();
        position.unmake_move(undo);
        searcher.pop_nnue(nnue_delta.as_ref());
        let score = child.score.negated();
        candidates.push(Candidate {
            mv,
            score,
            nodes: searcher.stats.nodes - before.nodes,
            qnodes: searcher.stats.qnodes - before.qnodes,
            cutoffs: searcher.stats.cutoffs - before.cutoffs,
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
        config,
        completed,
        best_move,
        score: if best_move.is_some() {
            best
        } else {
            Score::centipawns(evaluation.total)
        },
        principal_variation: best_line.to_vec(),
        candidates,
        evaluation,
        stats: searcher.stats,
    }
}

fn normalized_history(current: u64, prior: &[u64]) -> Vec<u64> {
    let mut history = prior.to_vec();
    history.push(current);
    history
}

fn terminal_score(position: &Position, legal: &MoveList, ply: u8, repeated: bool) -> Option<Score> {
    if legal.is_empty() {
        return if position.in_check(position.side_to_move()) {
            Some(Score::mated(ply))
        } else {
            Some(Score::draw())
        };
    }
    if position.halfmove_clock() >= 100 || repeated || position.has_insufficient_material() {
        Some(Score::draw())
    } else {
        None
    }
}

fn tactical_moves(position: &Position, legal: MoveList) -> MoveList {
    let mut tactical = MoveList::default();
    for mv in legal {
        if is_capture(position, mv) || mv.promotion().is_some() {
            tactical.push(mv);
        }
    }
    tactical
}

fn ordered_moves(
    position: &Position,
    mut legal: MoveList,
    first: Option<Move>,
    enabled: bool,
) -> MoveList {
    if !enabled {
        return legal;
    }
    legal.as_mut_slice().sort_unstable_by(|left, right| {
        move_priority(position, *right, first)
            .cmp(&move_priority(position, *left, first))
            .then_with(|| left.cmp(right))
    });
    legal
}

fn move_priority(position: &Position, mv: Move, first: Option<Move>) -> i32 {
    if Some(mv) == first {
        return 1_000_000;
    }
    let promotion = mv.promotion().map_or(0, piece_value);
    let capture = captured_kind(position, mv).map_or(0, |victim| {
        let attacker = position
            .piece_at(mv.from())
            .map_or(0, |piece| piece_value(piece.kind));
        50_000 + piece_value(victim) * 10 - attacker
    });
    promotion * 100 + capture
}

fn is_capture(position: &Position, mv: Move) -> bool {
    mv.kind() == MoveKind::EnPassant || position.piece_at(mv.to()).is_some()
}

fn captured_kind(position: &Position, mv: Move) -> Option<PieceKind> {
    if mv.kind() == MoveKind::EnPassant {
        Some(PieceKind::Pawn)
    } else {
        position.piece_at(mv.to()).map(|piece| piece.kind)
    }
}

const fn piece_value(kind: PieceKind) -> i32 {
    [100, 320, 330, 500, 900, 20_000][kind.index()]
}

fn prefer(candidate: Move, current: Option<Move>) -> bool {
    current.is_none_or(|mv| candidate < mv)
}
