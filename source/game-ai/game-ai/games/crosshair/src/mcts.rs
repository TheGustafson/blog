use crate::network::PolicyValueNetwork;
use crate::position::{Color, GameResult, Move, MoveList, Position};
use crate::search::{
    SearchOptions, Searcher, evaluate_normalized, normalized_tactical_priors, tactical_move_score,
};
use std::cmp::Ordering;
use std::fmt;
use std::str::FromStr;
#[cfg(not(target_arch = "wasm32"))]
use std::time::{Duration, Instant};

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

const DEFAULT_EXPLORATION: f64 = std::f64::consts::SQRT_2;
const TACTICAL_ROLLOUT_FREQUENCY: usize = 4;

/// Search strategy supported by the Crosshair engine.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Strategy {
    /// AlphaZero-style PUCT using [`PolicyValueNetwork`] priors and leaf values.
    #[default]
    AzPuct,
    /// PUCT using handcrafted tactical priors and handcrafted leaf evaluation.
    HandcraftedPuct,
    /// UCT with tactically biased rollouts.
    TacticalUct,
    /// Vanilla UCT with uniform random rollouts.
    RandomUct,
    /// Iterative-deepening negamax with alpha-beta pruning and Zobrist transposition table.
    AlphaBeta,
}

impl Strategy {
    /// Returns the canonical protocol/JSON identifier for this strategy.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AzPuct => "az-puct",
            Self::HandcraftedPuct => "handcrafted-puct",
            Self::TacticalUct => "tactical-uct",
            Self::RandomUct => "random-uct",
            Self::AlphaBeta => "alpha-beta",
        }
    }

    const fn is_puct(self) -> bool {
        matches!(self, Self::AzPuct | Self::HandcraftedPuct)
    }
}

impl fmt::Display for Strategy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Strategy {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "az-puct" => Ok(Self::AzPuct),
            "handcrafted-puct" => Ok(Self::HandcraftedPuct),
            "tactical-uct" => Ok(Self::TacticalUct),
            "random-uct" => Ok(Self::RandomUct),
            "alpha-beta" => Ok(Self::AlphaBeta),
            _ => Err(
                "strategy must be az-puct, handcrafted-puct, tactical-uct, random-uct, or alpha-beta",
            ),
        }
    }
}

/// Configuration for a Monte Carlo tree search or unified engine search.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MctsOptions {
    /// Maximum MCTS simulations (`1..=1_000_000`).
    pub max_simulations: u32,
    /// Soft time limit in milliseconds (`0` disables the time limit).
    pub soft_time_ms: u32,
    /// Exploration constant ($c_{\text{puct}}$ or $c_{\text{uct}}$).
    pub exploration: f64,
    /// Deterministic PRNG seed.
    pub seed: u64,
    /// Selected search strategy.
    pub strategy: Strategy,
    /// Dirichlet concentration parameter $\alpha$ for root exploration noise (`0.0` disables).
    pub dirichlet_alpha: f64,
    /// Mixing weight $\varepsilon \in [0, 1]$ for root Dirichlet noise (`0.0` disables).
    pub dirichlet_eps: f64,
    /// Maximum depth when `strategy == Strategy::AlphaBeta`.
    pub alpha_beta_depth: u8,
    /// Node limit when `strategy == Strategy::AlphaBeta`.
    pub alpha_beta_nodes: u64,
}

impl Default for MctsOptions {
    fn default() -> Self {
        Self {
            max_simulations: 2_000,
            soft_time_ms: 0,
            exploration: DEFAULT_EXPLORATION,
            seed: 1,
            strategy: Strategy::AzPuct,
            dirichlet_alpha: 0.0,
            dirichlet_eps: 0.0,
            alpha_beta_depth: 6,
            alpha_beta_nodes: 200_000,
        }
    }
}

/// Named strength preset for MCTS search.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MctsPreset {
    /// Preset name (`"beginner"` through `"maximum"`).
    pub name: &'static str,
    /// Search options for this preset.
    pub options: MctsOptions,
}

/// Standard six strength presets for Crosshair MCTS.
pub const MCTS_PRESETS: [MctsPreset; 6] = [
    preset("beginner", 100, 25),
    preset("easy", 400, 50),
    preset("medium", 1_500, 120),
    preset("hard", 5_000, 250),
    preset("expert", 15_000, 600),
    preset("maximum", 40_000, 1_000),
];

