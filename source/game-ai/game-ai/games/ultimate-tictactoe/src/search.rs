use crate::Move;
use crate::lookup::{MiniInfo, MiniTable};
use crate::position::{GameResult, LINES, MiniResult, Player, Position, has_line};
#[cfg(not(target_arch = "wasm32"))]
use std::time::{Duration, Instant};

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

const INF: i32 = 32_000;
const MATE: i32 = 30_000;
const DEFAULT_TT_ENTRIES: usize = 1 << 16;
const MAX_PLY: usize = 81;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// Limits for one iterative-deepening search. Zero disables the soft time limit.
pub struct SearchOptions {
    pub max_depth: u8,
    pub node_limit: u64,
    pub soft_time_ms: u32,
}

impl Default for SearchOptions {
    fn default() -> Self {
        SEARCH_PRESETS[SEARCH_PRESETS.len() - 1].options
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SearchPreset {
    pub name: &'static str,
    pub options: SearchOptions,
}

pub const SEARCH_PRESETS: [SearchPreset; 6] = [
    SearchPreset {
        name: "beginner",
        options: SearchOptions {
            max_depth: 1,
            node_limit: 500,
            soft_time_ms: 25,
        },
    },
    SearchPreset {
        name: "easy",
        options: SearchOptions {
            max_depth: 2,
            node_limit: 2_000,
            soft_time_ms: 40,
        },
    },
    SearchPreset {
        name: "medium",
        options: SearchOptions {
            max_depth: 3,
            node_limit: 10_000,
            soft_time_ms: 80,
        },
    },
    SearchPreset {
        name: "hard",
        options: SearchOptions {
            max_depth: 5,
            node_limit: 75_000,
            soft_time_ms: 250,
        },
    },
    SearchPreset {
        name: "expert",
        options: SearchOptions {
            max_depth: 7,
            node_limit: 300_000,
            soft_time_ms: 650,
        },
    },
    SearchPreset {
        name: "maximum",
        options: SearchOptions {
            max_depth: 20,
            node_limit: 900_000,
            soft_time_ms: 1_000,
        },
    },
];

pub fn search_preset(name: &str) -> Option<SearchPreset> {
    SEARCH_PRESETS
        .iter()
        .copied()
        .find(|preset| preset.name == name)
}

#[derive(Clone, Debug, Eq, PartialEq)]
/// The last fully completed iteration and its principal variation.
pub struct SearchReport {
    pub best_move: Option<Move>,
    pub score: i32,
    pub depth: u8,
    pub nodes: u64,
    pub tt_hits: u64,
    pub cutoffs: u64,
    pub principal_variation: Vec<Move>,
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

/// Reusable PVS alpha-beta state, including the mini-board and transposition tables.
pub struct Searcher {
    mini: MiniTable,
    table: Vec<Entry>,
    history: [[i32; 81]; 2],
    nodes: u64,
    node_limit: u64,
    deadline: Deadline,
    tt_hits: u64,
    cutoffs: u64,
}

impl Default for Searcher {
    fn default() -> Self {
        Self::new()
    }
}

impl Searcher {
    pub fn new() -> Self {
        Self {
            mini: MiniTable::build(),
            table: vec![Entry::default(); DEFAULT_TT_ENTRIES],
            history: [[0; 81]; 2],
            nodes: 0,
            node_limit: 0,
            deadline: Deadline::disabled(),
            tt_hits: 0,
            cutoffs: 0,
        }
    }

    /// Searches one depth at a time and preserves the last result when a limit interrupts search.
    pub fn search(&mut self, position: Position, options: SearchOptions) -> SearchReport {
        self.nodes = 0;
        self.tt_hits = 0;
        self.cutoffs = 0;
        self.node_limit = options.node_limit.max(1);
        self.deadline = Deadline::after(options.soft_time_ms);
        self.age_history();

        let legal = position.legal_moves();
        let fallback = legal.iter().next();
        if fallback.is_none() {
            return SearchReport {
                best_move: None,
                score: terminal_score(position, 0).unwrap_or(0),
                depth: 0,
                nodes: 0,
                tt_hits: 0,
                cutoffs: 0,
                principal_variation: Vec::new(),
            };
        }

        let mut completed_depth = 0;
        let mut best_move = fallback;
        let mut best_score = evaluate_position(&self.mini, position);
        for depth in 1..=options.max_depth.max(1) {
            if self.deadline.expired() {
                break;
            }
            let Some((mv, score)) = self.search_root(position, depth) else {
                break;
            };
            best_move = Some(mv);
            best_score = score;
            completed_depth = depth;
            if score.abs() >= MATE - MAX_PLY as i32 {
                break;
            }
        }

        SearchReport {
            best_move,
            score: best_score,
            depth: completed_depth,
            nodes: self.nodes,
            tt_hits: self.tt_hits,
            cutoffs: self.cutoffs,
            principal_variation: self.principal_variation(position, completed_depth),
        }
    }

    fn search_root(&mut self, position: Position, depth: u8) -> Option<(Move, i32)> {
        let key = position.hash();
        let tt_move = self.probe(key).and_then(|entry| entry.best_move);
        let mut moves = self.ordered_moves(position, tt_move);
        let mut alpha = -INF;
        let beta = INF;
        let mut best_move = moves.first().map(|item| item.0)?;
        let mut best_score = -INF;

        for (index, (mv, _)) in moves.drain(..).enumerate() {
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
            if score > best_score {
                best_score = score;
                best_move = mv;
            }
            alpha = alpha.max(score);
        }

        self.store(key, depth, best_score, Bound::Exact, Some(best_move), 0);
        Some((best_move, best_score))
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
            return self.threat_search(position, ply, alpha, beta, 2);
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
                self.history[position.side_to_move().index()][mv.global_index() as usize] +=
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

    // Wins, blocks, forks, and free-choice transitions are this game's captures.
    fn threat_search(
        &mut self,
        position: Position,
        ply: u8,
        mut alpha: i32,
        beta: i32,
        remaining: u8,
    ) -> Option<i32> {
        let stand_pat = evaluate_position(&self.mini, position);
        if remaining == 0 || stand_pat >= beta {
            return Some(stand_pat);
        }
        alpha = alpha.max(stand_pat);
        let moves = self.noisy_moves(position);
        if moves.is_empty() {
            return Some(stand_pat);
        }
        for mv in moves {
            if !self.visit_node() {
                return None;
            }
            let child = position.play(mv).expect("noisy moves are legal");
            let score = if let Some(terminal) = terminal_score(child, ply + 1) {
                -terminal
            } else {
                -self.threat_search(child, ply + 1, -beta, -alpha, remaining - 1)?
            };
            if score >= beta {
                return Some(score);
            }
            alpha = alpha.max(score);
        }
        Some(alpha)
    }

    fn ordered_moves(&self, position: Position, tt_move: Option<Move>) -> Vec<(Move, i32)> {
        let side = position.side_to_move();
        let opponent = side.other();
        let mut moves: Vec<_> = position
            .legal_moves()
            .iter()
            .map(|mv| {
                let board = mv.board();
                let cell_bit = 1 << mv.cell();
                let (x, o) = position.mini_masks(board);
                let info = self.mini.get(x, o);
                let child = position.play(mv).expect("generated move is legal");
                let mut score = self.history[side.index()][mv.global_index() as usize];
                if Some(mv) == tt_move {
                    score += 35_000;
                }
                if child.result() == GameResult::Win(side) {
                    score += 1_000_000;
                }
                if info.winning_moves[side.index()] & cell_bit != 0 {
                    score += 90_000 + macro_board_weight(board) * 2_000;
                }
                if info.winning_moves[opponent.index()] & cell_bit != 0 {
                    score += 55_000;
                    if would_complete_macro(position, opponent, board) {
                        score += 500_000;
                    }
                }
                if info.fork_moves[side.index()] & cell_bit != 0 {
                    score += 18_000;
                }
                score += routing_order_score(&self.mini, child, side);
                (mv, score)
            })
            .collect();
        moves.sort_unstable_by(|left, right| {
            right
                .1
                .cmp(&left.1)
                .then_with(|| left.0.global_index().cmp(&right.0.global_index()))
        });
        moves
    }

    fn noisy_moves(&self, position: Position) -> Vec<Move> {
        let side = position.side_to_move();
        let opponent = side.other();
        let mut noisy: Vec<(Move, i32)> = position
            .legal_moves()
            .iter()
            .filter_map(|mv| {
                let (x, o) = position.mini_masks(mv.board());
                let info = self.mini.get(x, o);
                let bit = 1 << mv.cell();
                let child = position.play(mv).ok()?;
                let global = child.result() == GameResult::Win(side);
                let local = info.winning_moves[side.index()] & bit != 0;
                let block = info.winning_moves[opponent.index()] & bit != 0;
                let fork = info.fork_moves[side.index()] & bit != 0;
                let free_choice = child.active_board().is_none();
                if !(global || local || block || fork || free_choice) {
                    return None;
                }
                let score = if global {
                    100_000
                } else if local {
                    10_000
                } else if block {
                    8_000
                } else if fork {
                    5_000
                } else {
                    1_000
                };
                Some((mv, score))
            })
            .collect();
        noisy.sort_unstable_by_key(|item| std::cmp::Reverse(item.1));
        noisy.truncate(12);
        noisy.into_iter().map(|item| item.0).collect()
    }

    fn visit_node(&mut self) -> bool {
        if self.nodes >= self.node_limit {
            return false;
        }
        self.nodes += 1;
        if self.nodes & 255 == 0 && self.deadline.expired() {
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
            position = position.play(mv).expect("PV move was checked as legal");
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

#[cfg(not(target_arch = "wasm32"))]
struct Deadline(Option<Instant>);

#[cfg(not(target_arch = "wasm32"))]
impl Deadline {
    fn disabled() -> Self {
        Self(None)
    }

    fn after(milliseconds: u32) -> Self {
        Self(
            (milliseconds > 0)
                .then(|| Instant::now() + Duration::from_millis(u64::from(milliseconds))),
        )
    }

    fn expired(&self) -> bool {
        self.0.is_some_and(|deadline| Instant::now() >= deadline)
    }
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = performance, js_name = now)]
    fn performance_now() -> f64;
}

#[cfg(target_arch = "wasm32")]
struct Deadline(Option<f64>);

#[cfg(target_arch = "wasm32")]
impl Deadline {
    fn disabled() -> Self {
        Self(None)
    }

    fn after(milliseconds: u32) -> Self {
        Self((milliseconds > 0).then(|| performance_now() + f64::from(milliseconds)))
    }

    fn expired(&self) -> bool {
        self.0.is_some_and(|deadline| performance_now() >= deadline)
    }
}

fn terminal_score(position: Position, ply: u8) -> Option<i32> {
    match position.result() {
        GameResult::Ongoing => None,
        GameResult::Draw => Some(0),
        GameResult::Win(winner) if winner == position.side_to_move() => Some(MATE - i32::from(ply)),
        GameResult::Win(_) => Some(-MATE + i32::from(ply)),
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

pub(crate) fn evaluate_position(table: &MiniTable, position: Position) -> i32 {
    let (macro_x, macro_o, macro_drawn) = position.macro_masks();
    let mut absolute = macro_score(macro_x, macro_o, macro_drawn);
    const BOARD_WEIGHT: [i32; 9] = [3, 2, 3, 2, 4, 2, 3, 2, 3];
    for board in 0..9 {
        if position.mini_result(board) != MiniResult::Open {
            continue;
        }
        let (x, o) = position.mini_masks(board);
        let info = table.get(x, o);
        debug_assert!(info.winner.is_none() && !info.drawn);
        absolute += BOARD_WEIGHT[board as usize]
            * (i32::from(info.potential[Player::X.index()])
                - i32::from(info.potential[Player::O.index()]));
    }

    let side = position.side_to_move();
    let opponent = side.other();
    let routing = if let Some(board) = position.active_board() {
        let (x, o) = position.mini_masks(board);
        let info = table.get(x, o);
        let our_wins = info.winning_moves[side.index()].count_ones() as i32;
        let their_wins = info.winning_moves[opponent.index()].count_ones() as i32;
        let our_forks = info.fork_moves[side.index()].count_ones() as i32;
        95 * our_wins - 80 * their_wins + 35 * our_forks + info.empty.count_ones() as i32
    } else {
        30 + (position.legal_moves().len() as i32).min(40)
    };

    if side == Player::X {
        absolute + routing
    } else {
        -absolute + routing
    }
}

fn macro_score(x: u16, o: u16, drawn: u16) -> i32 {
    let mut score = 0;
    const BOARD_WEIGHT: [i32; 9] = [3, 2, 3, 2, 4, 2, 3, 2, 3];
    for (board, weight) in BOARD_WEIGHT.into_iter().enumerate() {
        let bit = 1 << board;
        if x & bit != 0 {
            score += 420 * weight;
        }
        if o & bit != 0 {
            score -= 420 * weight;
        }
    }
    for line in LINES {
        let x_count = (x & line).count_ones();
        let o_count = (o & line).count_ones();
        if o_count == 0 && drawn & line == 0 {
            score += match x_count {
                1 => 90,
                2 => 900,
                _ => 0,
            };
        }
        if x_count == 0 && drawn & line == 0 {
            score -= match o_count {
                1 => 90,
                2 => 900,
                _ => 0,
            };
        }
    }
    score
}

fn macro_board_weight(board: u8) -> i32 {
    match board {
        4 => 4,
        0 | 2 | 6 | 8 => 3,
        _ => 2,
    }
}

fn would_complete_macro(position: Position, player: Player, board: u8) -> bool {
    let (x, o, _) = position.macro_masks();
    let mask = match player {
        Player::X => x,
        Player::O => o,
    };
    has_line(mask | (1 << board))
}

fn routing_order_score(table: &MiniTable, child: Position, mover: Player) -> i32 {
    let Some(board) = child.active_board() else {
        return -22_000;
    };
    let (x, o) = child.mini_masks(board);
    let info: MiniInfo = table.get(x, o);
    let opponent = mover.other();
    let danger = info.winning_moves[opponent.index()].count_ones() as i32;
    let pressure = info.winning_moves[mover.index()].count_ones() as i32;
    -14_000 * danger + 2_500 * pressure - info.empty.count_ones() as i32 * 20
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drawn_boards_block_macro_line_potential() {
        let two_x = (1 << 0) | (1 << 1);
        let open = macro_score(two_x, 0, 0);
        let blocked = macro_score(two_x, 0, 1 << 2);
        assert!(open - blocked >= 900);
    }

    #[test]
    fn mate_scores_survive_transposition_table_distance_adjustment() {
        for ply in [0, 1, 17, 80] {
            for score in [MATE - 2, -MATE + 2, 417, -417] {
                assert_eq!(decode_score(encode_score(score, ply), ply), score);
            }
        }
    }
}
