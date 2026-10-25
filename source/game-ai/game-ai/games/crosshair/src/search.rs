use crate::position::{
    BOARD_MASK, BOTTOM_EDGE_MASK, CELLS, Color, GameResult, LEFT_EDGE_MASK, Move, Position,
    RIGHT_EDGE_MASK, SIZE, TOP_EDGE_MASK, compute_flips, dilate_ortho, has_winning_road,
};
#[cfg(not(target_arch = "wasm32"))]
use std::time::{Duration, Instant};

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

const INF: i32 = 32_000;
const MATE: i32 = 30_000;
const DEFAULT_TT_ENTRIES: usize = 1 << 16; // 65,536 entries
const MAX_PLY: usize = CELLS;
const EVAL_SCALE: f64 = 1_800.0;

const CELL_WEIGHT: [i32; CELLS] = [
    1, 2, 3, 4, 3, 2, 1, // rank 1
    2, 3, 4, 5, 4, 3, 2, // rank 2
    3, 4, 6, 7, 6, 4, 3, // rank 3
    4, 5, 7, 8, 7, 5, 4, // rank 4
    3, 4, 6, 7, 6, 4, 3, // rank 5
    2, 3, 4, 5, 4, 3, 2, // rank 6
    1, 2, 3, 4, 3, 2, 1, // rank 7
];

const SPAN_BONUS: [i32; SIZE + 1] = [0, 15, 75, 210, 460, 920, 1_750, 4_000];

const RANK_MASKS: [u64; SIZE] = [
    TOP_EDGE_MASK,
    TOP_EDGE_MASK << 7,
    TOP_EDGE_MASK << 14,
    TOP_EDGE_MASK << 21,
    TOP_EDGE_MASK << 28,
    TOP_EDGE_MASK << 35,
    BOTTOM_EDGE_MASK,
];

const FILE_MASKS: [u64; SIZE] = [
    LEFT_EDGE_MASK,
    LEFT_EDGE_MASK << 1,
    LEFT_EDGE_MASK << 2,
    LEFT_EDGE_MASK << 3,
    LEFT_EDGE_MASK << 4,
    LEFT_EDGE_MASK << 5,
    RIGHT_EDGE_MASK,
];

/// Limits for one iterative-deepening alpha-beta search. Zero disables the soft time limit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SearchOptions {
    /// Maximum iterative-deepening depth (`1..=20`).
    pub max_depth: u8,
    /// Maximum visited nodes (`1..=10_000_000`).
    pub node_limit: u64,
    /// Soft time budget in milliseconds (`0` disables the time limit).
    pub soft_time_ms: u32,
}

impl Default for SearchOptions {
    fn default() -> Self {
        Self {
            max_depth: 6,
            node_limit: 200_000,
            soft_time_ms: 0,
        }
    }
}

/// Named strength preset for the alpha-beta searcher.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SearchPreset {
    /// Preset identifier (`"beginner"` through `"maximum"`).
    pub name: &'static str,
    /// Search limits for this preset.
    pub options: SearchOptions,
}

/// Standard six strength presets for alpha-beta search.
pub const SEARCH_PRESETS: [SearchPreset; 6] = [
    SearchPreset {
        name: "beginner",
        options: SearchOptions {
            max_depth: 2,
            node_limit: 500,
            soft_time_ms: 25,
        },
    },
    SearchPreset {
        name: "easy",
        options: SearchOptions {
            max_depth: 3,
            node_limit: 2_500,
            soft_time_ms: 50,
        },
    },
    SearchPreset {
        name: "medium",
        options: SearchOptions {
            max_depth: 5,
            node_limit: 15_000,
            soft_time_ms: 120,
        },
    },
    SearchPreset {
        name: "hard",
        options: SearchOptions {
            max_depth: 6,
            node_limit: 60_000,
            soft_time_ms: 250,
        },
    },
    SearchPreset {
        name: "expert",
        options: SearchOptions {
            max_depth: 8,
            node_limit: 200_000,
            soft_time_ms: 600,
        },
    },
    SearchPreset {
        name: "maximum",
        options: SearchOptions {
            max_depth: 12,
            node_limit: 600_000,
            soft_time_ms: 1_000,
        },
    },
];

