use crate::lookup::{MiniInfo, MiniTable};
use crate::network::PolicyNetwork;
use crate::position::has_line;
use crate::search::evaluate_position;
use crate::{GameResult, MiniResult, Move, Player, Position};
use std::cmp::Ordering;
#[cfg(not(target_arch = "wasm32"))]
use std::time::{Duration, Instant};

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

const DEFAULT_EXPLORATION: f64 = std::f64::consts::SQRT_2;
const TACTICAL_ROLLOUT_FREQUENCY: usize = 4;
const VALUE_SCALE: f64 = 4_000.0;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
/// A complete MCTS strategy with no ignored or incompatible settings.
pub enum MctsStrategy {
    UctRandom,
    UctTactical,
    PuctHandcrafted,
    #[default]
    PuctLearned,
}

impl MctsStrategy {
    pub const fn name(self) -> &'static str {
        match self {
            Self::UctRandom => "random-uct",
            Self::UctTactical => "tactical-uct",
            Self::PuctHandcrafted => "handcrafted-puct",
            Self::PuctLearned => "learned-puct",
        }
    }

    const fn is_puct(self) -> bool {
        matches!(self, Self::PuctHandcrafted | Self::PuctLearned)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
/// Work limits, strategy, exploration constant, and deterministic seed.
pub struct MctsOptions {
    pub max_simulations: u32,
    pub soft_time_ms: u32,
    pub exploration: f64,
    pub seed: u64,
    pub strategy: MctsStrategy,
}

impl Default for MctsOptions {
    fn default() -> Self {
        MCTS_PRESETS[MCTS_PRESETS.len() - 1].options
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MctsPreset {
    pub name: &'static str,
    pub options: MctsOptions,
}

pub const MCTS_PRESETS: [MctsPreset; 6] = [
    preset("beginner", 100, 25),
    preset("easy", 500, 50),
    preset("medium", 2_000, 100),
    preset("hard", 10_000, 250),
    preset("expert", 40_000, 650),
    preset("maximum", 100_000, 1_000),
];

const fn preset(name: &'static str, max_simulations: u32, soft_time_ms: u32) -> MctsPreset {
    MctsPreset {
        name,
        options: MctsOptions {
            max_simulations,
            soft_time_ms,
            exploration: DEFAULT_EXPLORATION,
            seed: 1,
            strategy: MctsStrategy::PuctLearned,
        },
    }
}

pub fn mcts_preset(name: &str) -> Option<MctsPreset> {
    MCTS_PRESETS
        .iter()
        .copied()
        .find(|preset| preset.name == name)
}

#[derive(Clone, Debug, PartialEq)]
pub struct MctsMoveStats {
    pub mv: Move,
    pub visits: u32,
    /// Policy probability before any simulations were run.
    pub prior: f64,
    /// Expected score where a win is 1, a draw is 0.5, and a loss is 0.
    pub expected_score: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MctsReport {
    pub best_move: Option<Move>,
    pub simulations: u32,
    pub tree_nodes: u32,
    pub root_visits: u32,
    pub expected_score: f64,
    pub elapsed_ms: u32,
    pub strategy: MctsStrategy,
    pub rollout_moves: u64,
    pub leaf_evaluations: u32,
    pub root_moves: Vec<MctsMoveStats>,
}

#[derive(Clone, Copy)]
struct PendingMove {
    mv: Move,
    prior: f64,
}

struct Node {
    position: Position,
    parent: Option<usize>,
    incoming_move: Option<Move>,
    children: Vec<usize>,
    unexpanded: Vec<PendingMove>,
    prior: f64,
    visits: u32,
    value_sum: f64,
}

impl Node {
    fn new(
        position: Position,
        parent: Option<usize>,
        incoming_move: Option<Move>,
        prior: f64,
        table: &MiniTable,
        network: Option<&PolicyNetwork>,
        strategy: MctsStrategy,
    ) -> Self {
        let unexpanded = move_priors(table, network, position, strategy);
        Self {
            position,
            parent,
            incoming_move,
            children: Vec::new(),
            unexpanded,
            prior,
            visits: 0,
            value_sum: 0.0,
        }
    }
}

/// A reusable MCTS searcher supporting UCT rollouts and policy-guided PUCT.
pub struct MctsSearcher {
    nodes: Vec<Node>,
    random: SplitMix64,
    mini: MiniTable,
    policy: PolicyNetwork,
}

impl Default for MctsSearcher {
    fn default() -> Self {
        Self::new()
    }
}

impl MctsSearcher {
    pub fn new() -> Self {
        Self {
            nodes: Vec::with_capacity(4_096),
            random: SplitMix64(1),
            mini: MiniTable::build(),
            policy: PolicyNetwork::embedded().expect("embedded policy artifact is valid"),
        }
    }

    pub fn search(&mut self, position: Position, options: MctsOptions) -> MctsReport {
        self.nodes.clear();
        self.random = SplitMix64(options.seed);
        let root = Node::new(
            position,
            None,
            None,
            1.0,
            &self.mini,
            self.network(options.strategy),
            options.strategy,
        );
        self.nodes.push(root);
        let root_player = position.side_to_move();
        let clock = Clock::new(options.soft_time_ms);
        let mut simulations = 0;
        let mut rollout_moves = 0;
        let mut leaf_evaluations = 0;

        if position.result() == GameResult::Ongoing {
            for simulation in 0..options.max_simulations.max(1) {
                if simulation > 0 && simulation % 16 == 0 && clock.expired() {
                    break;
                }
                let work = self.run_simulation(
                    root_player,
                    options.exploration.max(0.0),
                    options.strategy,
                );
                rollout_moves += u64::from(work.rollout_moves);
                leaf_evaluations += u32::from(work.leaf_evaluated);
                simulations += 1;
            }
        }

        let mut root_moves = self.nodes[0]
            .children
            .iter()
            .map(|&index| {
                let child = &self.nodes[index];
                MctsMoveStats {
                    mv: child.incoming_move.expect("root children have moves"),
                    visits: child.visits,
                    prior: child.prior,
                    expected_score: expected_score(child.value_sum, child.visits),
                }
            })
            .collect::<Vec<_>>();
        root_moves.sort_by(|left, right| {
            right
                .visits
                .cmp(&left.visits)
                .then_with(|| right.expected_score.total_cmp(&left.expected_score))
                .then_with(|| left.mv.global_index().cmp(&right.mv.global_index()))
        });
        let best_move = root_moves.first().map(|stats| stats.mv);
        let expected_score = root_moves.first().map_or(0.5, |stats| stats.expected_score);

        MctsReport {
            best_move,
            simulations,
            tree_nodes: self.nodes.len() as u32,
            root_visits: self.nodes[0].visits,
            expected_score,
            elapsed_ms: clock.elapsed_ms(),
            strategy: options.strategy,
            rollout_moves,
            leaf_evaluations,
            root_moves,
        }
    }

    fn run_simulation(
        &mut self,
        root_player: Player,
        exploration: f64,
        strategy: MctsStrategy,
    ) -> SimulationWork {
        let mut node = 0;
        loop {
            if self.nodes[node].position.result() != GameResult::Ongoing {
                break;
            }
            match strategy {
                MctsStrategy::UctRandom | MctsStrategy::UctTactical => {
                    if !self.nodes[node].unexpanded.is_empty() {
                        node = self.expand_random(node, strategy);
                        break;
                    }
                    if self.nodes[node].children.is_empty() {
                        break;
                    }
                    node = self.select_uct_child(node, root_player, exploration);
                }
                MctsStrategy::PuctHandcrafted | MctsStrategy::PuctLearned => {
                    match self.select_puct_action(node, root_player, exploration) {
                        PuctAction::Expand(index) => {
                            node = self.expand_at(node, index, strategy);
                            break;
                        }
                        PuctAction::Descend(child) => node = child,
                        PuctAction::None => break,
                    }
                }
            }
        }

        let (value, work) = match strategy {
            MctsStrategy::UctRandom | MctsStrategy::UctTactical => {
                let (value, rollout_moves) = self.rollout(
                    self.nodes[node].position,
                    root_player,
                    strategy == MctsStrategy::UctTactical,
                );
                (
                    value,
                    SimulationWork {
                        rollout_moves,
                        leaf_evaluated: false,
                    },
                )
            }
            MctsStrategy::PuctHandcrafted | MctsStrategy::PuctLearned => (
                self.leaf_value(node, root_player),
                SimulationWork {
                    rollout_moves: 0,
                    leaf_evaluated: true,
                },
            ),
        };
        let mut current = Some(node);
        while let Some(index) = current {
            self.nodes[index].visits += 1;
            self.nodes[index].value_sum += value;
            current = self.nodes[index].parent;
        }
        work
    }

    fn expand_random(&mut self, parent: usize, strategy: MctsStrategy) -> usize {
        let move_index = self.random.index(self.nodes[parent].unexpanded.len());
        self.expand_at(parent, move_index, strategy)
    }

    fn expand_at(&mut self, parent: usize, move_index: usize, strategy: MctsStrategy) -> usize {
        let pending = self.nodes[parent].unexpanded.swap_remove(move_index);
        let mv = pending.mv;
        let position = self.nodes[parent]
            .position
            .play(mv)
            .expect("tree expansion uses legal moves");
        let child = self.nodes.len();
        let child_node = Node::new(
            position,
            Some(parent),
            Some(mv),
            pending.prior,
            &self.mini,
            self.network(strategy),
            strategy,
        );
        self.nodes.push(child_node);
        self.nodes[parent].children.push(child);
        child
    }

    fn select_uct_child(&self, parent: usize, root_player: Player, exploration: f64) -> usize {
        let parent_node = &self.nodes[parent];
        let maximize_root = parent_node.position.side_to_move() == root_player;
        let log_parent = f64::from(parent_node.visits.max(1)).ln();
        let mut best = parent_node.children[0];
        let mut best_score = f64::NEG_INFINITY;

        for &child_index in &parent_node.children {
            let child = &self.nodes[child_index];
            let mean = child.value_sum / f64::from(child.visits.max(1));
            let exploitation = if maximize_root { mean } else { -mean };
            let score =
                exploitation + exploration * (log_parent / f64::from(child.visits.max(1))).sqrt();
            let move_index = child
                .incoming_move
                .expect("non-root nodes have moves")
                .global_index();
            let best_move_index = self.nodes[best]
                .incoming_move
                .expect("non-root nodes have moves")
                .global_index();
            if score > best_score || (score == best_score && move_index < best_move_index) {
                best = child_index;
                best_score = score;
            }
        }
        best
    }

    fn select_puct_action(
        &self,
        parent: usize,
        root_player: Player,
        exploration: f64,
    ) -> PuctAction {
        let parent_node = &self.nodes[parent];
        let maximize_root = parent_node.position.side_to_move() == root_player;
        let parent_scale = (f64::from(parent_node.visits) + 1.0).sqrt();
        let mut best_action = PuctAction::None;
        let mut best_move = u8::MAX;
        let mut best_score = f64::NEG_INFINITY;

        for (index, pending) in parent_node.unexpanded.iter().enumerate() {
            let score = exploration * pending.prior * parent_scale;
            update_puct_choice(
                &mut best_action,
                &mut best_move,
                &mut best_score,
                PuctAction::Expand(index),
                pending.mv,
                score,
            );
        }
        for &child_index in &parent_node.children {
            let child = &self.nodes[child_index];
            let mean = child.value_sum / f64::from(child.visits.max(1));
            let exploitation = if maximize_root { mean } else { -mean };
            let score = exploitation
                + exploration * child.prior * parent_scale / (1.0 + f64::from(child.visits));
            update_puct_choice(
                &mut best_action,
                &mut best_move,
                &mut best_score,
                PuctAction::Descend(child_index),
                child.incoming_move.expect("non-root nodes have moves"),
                score,
            );
        }
        best_action
    }

    fn leaf_value(&self, node: usize, root_player: Player) -> f64 {
        let position = self.nodes[node].position;
        match position.result() {
            GameResult::Win(winner) if winner == root_player => 1.0,
            GameResult::Win(_) => -1.0,
            GameResult::Draw => 0.0,
            GameResult::Ongoing => {
                let side_value = f64::from(evaluate_position(&self.mini, position));
                let bounded = side_value / (side_value.abs() + VALUE_SCALE);
                if position.side_to_move() == root_player {
                    bounded
                } else {
                    -bounded
                }
            }
        }
    }

    fn rollout(
        &mut self,
        mut position: Position,
        root_player: Player,
        tactical: bool,
    ) -> (f64, u32) {
        let mut plies = 0;
        while position.result() == GameResult::Ongoing {
            let moves = position.legal_moves();
            let mv = match tactical {
                true if self.random.index(TACTICAL_ROLLOUT_FREQUENCY) == 0 => {
                    self.tactical_move(position, &moves)
                }
                _ => moves
                    .iter()
                    .nth(self.random.index(moves.len()))
                    .expect("ongoing positions have legal moves"),
            };
            position = position.play(mv).expect("rollouts use legal moves");
            plies += 1;
        }
        let value = match position.result() {
            GameResult::Win(winner) if winner == root_player => 1.0,
            GameResult::Win(_) => -1.0,
            GameResult::Draw => 0.0,
            GameResult::Ongoing => unreachable!(),
        };
        (value, plies)
    }

    fn tactical_move(&mut self, position: Position, moves: &crate::MoveList) -> Move {
        let mut best_move = None;
        let mut best_score = i32::MIN;
        let mut tied = 0;
        for mv in moves.iter() {
            let score = self.tactical_score(position, mv);
            match score.cmp(&best_score) {
                Ordering::Greater => {
                    best_move = Some(mv);
                    best_score = score;
                    tied = 1;
                }
                Ordering::Equal => {
                    tied += 1;
                    if self.random.index(tied) == 0 {
                        best_move = Some(mv);
                    }
                }
                Ordering::Less => {}
            }
        }
        best_move.expect("ongoing positions have legal moves")
    }

    fn tactical_score(&self, position: Position, mv: Move) -> i32 {
        let side = position.side_to_move();
        let opponent = side.other();
        let board = mv.board();
        let bit = 1 << mv.cell();
        let (x, o) = position.mini_masks(board);
        let info = self.mini.get(x, o);
        let child = position.play(mv).expect("rollout scoring uses legal moves");
        let mut score = 0;
        if child.result() == GameResult::Win(side) {
            score += 2_000_000;
        }
        if info.winning_moves[side.index()] & bit != 0 {
            score += 100_000 + macro_board_weight(board) * 2_000;
        }
        if info.winning_moves[opponent.index()] & bit != 0 {
            score += 60_000;
            if would_complete_macro(position, opponent, board) {
                score += 600_000;
            }
        }
        if info.fork_moves[side.index()] & bit != 0 {
            score += 20_000;
        }
        score + self.routing_score(child, side)
    }

    fn routing_score(&self, child: Position, mover: Player) -> i32 {
        let opponent = mover.other();
        let Some(board) = child.active_board() else {
            let immediate_macro_loss = (0..9).any(|board| {
                child.mini_result(board) == MiniResult::Open
                    && would_complete_macro(child, opponent, board)
                    && self.mini_info(child, board).winning_moves[opponent.index()] != 0
            });
            return if immediate_macro_loss {
                -700_000
            } else {
                -22_000
            };
        };
        let info = self.mini_info(child, board);
        let macro_danger = info.winning_moves[opponent.index()] != 0
            && would_complete_macro(child, opponent, board);
        if macro_danger { -700_000 } else { 0 }
    }

    fn mini_info(&self, position: Position, board: u8) -> MiniInfo {
        let (x, o) = position.mini_masks(board);
        self.mini.get(x, o)
    }

    fn network(&self, strategy: MctsStrategy) -> Option<&PolicyNetwork> {
        (strategy == MctsStrategy::PuctLearned).then_some(&self.policy)
    }
}

#[derive(Clone, Copy)]
enum PuctAction {
    Expand(usize),
    Descend(usize),
    None,
}

struct SimulationWork {
    rollout_moves: u32,
    leaf_evaluated: bool,
}

fn update_puct_choice(
    best_action: &mut PuctAction,
    best_move: &mut u8,
    best_score: &mut f64,
    action: PuctAction,
    mv: Move,
    score: f64,
) {
    let move_index = mv.global_index();
    if score > *best_score || (score == *best_score && move_index < *best_move) {
        *best_action = action;
        *best_move = move_index;
        *best_score = score;
    }
}

fn move_priors(
    table: &MiniTable,
    network: Option<&PolicyNetwork>,
    position: Position,
    strategy: MctsStrategy,
) -> Vec<PendingMove> {
    let moves = position.legal_moves().iter().collect::<Vec<_>>();
    if moves.is_empty() {
        return Vec::new();
    }
    if !strategy.is_puct() {
        let prior = 1.0 / moves.len() as f64;
        return moves
            .into_iter()
            .map(|mv| PendingMove { mv, prior })
            .collect();
    }

    if strategy == MctsStrategy::PuctLearned {
        let priors = network
            .expect("learned PUCT requires the embedded policy")
            .predict(position);
        return moves
            .into_iter()
            .map(|mv| PendingMove {
                mv,
                prior: priors[mv.global_index() as usize],
            })
            .collect();
    }

    let scores = moves
        .iter()
        .copied()
        .map(|mv| policy_score(table, position, mv))
        .collect::<Vec<_>>();
    let total = scores
        .iter()
        .map(|score| f64::from((*score).max(1)))
        .sum::<f64>();
    moves
        .into_iter()
        .zip(scores)
        .map(|(mv, score)| PendingMove {
            mv,
            prior: f64::from(score.max(1)) / total,
        })
        .collect()
}

fn policy_score(table: &MiniTable, position: Position, mv: Move) -> i32 {
    let side = position.side_to_move();
    let opponent = side.other();
    let board = mv.board();
    let bit = 1 << mv.cell();
    let (x, o) = position.mini_masks(board);
    let info = table.get(x, o);
    let child = position.play(mv).expect("policy scores legal moves");
    let mut score = macro_board_weight(board) * 24 + cell_weight(mv.cell()) * 18;

    if child.result() == GameResult::Win(side) {
        score += 2_000_000;
    }
    if info.winning_moves[side.index()] & bit != 0 {
        score += 160_000 + macro_board_weight(board) * 3_000;
    }
    if info.winning_moves[opponent.index()] & bit != 0 {
        score += 90_000;
        if would_complete_macro(position, opponent, board) {
            score += 700_000;
        }
    }
    if info.fork_moves[side.index()] & bit != 0 {
        score += 30_000;
    }
    score + policy_routing_score(table, child, side)
}

fn policy_routing_score(table: &MiniTable, child: Position, mover: Player) -> i32 {
    let opponent = mover.other();
    let Some(board) = child.active_board() else {
        let immediate_macro_loss = (0..9).any(|board| {
            child.mini_result(board) == MiniResult::Open
                && would_complete_macro(child, opponent, board)
                && mini_info(table, child, board).winning_moves[opponent.index()] != 0
        });
        return if immediate_macro_loss {
            -900_000
        } else {
            -18_000
        };
    };
    let info = mini_info(table, child, board);
    let opponent_wins = info.winning_moves[opponent.index()].count_ones() as i32;
    if opponent_wins > 0 && would_complete_macro(child, opponent, board) {
        return -900_000;
    }
    let our_wins = info.winning_moves[mover.index()].count_ones() as i32;
    -22_000 * opponent_wins + 4_000 * our_wins - info.empty.count_ones() as i32 * 4
}

fn mini_info(table: &MiniTable, position: Position, board: u8) -> MiniInfo {
    let (x, o) = position.mini_masks(board);
    table.get(x, o)
}

fn cell_weight(cell: u8) -> i32 {
    match cell {
        4 => 4,
        0 | 2 | 6 | 8 => 3,
        _ => 2,
    }
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

fn expected_score(value_sum: f64, visits: u32) -> f64 {
    if visits == 0 {
        0.5
    } else {
        (value_sum / f64::from(visits) + 1.0) * 0.5
    }
}

struct SplitMix64(u64);

impl SplitMix64 {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut value = self.0;
        value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        value ^ (value >> 31)
    }

    fn index(&mut self, len: usize) -> usize {
        debug_assert!(len > 0);
        (self.next() as usize) % len
    }
}

#[cfg(not(target_arch = "wasm32"))]
struct Clock {
    started: Instant,
    deadline: Option<Instant>,
}

#[cfg(not(target_arch = "wasm32"))]
impl Clock {
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

    fn elapsed_ms(&self) -> u32 {
        self.started.elapsed().as_millis().min(u128::from(u32::MAX)) as u32
    }
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = performance, js_name = now)]
    fn performance_now() -> f64;
}

#[cfg(target_arch = "wasm32")]
struct Clock {
    started: f64,
    deadline: Option<f64>,
}

#[cfg(target_arch = "wasm32")]
impl Clock {
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

    fn elapsed_ms(&self) -> u32 {
        (performance_now() - self.started).clamp(0.0, f64::from(u32::MAX)) as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tactical_position(active_board: u8) -> Position {
        let mut x = [0; 9];
        let mut o = [0; 9];
        o[0] = 0b000_000_111;
        o[1] = 0b000_000_111;
        o[2] = 0b000_000_011;
        x[0] = 0b000_011_000;
        x[1] = 0b000_011_000;
        x[2] = 0b010_001_000;
        x[3] = 0b000_000_011;
        Position::from_cells(x, o, Some(active_board), Player::X).unwrap()
    }

    #[test]
    fn opponent_nodes_select_the_move_with_the_worst_root_value() {
        let table = MiniTable::build();
        let parent_position = Position::start().play(Move::new(4, 4)).unwrap();
        let moves = parent_position
            .legal_moves()
            .iter()
            .take(2)
            .collect::<Vec<_>>();
        let mut parent = Node::new(
            parent_position,
            None,
            None,
            1.0,
            &table,
            None,
            MctsStrategy::UctRandom,
        );
        parent.unexpanded.clear();
        parent.children = vec![1, 2];
        parent.visits = 20;
        let mut good_for_root = Node::new(
            parent_position.play(moves[0]).unwrap(),
            Some(0),
            Some(moves[0]),
            0.5,
            &table,
            None,
            MctsStrategy::UctRandom,
        );
        good_for_root.visits = 10;
        good_for_root.value_sum = 8.0;
        let mut bad_for_root = Node::new(
            parent_position.play(moves[1]).unwrap(),
            Some(0),
            Some(moves[1]),
            0.5,
            &table,
            None,
            MctsStrategy::UctRandom,
        );
        bad_for_root.visits = 10;
        bad_for_root.value_sum = -8.0;
        let searcher = MctsSearcher {
            nodes: vec![parent, good_for_root, bad_for_root],
            random: SplitMix64(1),
            mini: MiniTable::build(),
            policy: PolicyNetwork::embedded().unwrap(),
        };

        assert_eq!(searcher.select_uct_child(0, Player::X, 0.0), 2);
    }

    #[test]
    fn tactical_rollout_takes_an_immediate_local_win() {
        let mut x = [0; 9];
        let mut o = [0; 9];
        x[4] = 0b000_000_011;
        o[4] = 0b000_010_000;
        o[0] = 0b000_000_001;
        let position = Position::from_cells(x, o, Some(4), Player::X).unwrap();
        let mut searcher = MctsSearcher::new();

        let mv = searcher.tactical_move(position, &position.legal_moves());

        assert_eq!(mv, Move::new(4, 2));
        assert_eq!(
            position.play(mv).unwrap().mini_result(4),
            MiniResult::Win(Player::X)
        );
    }

    #[test]
    fn tactical_rollout_avoids_routing_into_an_immediate_macro_loss() {
        let position = tactical_position(3);
        let mut searcher = MctsSearcher::new();

        let mv = searcher.tactical_move(position, &position.legal_moves());
        let child = position.play(mv).unwrap();

        assert!(
            child
                .legal_moves()
                .iter()
                .all(|reply| { child.play(reply).unwrap().result() != GameResult::Win(Player::O) })
        );
    }
}