const fn preset(name: &'static str, max_simulations: u32, soft_time_ms: u32) -> MctsPreset {
    MctsPreset {
        name,
        options: MctsOptions {
            max_simulations,
            soft_time_ms,
            exploration: DEFAULT_EXPLORATION,
            seed: 1,
            strategy: Strategy::AzPuct,
            dirichlet_alpha: 0.0,
            dirichlet_eps: 0.0,
            alpha_beta_depth: 6,
            alpha_beta_nodes: 200_000,
        },
    }
}

/// Looks up an MCTS preset by name.
pub fn mcts_preset(name: &str) -> Option<MctsPreset> {
    MCTS_PRESETS
        .iter()
        .copied()
        .find(|preset| preset.name == name)
}

/// Per-root-move visit, prior, and score statistics.
#[derive(Clone, Debug, PartialEq)]
pub struct MctsMoveStats {
    /// Candidate move at the root.
    pub mv: Move,
    /// Number of simulations that visited this move.
    pub visits: u32,
    /// Policy prior probability $P(s, a)$ assigned before search.
    pub prior: f64,
    /// Normalized expected score in `[0.0, 1.0]` where `0.5` is even.
    pub expected_score: f64,
}

/// Complete decision report from [`MctsSearcher::search`].
#[derive(Clone, Debug, PartialEq)]
pub struct MctsReport {
    /// Strategy executed.
    pub strategy: Strategy,
    /// Selected move, or `None` if the position is terminal.
    pub best_move: Option<Move>,
    /// Number of MCTS simulations executed (`0` for `alpha-beta`).
    pub simulations: u32,
    /// Total nodes in the search tree.
    pub tree_nodes: u32,
    /// Total visits at the root node.
    pub root_visits: u32,
    /// Total rollout plies executed (`0` for PUCT and `alpha-beta`).
    pub rollout_moves: u64,
    /// Total leaf evaluations executed (`0` for rollout UCT).
    pub leaf_evaluations: u32,
    /// Completed search depth (`0` for MCTS, iterative-deepening depth for `alpha-beta`).
    pub depth: u8,
    /// Normalized expected score in `[0.0, 1.0]` where `0.5` is even.
    pub expected_score: f64,
    /// Raw root value prediction in `[-1.0, 1.0]` when using `"az-puct"`, else `None`.
    pub network_value: Option<f64>,
    /// Elapsed wall-clock time in milliseconds.
    pub elapsed_ms: f64,
    /// Root candidate statistics ordered from strongest to weakest.
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
    leaf_value: f64,
}

/// Reusable searcher supporting `az-puct`, `handcrafted-puct`, `tactical-uct`, `random-uct`, and `alpha-beta`.
pub struct MctsSearcher {
    nodes: Vec<Node>,
    random: SplitMix64,
    network: PolicyValueNetwork,
    alpha_beta: Searcher,
}

impl Default for MctsSearcher {
    fn default() -> Self {
        Self::new()
    }
}

impl MctsSearcher {
    /// Creates a new searcher initialized with the embedded default [`PolicyValueNetwork`].
    pub fn new() -> Self {
        Self::with_network(
            PolicyValueNetwork::embedded().expect("embedded policy-value network is valid"),
        )
    }

    /// Creates a new searcher with a custom [`PolicyValueNetwork`].
    pub fn with_network(network: PolicyValueNetwork) -> Self {
        Self {
            nodes: Vec::with_capacity(4_096),
            random: SplitMix64(1),
            network,
            alpha_beta: Searcher::new(),
        }
    }

    /// Returns a shared reference to the active [`PolicyValueNetwork`].
    pub fn network(&self) -> &PolicyValueNetwork {
        &self.network
    }

    /// Replaces the active [`PolicyValueNetwork`].
    pub fn set_network(&mut self, network: PolicyValueNetwork) {
        self.network = network;
    }