/// Looks up an alpha-beta preset by name.
pub fn search_preset(name: &str) -> Option<SearchPreset> {
    SEARCH_PRESETS
        .iter()
        .copied()
        .find(|preset| preset.name == name)
}

/// Per-root-move statistics collected during an alpha-beta search.
#[derive(Clone, Debug, PartialEq)]
pub struct SearchBranch {
    /// Root move.
    pub mv: Move,
    /// Nodes searched under this root move.
    pub visits: u32,
    /// Prior weight from tactical move ordering normalized to `[0.0, 1.0]`.
    pub prior: f64,
    /// Normalized expected score in `[0.0, 1.0]` (`0.5` is even).
    pub expected_score: f64,
    /// Raw centipawn-style search score.
    pub score: i32,
}

/// Summary of an iterative-deepening alpha-beta search.
#[derive(Clone, Debug, PartialEq)]
pub struct SearchReport {
    /// Selected move, or `None` if the position is terminal.
    pub best_move: Option<Move>,
    /// Raw search score from the side-to-move's perspective.
    pub score: i32,
    /// Normalized expected score in `[0.0, 1.0]` (`0.5` is even).
    pub expected_score: f64,
    /// Last fully completed iterative-deepening depth.
    pub depth: u8,
    /// Total visited nodes.
    pub nodes: u64,
    /// Total static leaf evaluations performed.
    pub leaf_evaluations: u32,
    /// Transposition table hits.
    pub tt_hits: u64,
    /// Beta cutoffs.
    pub cutoffs: u64,
    /// Elapsed wall-clock time in milliseconds.
    pub elapsed_ms: f64,
    /// Principal variation extracted from the transposition table.
    pub principal_variation: Vec<Move>,
    /// Root move statistics ordered from strongest to weakest.
    pub root_moves: Vec<SearchBranch>,
}

#[derive(Clone, Copy, Debug, Default)]
struct Entry {
    key: u64,
    score: i16,
    depth: u8,
    bound: Bound,
    best_move: Option<Move>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum Bound {
    #[default]
    Empty,
    Exact,
    Lower,
    Upper,
}

/// Reusable principal-variation alpha-beta searcher with a 65,536-entry Zobrist transposition table.
pub struct Searcher {
    table: Vec<Entry>,
    history: [[i32; CELLS]; 2],
    nodes: u64,
    leaf_evaluations: u32,
    node_limit: u64,
    clock: SearchClock,
    tt_hits: u64,
    cutoffs: u64,
}

impl Default for Searcher {
    fn default() -> Self {
        Self::new()
    }
}

impl Searcher {
    /// Creates a new searcher with an empty 65,536-entry transposition table.
    pub fn new() -> Self {
        Self {
            table: vec![Entry::default(); DEFAULT_TT_ENTRIES],
            history: [[0; CELLS]; 2],
            nodes: 0,
            leaf_evaluations: 0,
            node_limit: 0,
            clock: SearchClock::new(0),
            tt_hits: 0,
            cutoffs: 0,
        }
    }

    /// Runs iterative-deepening negamax search on `position`.
    pub fn search(&mut self, position: Position, options: SearchOptions) -> SearchReport {
        self.nodes = 0;
        self.leaf_evaluations = 0;
        self.tt_hits = 0;
        self.cutoffs = 0;
        self.node_limit = options.node_limit.max(1);
        self.clock = SearchClock::new(options.soft_time_ms);
        self.age_history();

        let legal = position.legal_moves();
        let Some(fallback) = legal.iter().next() else {
            let score = terminal_score(position, 0).unwrap_or(0);
            return SearchReport {
                best_move: None,
                score,
                expected_score: score_to_expected(score),
                depth: 0,
                nodes: 0,
                leaf_evaluations: 0,
                tt_hits: 0,
                cutoffs: 0,
                elapsed_ms: self.clock.elapsed_ms(),
                principal_variation: Vec::new(),
                root_moves: Vec::new(),
            };
        };

        let priors = normalized_tactical_priors(position);
        let mut completed_depth = 0;
        let mut best_move = fallback;
        let mut best_score = evaluate_position(position);
        let mut best_branches = legal
            .iter()
            .map(|mv| SearchBranch {
                mv,
                visits: 1,
                prior: priors[mv.index() as usize],
                expected_score: 0.5,
                score: 0,
            })
            .collect::<Vec<_>>();

        for depth in 1..=options.max_depth.max(1) {
            if depth > 1 && self.clock.expired() {
                break;
            }
            let Some((mv, score, branches)) = self.search_root(position, depth, &priors) else {
                break;
            };
            best_move = mv;
            best_score = score;
            best_branches = branches;
            completed_depth = depth;
            if score.abs() >= MATE - MAX_PLY as i32 {
                break;
            }
        }

        best_branches.sort_by(|a, b| {
            b.score
                .cmp(&a.score)
                .then_with(|| b.visits.cmp(&a.visits))
                .then_with(|| a.mv.index().cmp(&b.mv.index()))
        });

        SearchReport {
            best_move: Some(best_move),
            score: best_score,
            expected_score: score_to_expected(best_score),
            depth: completed_depth,
            nodes: self.nodes,
            leaf_evaluations: self.leaf_evaluations,
            tt_hits: self.tt_hits,
            cutoffs: self.cutoffs,
            elapsed_ms: self.clock.elapsed_ms(),
            principal_variation: self.principal_variation(position, completed_depth),
            root_moves: best_branches,
        }
    }

    fn search_root(
        &mut self,
        position: Position,
        depth: u8,
        priors: &[f64; CELLS],
    ) -> Option<(Move, i32, Vec<SearchBranch>)> {
        let key = position.hash();
        let tt_move = self.probe(key).and_then(|entry| entry.best_move);
        let moves = self.ordered_moves(position, tt_move);
        let mut alpha = -INF;
        let beta = INF;
        let mut best_move = moves.first()?.0;
        let mut best_score = -INF;
        let mut branches = Vec::with_capacity(moves.len());

        for (index, (mv, _)) in moves.into_iter().enumerate() {
            let before_nodes = self.nodes;
            let child = position.play(mv).expect("ordered moves are legal");
            let mut score;
            if index == 0 {
                score = -self.negamax(child, depth - 1, 1, -beta, -alpha)?;
            } else {
                score = -self.negamax(child, depth - 1, 1, -alpha - 1, -alpha)?;
                if score > alpha && score < beta {
                    score = -self.negamax(child, depth - 1, 1, -beta, -alpha)?;
                }
            }
            let branch_nodes = (self.nodes - before_nodes).max(1) as u32;
            branches.push(SearchBranch {
                mv,
                visits: branch_nodes,
                prior: priors[mv.index() as usize],
                expected_score: score_to_expected(score),
                score,
            });
            if score > best_score {
                best_score = score;
                best_move = mv;
            }
            alpha = alpha.max(score);
        }

        self.store(key, depth, best_score, Bound::Exact, Some(best_move), 0);
        Some((best_move, best_score, branches))
    }