    /// Searches `position` using `options` and returns the decision report.
    pub fn search(&mut self, position: Position, options: MctsOptions) -> MctsReport {
        if options.strategy == Strategy::AlphaBeta {
            let ab_report = self.alpha_beta.search(
                position,
                SearchOptions {
                    max_depth: options.alpha_beta_depth,
                    node_limit: options.alpha_beta_nodes,
                    soft_time_ms: options.soft_time_ms,
                },
            );
            let root_moves = ab_report
                .root_moves
                .into_iter()
                .map(|b| MctsMoveStats {
                    mv: b.mv,
                    visits: b.visits,
                    prior: b.prior,
                    expected_score: b.expected_score,
                })
                .collect();
            return MctsReport {
                strategy: Strategy::AlphaBeta,
                best_move: ab_report.best_move,
                simulations: 0,
                tree_nodes: ab_report.nodes.min(u64::from(u32::MAX)) as u32,
                root_visits: ab_report.nodes.min(u64::from(u32::MAX)) as u32,
                rollout_moves: 0,
                leaf_evaluations: ab_report.leaf_evaluations,
                depth: ab_report.depth,
                expected_score: ab_report.expected_score,
                network_value: None,
                elapsed_ms: ab_report.elapsed_ms,
                root_moves,
            };
        }

        self.nodes.clear();
        self.random = SplitMix64(options.seed);
        let clock = Clock::new(options.soft_time_ms);
        let root_player = position.side_to_move();

        let (mut root, root_net_val) =
            self.build_node(position, None, None, 1.0, root_player, options.strategy);
        if options.dirichlet_alpha > 0.0 && options.dirichlet_eps > 0.0 && root.unexpanded.len() > 1
        {
            apply_dirichlet_noise(
                &mut root.unexpanded,
                options.dirichlet_alpha,
                options.dirichlet_eps.clamp(0.0, 1.0),
                &mut self.random,
            );
        }
        self.nodes.push(root);

        let mut simulations = 0_u32;
        let mut rollout_moves = 0_u64;
        let mut leaf_evaluations = 0_u32;

        if position.result() == GameResult::Ongoing {
            for sim in 0..options.max_simulations.max(1) {
                if sim > 0 && sim % 16 == 0 && clock.expired() {
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
        // Also include any unexpanded root moves with 0 visits so root_moves can represent all candidates
        for pending in &self.nodes[0].unexpanded {
            root_moves.push(MctsMoveStats {
                mv: pending.mv,
                visits: 0,
                prior: pending.prior,
                expected_score: 0.5,
            });
        }
        root_moves.sort_by(|left, right| {
            right
                .visits
                .cmp(&left.visits)
                .then_with(|| right.expected_score.total_cmp(&left.expected_score))
                .then_with(|| right.prior.total_cmp(&left.prior))
                .then_with(|| left.mv.index().cmp(&right.mv.index()))
        });

        let best_move = root_moves.first().map(|stats| stats.mv);
        let expected_score = root_moves.first().map_or(0.5, |stats| stats.expected_score);

        MctsReport {
            strategy: options.strategy,
            best_move,
            simulations,
            tree_nodes: self.nodes.len() as u32,
            root_visits: self.nodes[0].visits,
            rollout_moves,
            leaf_evaluations,
            depth: 0,
            expected_score,
            network_value: root_net_val,
            elapsed_ms: clock.elapsed_ms(),
            root_moves,
        }
    }

    fn build_node(
        &self,
        position: Position,
        parent: Option<usize>,
        incoming_move: Option<Move>,
        prior: f64,
        root_player: Color,
        strategy: Strategy,
    ) -> (Node, Option<f64>) {
        let legal: Vec<Move> = position.legal_moves().iter().collect();
        if let GameResult::Win { winner, .. } = position.result() {
            let terminal_val = if winner == root_player { 1.0 } else { -1.0 };
            let net_val = (strategy == Strategy::AzPuct && parent.is_none()).then(|| {
                if winner == position.side_to_move() {
                    1.0
                } else {
                    -1.0
                }
            });
            return (
                Node {
                    position,
                    parent,
                    incoming_move,
                    children: Vec::new(),
                    unexpanded: Vec::new(),
                    prior,
                    visits: 0,
                    value_sum: 0.0,
                    leaf_value: terminal_val,
                },
                net_val,
            );
        }

        match strategy {
            Strategy::AzPuct => {
                let (priors, net_value_side) = self.network.evaluate(position);
                let leaf_value = if position.side_to_move() == root_player {
                    net_value_side
                } else {
                    -net_value_side
                };
                let unexpanded = legal
                    .into_iter()
                    .map(|mv| PendingMove {
                        mv,
                        prior: priors[mv.index() as usize],
                    })
                    .collect();
                (
                    Node {
                        position,
                        parent,
                        incoming_move,
                        children: Vec::new(),
                        unexpanded,
                        prior,
                        visits: 0,
                        value_sum: 0.0,
                        leaf_value,
                    },
                    Some(net_value_side),
                )
            }
            Strategy::HandcraftedPuct => {
                let priors = normalized_tactical_priors(position);
                let side_val = evaluate_normalized(position);
                let leaf_value = if position.side_to_move() == root_player {
                    side_val
                } else {
                    -side_val
                };
                let unexpanded = legal
                    .into_iter()
                    .map(|mv| PendingMove {
                        mv,
                        prior: priors[mv.index() as usize],
                    })
                    .collect();
                (
                    Node {
                        position,
                        parent,
                        incoming_move,
                        children: Vec::new(),
                        unexpanded,
                        prior,
                        visits: 0,
                        value_sum: 0.0,
                        leaf_value,
                    },
                    None,
                )
            }
            Strategy::TacticalUct | Strategy::RandomUct | Strategy::AlphaBeta => {
                let uniform = if legal.is_empty() {
                    0.0
                } else {
                    1.0 / (legal.len() as f64)
                };
                let unexpanded = legal
                    .into_iter()
                    .map(|mv| PendingMove { mv, prior: uniform })
                    .collect();
                (
                    Node {
                        position,
                        parent,
                        incoming_move,
                        children: Vec::new(),
                        unexpanded,
                        prior,
                        visits: 0,
                        value_sum: 0.0,
                        leaf_value: 0.0,
                    },
                    None,
                )
            }
        }
    }

    fn run_simulation(
        &mut self,
        root_player: Color,
        exploration: f64,
        strategy: Strategy,
    ) -> SimulationWork {
        let mut node = 0;
        loop {
            if self.nodes[node].position.result() != GameResult::Ongoing {
                break;
            }
            if !strategy.is_puct() {
                if !self.nodes[node].unexpanded.is_empty() {
                    node = self.expand_random(node, root_player, strategy);
                    break;
                }
                if self.nodes[node].children.is_empty() {
                    break;
                }
                node = self.select_uct_child(node, root_player, exploration);
            } else {
                match self.select_puct_action(node, root_player, exploration) {
                    PuctAction::Expand(index) => {
                        node = self.expand_at(node, index, root_player, strategy);
                        break;
                    }
                    PuctAction::Descend(child) => node = child,
                    PuctAction::None => break,
                }
            }
        }

        let (value, work) = if strategy.is_puct() {
            (
                self.nodes[node].leaf_value,
                SimulationWork {
                    rollout_moves: 0,
                    leaf_evaluated: true,
                },
            )
        } else {
            let (value, rollout_moves) = self.rollout(
                self.nodes[node].position,
                root_player,
                strategy == Strategy::TacticalUct,
            );
            (
                value,
                SimulationWork {
                    rollout_moves,
                    leaf_evaluated: false,
                },
            )
        };

        let mut current = Some(node);
        while let Some(index) = current {
            self.nodes[index].visits += 1;
            self.nodes[index].value_sum += value;
            current = self.nodes[index].parent;
        }
        work
    }

    fn expand_random(&mut self, parent: usize, root_player: Color, strategy: Strategy) -> usize {
        let move_index = self.random.index(self.nodes[parent].unexpanded.len());
        self.expand_at(parent, move_index, root_player, strategy)
    }

    fn expand_at(
        &mut self,
        parent: usize,
        move_index: usize,
        root_player: Color,
        strategy: Strategy,
    ) -> usize {
        let pending = self.nodes[parent].unexpanded.swap_remove(move_index);
        let position = self.nodes[parent]
            .position
            .play(pending.mv)
            .expect("tree expansion uses legal moves");
        let child_index = self.nodes.len();
        let (child_node, _) = self.build_node(
            position,
            Some(parent),
            Some(pending.mv),
            pending.prior,
            root_player,
            strategy,
        );
        self.nodes.push(child_node);
        self.nodes[parent].children.push(child_index);
        child_index
    }

    fn select_uct_child(&self, parent: usize, root_player: Color, exploration: f64) -> usize {
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
                .index();
            let best_move_index = self.nodes[best]
                .incoming_move
                .expect("non-root nodes have moves")
                .index();
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
        root_player: Color,
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

    fn rollout(
        &mut self,
        mut position: Position,
        root_player: Color,
        tactical: bool,
    ) -> (f64, u32) {
        let mut plies = 0;
        while position.result() == GameResult::Ongoing {
            let moves = position.legal_moves();
            let mv = if tactical && self.random.index(TACTICAL_ROLLOUT_FREQUENCY) == 0 {
                self.tactical_rollout_move(position, moves)
            } else {
                moves
                    .iter()
                    .nth(self.random.index(moves.len()))
                    .expect("ongoing positions have legal moves")
            };
            position = position.play(mv).expect("rollouts use legal moves");
            plies += 1;
        }
        let GameResult::Win { winner, .. } = position.result() else {
            unreachable!()
        };
        let value = if winner == root_player { 1.0 } else { -1.0 };
        (value, plies)
    }

    fn tactical_rollout_move(&mut self, position: Position, moves: MoveList) -> Move {
        let mut best_move = None;
        let mut best_score = i32::MIN;
        let mut tied = 0;
        for mv in moves {
            let score = tactical_move_score(position, mv);
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
    let move_index = mv.index();
    if score > *best_score || (score == *best_score && move_index < *best_move) {
        *best_action = action;
        *best_move = move_index;
        *best_score = score;
    }
}

fn expected_score(value_sum: f64, visits: u32) -> f64 {
    if visits == 0 {
        0.5
    } else {
        (value_sum / f64::from(visits) + 1.0) * 0.5
    }
}

fn apply_dirichlet_noise(
    unexpanded: &mut [PendingMove],
    alpha: f64,
    eps: f64,
    rng: &mut SplitMix64,
) {
    let mut gammas = Vec::with_capacity(unexpanded.len());
    let mut sum = 0.0_f64;
    for _ in 0..unexpanded.len() {
        let g = rng.gamma(alpha).max(1e-12);
        gammas.push(g);
        sum += g;
    }
    if sum <= 0.0 {
        return;
    }
    for (pending, g) in unexpanded.iter_mut().zip(gammas) {
        let noise = g / sum;
        pending.prior = (1.0 - eps) * pending.prior + eps * noise;
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

    fn uniform_open(&mut self) -> f64 {
        let u = (self.next() >> 11) as f64 / ((1_u64 << 53) as f64);
        u.clamp(1e-12, 1.0 - 1e-12)
    }

    fn standard_normal(&mut self) -> f64 {
        let u1 = self.uniform_open();
        let u2 = self.uniform_open();
        (-2.0 * u1.ln()).sqrt() * (std::f64::consts::TAU * u2).cos()
    }

    /// Marsaglia & Tsang's method for $\text{Gamma}(\alpha, 1)$ with $\alpha < 1$ boosting.
    fn gamma(&mut self, alpha: f64) -> f64 {
        if alpha < 1.0 {
            let u = self.uniform_open();
            return self.gamma(alpha + 1.0) * u.powf(1.0 / alpha);
        }
        let d = alpha - 1.0 / 3.0;
        let c = 1.0 / (9.0 * d).sqrt();
        loop {
            let x = self.standard_normal();
            let v = 1.0 + c * x;
            if v <= 0.0 {
                continue;
            }
            let v3 = v * v * v;
            let u = self.uniform_open();
            if u < 1.0 - 0.0331 * (x * x) * (x * x) {
                return d * v3;
            }
            if u.ln() < 0.5 * x * x + d * (1.0 - v3 + v3.ln()) {
                return d * v3;
            }
        }
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

    fn elapsed_ms(&self) -> f64 {
        (performance_now() - self.started).max(0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_five_strategies_produce_legal_decisions_from_start() {
        let mut searcher = MctsSearcher::new();
        let pos = Position::start();
        for strategy in [
            Strategy::AzPuct,
            Strategy::HandcraftedPuct,
            Strategy::TacticalUct,
            Strategy::RandomUct,
            Strategy::AlphaBeta,
        ] {
            let report = searcher.search(
                pos,
                MctsOptions {
                    max_simulations: 64,
                    strategy,
                    alpha_beta_depth: 3,
                    ..MctsOptions::default()
                },
            );
            assert_eq!(report.strategy, strategy);
            assert!(report.best_move.is_some());
            assert!(pos.legal_moves().contains(report.best_move.unwrap()));
            assert_eq!(report.network_value.is_some(), strategy == Strategy::AzPuct);
        }
    }

    #[test]
    fn dirichlet_noise_preserves_normalized_priors() {
        let mut searcher = MctsSearcher::new();
        let report = searcher.search(
            Position::start(),
            MctsOptions {
                max_simulations: 32,
                strategy: Strategy::AzPuct,
                dirichlet_alpha: 0.3,
                dirichlet_eps: 0.25,
                seed: 7,
                ..MctsOptions::default()
            },
        );
        let prior_sum: f64 = report.root_moves.iter().map(|m| m.prior).sum();
        assert!((prior_sum - 1.0).abs() < 1e-6);
    }
}