    fn negamax(
        &mut self,
        position: Position,
        depth: u8,
        ply: u8,
        mut alpha: i32,
        beta: i32,
    ) -> Option<i32> {
        if !self.visit_node() {
            return None;
        }
        if let Some(score) = terminal_score(position, ply) {
            return Some(score);
        }
        if depth == 0 {
            self.leaf_evaluations += 1;
            return Some(evaluate_position(position));
        }

        let key = position.hash();
        let original_alpha = alpha;
        let mut tt_move = None;
        if let Some(entry) = self.probe(key) {
            tt_move = entry.best_move;
            if entry.depth >= depth {
                self.tt_hits += 1;
                let score = decode_score(i32::from(entry.score), ply);
                match entry.bound {
                    Bound::Exact => return Some(score),
                    Bound::Lower if score >= beta => return Some(score),
                    Bound::Upper if score <= alpha => return Some(score),
                    _ => {}
                }
            }
        }

        let moves = self.ordered_moves(position, tt_move);
        let mut best_score = -INF;
        let mut best_move = None;
        for (index, (mv, _)) in moves.into_iter().enumerate() {
            let child = position.play(mv).expect("ordered moves are legal");
            let mut score;
            if index == 0 {
                score = -self.negamax(child, depth - 1, ply + 1, -beta, -alpha)?;
            } else {
                score = -self.negamax(child, depth - 1, ply + 1, -alpha - 1, -alpha)?;
                if score > alpha && score < beta {
                    score = -self.negamax(child, depth - 1, ply + 1, -beta, -alpha)?;
                }
            }
            if score > best_score {
                best_score = score;
                best_move = Some(mv);
            }
            alpha = alpha.max(score);
            if alpha >= beta {
                self.cutoffs += 1;
                self.history[position.side_to_move().index()][mv.index() as usize] +=
                    i32::from(depth) * i32::from(depth);
                break;
            }
        }

        let bound = if best_score <= original_alpha {
            Bound::Upper
        } else if best_score >= beta {
            Bound::Lower
        } else {
            Bound::Exact
        };
        self.store(key, depth, best_score, bound, best_move, ply);
        Some(best_score)
    }

    fn ordered_moves(&self, position: Position, tt_move: Option<Move>) -> Vec<(Move, i32)> {
        let side = position.side_to_move();
        let mut moves = position
            .legal_moves()
            .iter()
            .map(|mv| {
                let mut score = self.history[side.index()][mv.index() as usize];
                if Some(mv) == tt_move {
                    score += 500_000;
                }
                score += tactical_move_score(position, mv);
                (mv, score)
            })
            .collect::<Vec<_>>();
        moves.sort_unstable_by(|left, right| {
            right
                .1
                .cmp(&left.1)
                .then_with(|| left.0.index().cmp(&right.0.index()))
        });
        moves
    }

    fn visit_node(&mut self) -> bool {
        if self.nodes >= self.node_limit {
            return false;
        }
        self.nodes += 1;
        if self.nodes & 255 == 0 && self.clock.expired() {
            return false;
        }
        true
    }

    fn probe(&self, key: u64) -> Option<Entry> {
        let entry = self.table[key as usize & (self.table.len() - 1)];
        (entry.bound != Bound::Empty && entry.key == key).then_some(entry)
    }

    fn store(
        &mut self,
        key: u64,
        depth: u8,
        score: i32,
        bound: Bound,
        best_move: Option<Move>,
        ply: u8,
    ) {
        let index = key as usize & (self.table.len() - 1);
        let old = self.table[index];
        if old.key != key || depth >= old.depth || bound == Bound::Exact {
            self.table[index] = Entry {
                key,
                score: encode_score(score, ply) as i16,
                depth,
                bound,
                best_move,
            };
        }
    }

    fn principal_variation(&self, mut position: Position, depth: u8) -> Vec<Move> {
        let mut variation = Vec::with_capacity(depth as usize);
        for _ in 0..depth {
            let Some(entry) = self.probe(position.hash()) else {
                break;
            };
            let Some(mv) = entry.best_move else {
                break;
            };
            if !position.legal_moves().contains(mv) {
                break;
            }
            variation.push(mv);
            position = position.play(mv).expect("PV move was verified legal");
        }
        variation
    }

    fn age_history(&mut self) {
        for side in &mut self.history {
            for score in side {
                *score /= 2;
            }
        }
    }
}

/// Converts a raw search score into a normalized expected score in `[0.0, 1.0]`.
pub fn score_to_expected(score: i32) -> f64 {
    if score >= MATE - MAX_PLY as i32 {
        1.0
    } else if score <= -MATE + MAX_PLY as i32 {
        0.0
    } else {
        let s = f64::from(score);
        let bounded = s / (s.abs() + EVAL_SCALE);
        (bounded + 1.0) * 0.5
    }
}

/// Evaluates `position` from `position.side_to_move()`'s perspective in `[-1.0, 1.0]`.
pub fn evaluate_normalized(position: Position) -> f64 {
    let side = position.side_to_move();
    match position.result() {
        GameResult::Win { winner, .. } => {
            if winner == side {
                1.0
            } else {
                -1.0
            }
        }
        GameResult::Ongoing => {
            let raw = f64::from(evaluate_position(position));
            raw / (raw.abs() + EVAL_SCALE)
        }
    }
}

/// Handcrafted static evaluation from `position.side_to_move()`'s perspective.
pub fn evaluate_position(position: Position) -> i32 {
    let side = position.side_to_move();
    let opp = side.opponent();
    match position.result() {
        GameResult::Win { winner, .. } => {
            return if winner == side { MATE } else { -MATE };
        }
        GameResult::Ongoing => {}
    }

    let own_bits = position.stones_for(side);
    let opp_bits = position.stones_for(opp);
    let mut score = 0_i32;

    // 1. Stone differential (each stone provides pinch anchors and connection density)
    let stone_diff = own_bits.count_ones() as i32 - opp_bits.count_ones() as i32;
    score += stone_diff * 55;

    // 2. Centrality of occupied cells
    let mut mask = own_bits;
    while mask != 0 {
        let idx = mask.trailing_zeros() as usize;
        mask &= mask - 1;
        score += CELL_WEIGHT[idx] * 5;
    }
    let mut mask = opp_bits;
    while mask != 0 {
        let idx = mask.trailing_zeros() as usize;
        mask &= mask - 1;
        score -= CELL_WEIGHT[idx] * 5;
    }

    // 3. Edge-to-edge 4-orthogonal component span and 2x2 diagonal bridge potential
    score += connection_potential(own_bits, opp_bits, side);
    score -= connection_potential(opp_bits, own_bits, opp);

    // 4. Active crosshair tactical availability
    for mv in position.legal_moves() {
        let idx = mv.index() as usize;
        let flips = compute_flips(idx, own_bits, opp_bits);
        if has_winning_road(own_bits | mv.bit() | flips, side) {
            score += 4_500;
            break;
        }
    }
    if position.is_wildcard() {
        score += 180;
    }

    score
}

/// Evaluates `color`'s connected component spans, edge anchors, and 2×2 virtual bridges.
pub fn connection_potential(stones: u64, opp_stones: u64, color: Color) -> i32 {
    if stones == 0 {
        return 0;
    }
    let slice_masks = match color {
        Color::Red => &RANK_MASKS,
        Color::Blue => &FILE_MASKS,
    };

    let mut score = 0_i32;
    let mut slices_touched = 0_i32;
    for &m in slice_masks {
        if (stones & m) != 0 {
            slices_touched += 1;
        }
    }
    score += slices_touched * 42;

    // Connected component span along the target axis
    let mut remaining = stones;
    let mut best_span = 0_usize;
    while remaining != 0 {
        let seed = 1_u64 << remaining.trailing_zeros();
        let mut comp = seed;
        loop {
            let next = dilate_ortho(comp, remaining);
            if next == comp {
                break;
            }
            comp = next;
        }
        remaining &= !comp;

        let mut min_s = SIZE;
        let mut max_s = 0_usize;
        for (s, &m) in slice_masks.iter().enumerate() {
            if (comp & m) != 0 {
                if s < min_s {
                    min_s = s;
                }
                max_s = s;
            }
        }
        let span = (max_s + 1).saturating_sub(min_s);
        score += SPAN_BONUS[span];
        if min_s == 0 {
            score += 65;
        }
        if max_s == SIZE - 1 {
            score += 65;
        }
        if span > best_span {
            best_span = span;
        }
    }
    score += SPAN_BONUS[best_span] / 2;

    // 2x2 diagonal virtual bridges and crosscut pinch threats
    let empty = !(stones | opp_stones) & BOARD_MASK;
    for r in 0..(SIZE - 1) {
        for f in 0..(SIZE - 1) {
            let c00 = 1_u64 << (r * SIZE + f);
            let c10 = 1_u64 << (r * SIZE + f + 1);
            let c01 = 1_u64 << ((r + 1) * SIZE + f);
            let c11 = 1_u64 << ((r + 1) * SIZE + f + 1);

            let d1_own = (stones & (c00 | c11)) == (c00 | c11);
            let d2_own = (stones & (c10 | c01)) == (c10 | c01);
            if d1_own && ((empty & (c10 | c01)) != 0) {
                score += 55;
            }
            if d2_own && ((empty & (c00 | c11)) != 0) {
                score += 55;
            }
            // Pending 2x2 crosscut trap: opponent holds one diagonal and we hold one corner of the other
            if (opp_stones & (c10 | c01)) == (c10 | c01)
                && (stones & (c00 | c11)).count_ones() == 1
                && (empty & (c00 | c11)) != 0
            {
                score += 110;
            }
            if (opp_stones & (c00 | c11)) == (c00 | c11)
                && (stones & (c10 | c01)).count_ones() == 1
                && (empty & (c10 | c01)) != 0
            {
                score += 110;
            }
        }
    }

    score
}

/// Computes normalized handcrafted prior probabilities over legal moves for `position`.
pub fn normalized_tactical_priors(position: Position) -> [f64; CELLS] {
    let mut priors = [0.0_f64; CELLS];
    let legal = position.legal_moves();
    if legal.is_empty() {
        return priors;
    }
    let mut scores = [0_i32; CELLS];
    let mut max_score = i32::MIN;
    for mv in legal {
        let s = tactical_move_score(position, mv);
        scores[mv.index() as usize] = s;
        if s > max_score {
            max_score = s;
        }
    }
    let temp = 18_000.0_f64;
    let mut total = 0.0_f64;
    for mv in legal {
        let idx = mv.index() as usize;
        let diff = f64::from(scores[idx] - max_score) / temp;
        let w = diff.max(-20.0).exp();
        priors[idx] = w;
        total += w;
    }
    if total > 0.0 {
        for mv in legal {
            priors[mv.index() as usize] /= total;
        }
    }
    priors
}

/// Fast tactical score for ordering moves and seeding handcrafted PUCT / tactical UCT.
pub fn tactical_move_score(position: Position, mv: Move) -> i32 {
    let side = position.side_to_move();
    let opp = side.opponent();
    let own_bits = position.stones_for(side);
    let opp_bits = position.stones_for(opp);

    let child = position
        .play(mv)
        .expect("tactical score requires legal move");
    if matches!(child.result(), GameResult::Win { winner, .. } if winner == side) {
        return 1_000_000;
    }

    let idx = mv.index() as usize;
    let bit = mv.bit();
    let flips = child.last_flips().len() as i32;
    let mut score = CELL_WEIGHT[idx] * 220 + flips * 18_000;

    // Check if occupying mv directly blocks an opponent 1-move road completion
    let opp_flips_at_mv = compute_flips(idx, opp_bits, own_bits);
    if has_winning_road(opp_bits | bit | opp_flips_at_mv, opp) {
        score += 110_000;
    }

    // Connection progress delta
    let before_conn = connection_potential(own_bits, opp_bits, side);
    let after_conn = connection_potential(child.stones_for(side), child.stones_for(opp), side);
    score += (after_conn - before_conn) * 14;

    // Routing safety in child: penalize granting opponent an immediate win or wildcard
    if child.is_wildcard() {
        score -= 22_000;
    }
    if opponent_has_immediate_win(child) {
        score -= 480_000;
    }

    score
}

#[inline]
fn opponent_has_immediate_win(child: Position) -> bool {
    let opp = child.side_to_move();
    let opp_bits = child.stones_for(opp);
    let own_bits = child.stones_for(opp.opponent());

    for opp_mv in child.legal_moves() {
        let flips = compute_flips(opp_mv.index() as usize, opp_bits, own_bits);
        if has_winning_road(opp_bits | opp_mv.bit() | flips, opp) {
            return true;
        }
    }
    false
}

fn terminal_score(position: Position, ply: u8) -> Option<i32> {
    match position.result() {
        GameResult::Ongoing => None,
        GameResult::Win { winner, .. } if winner == position.side_to_move() => {
            Some(MATE - i32::from(ply))
        }
        GameResult::Win { .. } => Some(-MATE + i32::from(ply)),
    }
}

fn encode_score(score: i32, ply: u8) -> i32 {
    if score > MATE - MAX_PLY as i32 {
        score + i32::from(ply)
    } else if score < -MATE + MAX_PLY as i32 {
        score - i32::from(ply)
    } else {
        score
    }
}

fn decode_score(score: i32, ply: u8) -> i32 {
    if score > MATE - MAX_PLY as i32 {
        score - i32::from(ply)
    } else if score < -MATE + MAX_PLY as i32 {
        score + i32::from(ply)
    } else {
        score
    }
}

#[cfg(not(target_arch = "wasm32"))]
struct SearchClock {
    started: Instant,
    deadline: Option<Instant>,
}

#[cfg(not(target_arch = "wasm32"))]
impl SearchClock {
    fn new(milliseconds: u32) -> Self {
        let started = Instant::now();
        Self {
            started,
            deadline: (milliseconds > 0)
                .then(|| started + Duration::from_millis(u64::from(milliseconds))),
        }
    }

    fn expired(&self) -> bool {
        self.deadline
            .is_some_and(|deadline| Instant::now() >= deadline)
    }

    fn elapsed_ms(&self) -> f64 {
        self.started.elapsed().as_secs_f64() * 1_000.0
    }
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = performance, js_name = now)]
    fn performance_now() -> f64;
}

#[cfg(target_arch = "wasm32")]
struct SearchClock {
    started: f64,
    deadline: Option<f64>,
}

#[cfg(target_arch = "wasm32")]
impl SearchClock {
    fn new(milliseconds: u32) -> Self {
        let started = performance_now();
        Self {
            started,
            deadline: (milliseconds > 0).then(|| started + f64::from(milliseconds)),
        }
    }

    fn expired(&self) -> bool {
        self.deadline
            .is_some_and(|deadline| performance_now() >= deadline)
    }

    fn elapsed_ms(&self) -> f64 {
        (performance_now() - self.started).max(0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mv(coord: &str) -> Move {
        coord.parse().unwrap()
    }

    #[test]
    fn alpha_beta_finds_immediate_winning_road_completion() {
        // Red is one move away from connecting Top (d1) to Bottom (d7) by playing d7 (which pinches d6)
        let pos = Position::from_moves(&[
            mv("d1"),
            mv("a1"),
            mv("a2"),
            mv("d2"),
            mv("d3"),
            mv("a3"),
            mv("a4"),
            mv("d4"),
            mv("d5"),
            mv("a5"),
            mv("a6"),
            mv("d6"),
        ])
        .unwrap();
        let mut searcher = Searcher::new();
        let report = searcher.search(
            pos,
            SearchOptions {
                max_depth: 3,
                node_limit: 5_000,
                soft_time_ms: 0,
            },
        );
        assert_eq!(report.best_move, Some(mv("d7")));
        assert!(report.score >= MATE - 15);
    }

    #[test]
    fn mate_scores_round_trip_through_transposition_encoding() {
        for ply in [0, 1, 14, 48] {
            for score in [MATE - 3, -MATE + 3, 250, -250] {
                assert_eq!(decode_score(encode_score(score, ply), ply), score);
            }
        }
    }
}
