use crate::knowledge::{Knowledge, analyze};
use crate::{Cell, Color, GameResult, Move, Position, Seat};
#[cfg(not(target_arch = "wasm32"))]
use std::time::{Duration, Instant};

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

const DEFAULT_EXPLORATION: f64 = 0.2;
const DEFAULT_RAVE_EQUIVALENCE: f64 = 1_000.0;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MctsStrategy {
    PlainUct,
    UctRave,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RolloutPolicy {
    Random,
    SaveBridge,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KnowledgePolicy {
    Disabled,
    InferiorCells { min_visits: u32 },
}

impl KnowledgePolicy {
    pub const fn min_visits(self) -> Option<u32> {
        match self {
            Self::Disabled => None,
            Self::InferiorCells { min_visits } => Some(min_visits),
        }
    }
}

impl RolloutPolicy {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Random => "random",
            Self::SaveBridge => "save-bridge",
        }
    }
}

impl MctsStrategy {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PlainUct => "plain-uct",
            Self::UctRave => "uct-rave",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MctsOptions {
    pub max_simulations: u32,
    pub soft_time_ms: u32,
    pub exploration: f64,
    pub strategy: MctsStrategy,
    pub rave_equivalence: f64,
    pub rollout_policy: RolloutPolicy,
    pub knowledge_policy: KnowledgePolicy,
    pub use_virtual_connections: bool,
    pub seed: u64,
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
    preset("beginner", 200, 50),
    preset("easy", 1_000, 100),
    preset("medium", 4_000, 200),
    preset("hard", 20_000, 500),
    preset("expert", 80_000, 1_300),
    preset("maximum", 200_000, 2_000),
];

const fn preset(name: &'static str, max_simulations: u32, soft_time_ms: u32) -> MctsPreset {
    MctsPreset {
        name,
        options: MctsOptions {
            max_simulations,
            soft_time_ms,
            exploration: DEFAULT_EXPLORATION,
            strategy: MctsStrategy::UctRave,
            rave_equivalence: DEFAULT_RAVE_EQUIVALENCE,
            rollout_policy: RolloutPolicy::SaveBridge,
            knowledge_policy: KnowledgePolicy::InferiorCells { min_visits: 32 },
            use_virtual_connections: true,
            seed: 1,
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
    pub expected_score: f64,
    pub rave_visits: u32,
    pub rave_expected_score: f64,
    pub proven_winner: Option<Seat>,
    pub proof_distance: Option<u16>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MctsReport {
    pub best_move: Option<Move>,
    pub strategy: MctsStrategy,
    pub rave_equivalence: f64,
    pub rollout_policy: RolloutPolicy,
    pub knowledge_policy: KnowledgePolicy,
    pub virtual_connections_enabled: bool,
    pub simulations: u32,
    pub tree_nodes: u32,
    pub root_visits: u32,
    pub expected_score: f64,
    pub elapsed_ms: u32,
    pub rollout_moves: u64,
    pub bridge_replies: u64,
    pub knowledge_nodes: u32,
    pub pruned_moves: u32,
    pub must_play_nodes: u32,
    pub root_pruned_moves: u32,
    pub root_must_play_moves: u32,
    pub virtual_connections: u32,
    pub semi_connections: u32,
    pub connection_search_truncated_nodes: u32,
    pub proven_nodes: u32,
    pub solver_propagations: u32,
    pub proven_winner: Option<Seat>,
    pub proof_distance: Option<u16>,
    pub root_moves: Vec<MctsMoveStats>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Proof {
    winner: Seat,
    distance: Option<u16>,
}

struct Node {
    position: Position,
    parent: Option<usize>,
    incoming_move: Option<Move>,
    children: Vec<usize>,
    permutation_offset: u16,
    permutation_step: u16,
    next_slot: u16,
    swap_unexpanded: bool,
    visits: u32,
    value_sum: f64,
    rave_visits: u32,
    rave_value_sum: f64,
    knowledge: Option<Knowledge>,
    proof: Option<Proof>,
}

impl Node {
    fn new(
        position: Position,
        parent: Option<usize>,
        incoming_move: Option<Move>,
        random: &mut SplitMix64,
    ) -> Self {
        let cells = position.size().cell_count();
        let proof = match position.result() {
            GameResult::Win(winner) => Some(Proof {
                winner,
                distance: Some(0),
            }),
            GameResult::Ongoing => None,
        };
        Self {
            position,
            parent,
            incoming_move,
            children: Vec::new(),
            permutation_offset: random.index(usize::from(cells)) as u16,
            permutation_step: coprime_step(cells, random),
            next_slot: 0,
            swap_unexpanded: position.swap_available(),
            visits: 0,
            value_sum: 0.0,
            rave_visits: 0,
            rave_value_sum: 0.0,
            knowledge: None,
            proof,
        }
    }

    fn next_unexpanded(&mut self) -> Option<Move> {
        if self.swap_unexpanded {
            self.swap_unexpanded = false;
            return Some(Move::Swap);
        }
        let cells = self.position.size().cell_count();
        while self.next_slot < cells {
            let dense = (u32::from(self.permutation_offset)
                + u32::from(self.next_slot) * u32::from(self.permutation_step))
                % u32::from(cells);
            self.next_slot += 1;
            let cell = crate::Cell::from_dense(dense as u16, self.position.size());
            let mv = Move::Place(cell);
            if self.position.is_legal(mv) && self.knowledge.is_none_or(|info| info.allows(mv)) {
                return Some(mv);
            }
        }
        None
    }

    fn allows(&self, mv: Move) -> bool {
        self.knowledge.is_none_or(|knowledge| knowledge.allows(mv))
    }

    fn expansion_exhausted(&self) -> bool {
        !self.swap_unexpanded && self.next_slot >= self.position.size().cell_count()
    }
}

pub struct MctsSearcher {
    nodes: Vec<Node>,
    random: SplitMix64,
}

impl Default for MctsSearcher {
    fn default() -> Self {
        Self::new()
    }
}

impl MctsSearcher {
    pub fn new() -> Self {
        Self {
            nodes: Vec::with_capacity(16_384),
            random: SplitMix64(1),
        }
    }

    pub fn search(&mut self, position: Position, options: MctsOptions) -> MctsReport {
        self.nodes.clear();
        self.random = SplitMix64(options.seed);
        self.nodes
            .push(Node::new(position, None, None, &mut self.random));
        let root_seat = position.seat_to_move();
        let clock = Clock::new(options.soft_time_ms);
        let mut simulations = 0;
        let mut rollout_moves = 0;
        let mut bridge_replies = 0;
        let mut counters = SearchCounters::default();

        if position.result() == GameResult::Ongoing {
            for simulation in 0..options.max_simulations.max(1) {
                if simulation > 0 && simulation % 16 == 0 && clock.expired() {
                    break;
                }
                let rollout = self.run_simulation(root_seat, options, &mut counters);
                rollout_moves += u64::from(rollout.moves);
                bridge_replies += u64::from(rollout.bridge_replies);
                simulations += 1;
                if self.nodes[0]
                    .proof
                    .is_some_and(|proof| proof.winner == root_seat)
                    && !self.nodes[0].children.is_empty()
                {
                    break;
                }
            }
        }

        let mut root_moves = self.nodes[0]
            .children
            .iter()
            .filter(|&&index| {
                self.nodes[0].allows(
                    self.nodes[index]
                        .incoming_move
                        .expect("root children have moves"),
                )
            })
            .map(|&index| {
                let child = &self.nodes[index];
                MctsMoveStats {
                    mv: child.incoming_move.expect("root children have moves"),
                    visits: child.visits,
                    expected_score: child.proof.map_or_else(
                        || mean_score(child),
                        |proof| f64::from(proof.winner == root_seat),
                    ),
                    rave_visits: child.rave_visits,
                    rave_expected_score: rave_mean_score(child),
                    proven_winner: child.proof.map(|proof| proof.winner),
                    proof_distance: child.proof.and_then(|proof| proof.distance),
                }
            })
            .collect::<Vec<_>>();
        root_moves.sort_by(|left, right| compare_root_moves(left, right, root_seat));
        let best_move = root_moves.first().map(|stats| stats.mv);
        let expected_score = self.nodes[0].proof.map_or_else(
            || root_moves.first().map_or(0.5, |stats| stats.expected_score),
            |proof| f64::from(proof.winner == root_seat),
        );

        MctsReport {
            best_move,
            strategy: options.strategy,
            rave_equivalence: options.rave_equivalence,
            rollout_policy: options.rollout_policy,
            knowledge_policy: options.knowledge_policy,
            virtual_connections_enabled: options.use_virtual_connections,
            simulations,
            tree_nodes: self.nodes.len() as u32,
            root_visits: self.nodes[0].visits,
            expected_score,
            elapsed_ms: clock.elapsed_ms(),
            rollout_moves,
            bridge_replies,
            knowledge_nodes: counters.knowledge_nodes,
            pruned_moves: counters.pruned_moves,
            must_play_nodes: counters.must_play_nodes,
            root_pruned_moves: self.nodes[0]
                .knowledge
                .map_or(0, |knowledge| knowledge.pruned.count()),
            root_must_play_moves: self.nodes[0]
                .knowledge
                .and_then(|knowledge| knowledge.must_play)
                .map_or(0, |moves| moves.count()),
            virtual_connections: counters.virtual_connections,
            semi_connections: counters.semi_connections,
            connection_search_truncated_nodes: counters.connection_search_truncated_nodes,
            proven_nodes: counters.proven_nodes,
            solver_propagations: counters.solver_propagations,
            proven_winner: self.nodes[0].proof.map(|proof| proof.winner),
            proof_distance: self.nodes[0].proof.and_then(|proof| proof.distance),
            root_moves,
        }
    }

    fn run_simulation(
        &mut self,
        root_seat: Seat,
        options: MctsOptions,
        counters: &mut SearchCounters,
    ) -> RolloutSummary {
        let mut node = 0;
        loop {
            if self.nodes[node].position.result() != GameResult::Ongoing
                || (node != 0 && self.nodes[node].proof.is_some())
            {
                break;
            }
            self.prepare_knowledge(
                node,
                options.knowledge_policy,
                options.use_virtual_connections,
                counters,
            );
            if node != 0 && self.nodes[node].proof.is_some() {
                break;
            }
            let next_move = self.nodes[node].next_unexpanded();
            if let Some(mv) = next_move {
                let position = self.nodes[node]
                    .position
                    .play(mv)
                    .expect("tree expansion uses legal moves");
                let child = self.nodes.len();
                self.nodes
                    .push(Node::new(position, Some(node), Some(mv), &mut self.random));
                self.nodes[node].children.push(child);
                node = child;
                break;
            }
            if self.nodes[node].children.is_empty() {
                break;
            }
            node = self.select_child(node, root_seat, options);
        }

        let last_move = self.nodes[node].incoming_move.and_then(Move::cell);
        let rollout = self.nodes[node].proof.map_or_else(
            || {
                self.rollout(
                    self.nodes[node].position,
                    root_seat,
                    last_move,
                    options.rollout_policy,
                )
            },
            |proof| Rollout {
                score: f64::from(proof.winner == root_seat),
                moves: 0,
                bridge_replies: 0,
                trace: MoveTrace::default(),
            },
        );
        let mut trace = rollout.trace;
        let mut current = Some(node);
        while let Some(index) = current {
            let parent = self.nodes[index].parent;
            self.nodes[index].visits += 1;
            self.nodes[index].value_sum += rollout.score;
            if options.strategy == MctsStrategy::UctRave {
                self.update_rave(index, trace, rollout.score);
            }
            if let (Some(parent), Some(mv)) = (parent, self.nodes[index].incoming_move) {
                trace.prepend(self.nodes[parent].position.color_to_move(), mv);
            }
            self.refresh_proof(index, counters);
            current = parent;
        }
        RolloutSummary {
            moves: rollout.moves,
            bridge_replies: rollout.bridge_replies,
        }
    }

    fn prepare_knowledge(
        &mut self,
        node: usize,
        policy: KnowledgePolicy,
        use_virtual_connections: bool,
        counters: &mut SearchCounters,
    ) {
        let Some(min_visits) = policy.min_visits() else {
            return;
        };
        if self.nodes[node].knowledge.is_some()
            || (node != 0 && self.nodes[node].visits < min_visits)
        {
            return;
        }

        let knowledge = analyze(self.nodes[node].position, use_virtual_connections);
        counters.knowledge_nodes += 1;
        counters.pruned_moves += knowledge.pruned.count();
        counters.must_play_nodes += u32::from(knowledge.must_play.is_some());
        counters.virtual_connections += knowledge.virtual_connections;
        counters.semi_connections += knowledge.semi_connections;
        counters.connection_search_truncated_nodes +=
            u32::from(knowledge.connection_search_truncated);
        if let Some(winner) = knowledge.proven_winner {
            self.nodes[node].proof = Some(Proof {
                winner,
                distance: None,
            });
            counters.proven_nodes += 1;
        }
        self.nodes[node].knowledge = Some(knowledge);
    }

    fn select_child(&self, parent: usize, root_seat: Seat, options: MctsOptions) -> usize {
        let node = &self.nodes[parent];
        let mover = node.position.seat_to_move();
        let root_turn = node.position.seat_to_move() == root_seat;
        let log_parent = f64::from(node.visits.max(1)).ln();
        let allowed = node
            .children
            .iter()
            .copied()
            .filter(|&child| {
                node.allows(
                    self.nodes[child]
                        .incoming_move
                        .expect("children have moves"),
                )
            })
            .collect::<Vec<_>>();
        if let Some(winning) = allowed
            .iter()
            .copied()
            .filter(|&child| {
                self.nodes[child]
                    .proof
                    .is_some_and(|proof| proof.winner == mover)
            })
            .min_by_key(|&child| {
                (
                    self.nodes[child]
                        .proof
                        .expect("filtered proof")
                        .distance
                        .unwrap_or(u16::MAX),
                    self.nodes[child]
                        .incoming_move
                        .expect("children have moves")
                        .order_key(),
                )
            })
        {
            return winning;
        }
        let unresolved = allowed
            .iter()
            .copied()
            .filter(|&child| self.nodes[child].proof.is_none())
            .collect::<Vec<_>>();
        if !unresolved.is_empty() {
            return unresolved
                .into_iter()
                .max_by(|&left, &right| {
                    let score = |index| {
                        let child = &self.nodes[index];
                        selection_score(child, root_turn, log_parent, options)
                    };
                    score(left).total_cmp(&score(right)).then_with(|| {
                        self.nodes[right]
                            .incoming_move
                            .expect("children have moves")
                            .order_key()
                            .cmp(
                                &self.nodes[left]
                                    .incoming_move
                                    .expect("children have moves")
                                    .order_key(),
                            )
                    })
                })
                .expect("unresolved selection requires a child");
        }
        allowed
            .into_iter()
            .max_by_key(|&child| {
                (
                    self.nodes[child]
                        .proof
                        .expect("all children proven")
                        .distance
                        .unwrap_or(u16::MAX),
                    std::cmp::Reverse(
                        self.nodes[child]
                            .incoming_move
                            .expect("children have moves")
                            .order_key(),
                    ),
                )
            })
            .expect("selection requires a child")
    }

    fn refresh_proof(&mut self, node: usize, counters: &mut SearchCounters) {
        if self.nodes[node].proof.is_some()
            || self.nodes[node].position.result() != GameResult::Ongoing
        {
            return;
        }
        let mover = self.nodes[node].position.seat_to_move();
        let allowed = self.nodes[node]
            .children
            .iter()
            .copied()
            .filter(|&child| {
                self.nodes[node].allows(
                    self.nodes[child]
                        .incoming_move
                        .expect("children have moves"),
                )
            })
            .collect::<Vec<_>>();
        let proof = if let Some(proof) = allowed
            .iter()
            .filter_map(|&child| self.nodes[child].proof)
            .filter(|proof| proof.winner == mover)
            .min_by_key(|proof| proof.distance.unwrap_or(u16::MAX))
        {
            Some(Proof {
                winner: mover,
                distance: proof.distance.map(|distance| distance.saturating_add(1)),
            })
        } else if self.nodes[node].expansion_exhausted()
            && !allowed.is_empty()
            && allowed.iter().all(|&child| {
                self.nodes[child]
                    .proof
                    .is_some_and(|proof| proof.winner == mover.other())
            })
        {
            Some(Proof {
                winner: mover.other(),
                distance: allowed
                    .iter()
                    .try_fold(0, |longest, &child| {
                        self.nodes[child]
                            .proof
                            .and_then(|proof| proof.distance)
                            .map(|distance| longest.max(distance))
                    })
                    .map(|distance| distance.saturating_add(1)),
            })
        } else {
            None
        };
        if let Some(proof) = proof {
            self.nodes[node].proof = Some(proof);
            counters.proven_nodes += 1;
            counters.solver_propagations += 1;
        }
    }

    fn update_rave(&mut self, node: usize, trace: MoveTrace, score: f64) {
        let color = self.nodes[node].position.color_to_move();
        let child_count = self.nodes[node].children.len();
        for slot in 0..child_count {
            let child = self.nodes[node].children[slot];
            let Some(cell) = self.nodes[child]
                .incoming_move
                .expect("children have moves")
                .cell()
            else {
                continue;
            };
            if trace.contains(color, cell) {
                self.nodes[child].rave_visits += 1;
                self.nodes[child].rave_value_sum += score;
            }
        }
    }

    fn rollout(
        &mut self,
        mut position: Position,
        root_seat: Seat,
        mut last_move: Option<Cell>,
        policy: RolloutPolicy,
    ) -> Rollout {
        if let GameResult::Win(winner) = position.result() {
            return Rollout {
                score: f64::from(winner == root_seat),
                moves: 0,
                bridge_replies: 0,
                trace: MoveTrace::default(),
            };
        }

        let mut moves = 0;
        let mut trace = MoveTrace::default();
        let mut crossed_swap = false;
        let mut bridge_replies = 0;
        if position.swap_available() {
            if self.random.next() & 1 == 0 {
                position = position.play(Move::Swap).expect("swap is available");
                moves += 1;
                crossed_swap = true;
                last_move = None;
            } else {
                position.rollout_decline_swap();
            }
        }

        let mut empty = position.empty_cells();
        for index in (1..empty.len()).rev() {
            let other = self.random.index(index + 1);
            empty.swap(index, other);
        }
        let empty_count = empty.len();
        let mut cursor = 0;
        while moves - u32::from(crossed_swap) < empty_count as u32 {
            let response = (policy != RolloutPolicy::Random)
                .then(|| last_move.and_then(|cell| self.random_bridge_response(position, cell)))
                .flatten();
            let cell = if let Some(response) = response {
                bridge_replies += 1;
                response
            } else {
                loop {
                    let cell = empty[cursor];
                    cursor += 1;
                    if position.color_at(cell).is_none() {
                        break cell;
                    }
                }
            };
            if !crossed_swap {
                trace.insert(position.color_to_move(), cell);
            }
            position.rollout_place(cell);
            moves += 1;
            last_move = Some(cell);
        }
        let winner = position.winner_on_full_board();
        Rollout {
            score: f64::from(winner == root_seat),
            moves,
            bridge_replies,
            trace,
        }
    }

    fn random_bridge_response(&mut self, position: Position, attacked: Cell) -> Option<Cell> {
        let (responses, count) =
            crate::patterns::bridge_responses(position, position.color_to_move(), attacked);
        (count > 0).then(|| responses[self.random.index(count)])
    }
}

fn mean_score(node: &Node) -> f64 {
    node.value_sum / f64::from(node.visits.max(1))
}

fn rave_mean_score(node: &Node) -> f64 {
    if node.rave_visits == 0 {
        0.5
    } else {
        node.rave_value_sum / f64::from(node.rave_visits)
    }
}

fn compare_root_moves(
    left: &MctsMoveStats,
    right: &MctsMoveStats,
    root_seat: Seat,
) -> std::cmp::Ordering {
    let rank = |stats: &MctsMoveStats| match stats.proven_winner {
        Some(winner) if winner == root_seat => 2,
        None => 1,
        Some(_) => 0,
    };
    rank(right)
        .cmp(&rank(left))
        .then_with(|| match (left.proven_winner, right.proven_winner) {
            (Some(left_winner), Some(right_winner))
                if left_winner == root_seat && right_winner == root_seat =>
            {
                proof_distance_rank(left.proof_distance)
                    .cmp(&proof_distance_rank(right.proof_distance))
            }
            (Some(left_winner), Some(right_winner))
                if left_winner != root_seat && right_winner != root_seat =>
            {
                proof_distance_rank(right.proof_distance)
                    .cmp(&proof_distance_rank(left.proof_distance))
            }
            _ => right
                .visits
                .cmp(&left.visits)
                .then_with(|| right.expected_score.total_cmp(&left.expected_score)),
        })
        .then_with(|| left.mv.order_key().cmp(&right.mv.order_key()))
}

fn proof_distance_rank(distance: Option<u16>) -> u16 {
    distance.unwrap_or(u16::MAX)
}

fn selection_score(node: &Node, root_turn: bool, log_parent: f64, options: MctsOptions) -> f64 {
    let orient = |score| if root_turn { score } else { 1.0 - score };
    let direct = orient(mean_score(node));
    let exploration =
        options.exploration.max(0.0) * (log_parent / f64::from(node.visits.max(1))).sqrt();
    if options.strategy == MctsStrategy::PlainUct || node.rave_visits == 0 {
        return direct + exploration;
    }
    let visits = f64::from(node.visits);
    let rave_visits = f64::from(node.rave_visits);
    let equivalence = options.rave_equivalence.max(1.0);
    let beta = rave_visits / (visits + rave_visits + 4.0 * visits * rave_visits / equivalence);
    (direct + exploration) * (1.0 - beta) + orient(rave_mean_score(node)) * beta
}

#[derive(Clone, Copy, Default)]
struct MoveTrace {
    red: [u64; 9],
    blue: [u64; 9],
}

impl MoveTrace {
    fn insert(&mut self, color: Color, cell: Cell) {
        let bits = match color {
            Color::Red => &mut self.red,
            Color::Blue => &mut self.blue,
        };
        let index = usize::from(cell.index());
        bits[index / 64] |= 1_u64 << (index % 64);
    }

    fn contains(self, color: Color, cell: Cell) -> bool {
        let bits = match color {
            Color::Red => &self.red,
            Color::Blue => &self.blue,
        };
        let index = usize::from(cell.index());
        bits[index / 64] & (1_u64 << (index % 64)) != 0
    }

    fn clear(&mut self) {
        *self = Self::default();
    }

    fn prepend(&mut self, color: Color, mv: Move) {
        match mv {
            Move::Place(cell) => self.insert(color, cell),
            Move::Swap => self.clear(),
        }
    }
}

struct Rollout {
    score: f64,
    moves: u32,
    bridge_replies: u32,
    trace: MoveTrace,
}

struct RolloutSummary {
    moves: u32,
    bridge_replies: u32,
}

#[derive(Default)]
struct SearchCounters {
    knowledge_nodes: u32,
    pruned_moves: u32,
    must_play_nodes: u32,
    virtual_connections: u32,
    semi_connections: u32,
    connection_search_truncated_nodes: u32,
    proven_nodes: u32,
    solver_propagations: u32,
}

fn coprime_step(cells: u16, random: &mut SplitMix64) -> u16 {
    loop {
        let candidate = 1 + random.index(usize::from(cells - 1)) as u16;
        if gcd(candidate, cells) == 1 {
            return candidate;
        }
    }
}

const fn gcd(mut left: u16, mut right: u16) -> u16 {
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left
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

    fn index(&mut self, length: usize) -> usize {
        debug_assert!(length > 0);
        (self.next() as usize) % length
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
    use crate::{BoardSize, SwapRule};

    #[test]
    fn amaf_uses_stone_color_after_the_pie_rule_swap() {
        let position = Position::new(BoardSize::new(9).unwrap(), SwapRule::Enabled)
            .play("e5".parse().unwrap())
            .unwrap()
            .play(Move::Swap)
            .unwrap();
        assert_eq!(position.color_to_move(), Color::Blue);

        let mv: Move = "f5".parse().unwrap();
        let mut random = SplitMix64(1);
        let root = Node::new(position, None, None, &mut random);
        let child = Node::new(position.play(mv).unwrap(), Some(0), Some(mv), &mut random);
        let mut searcher = MctsSearcher {
            nodes: vec![root, child],
            random,
        };
        searcher.nodes[0].children.push(1);

        let cell = mv.cell().unwrap();
        let mut wrong_color = MoveTrace::default();
        wrong_color.insert(Color::Red, cell);
        searcher.update_rave(0, wrong_color, 1.0);
        assert_eq!(searcher.nodes[1].rave_visits, 0);

        let mut correct_color = MoveTrace::default();
        correct_color.insert(Color::Blue, cell);
        searcher.update_rave(0, correct_color, 1.0);
        assert_eq!(searcher.nodes[1].rave_visits, 1);
        assert_eq!(searcher.nodes[1].rave_value_sum, 1.0);
    }

    #[test]
    fn rollout_swap_does_not_leak_amaf_evidence_across_the_color_change() {
        let position = Position::new(BoardSize::new(9).unwrap(), SwapRule::Enabled)
            .play("e5".parse().unwrap())
            .unwrap();
        let mut saw_swap = false;
        let mut saw_decline = false;
        for seed in 0..16 {
            let mut searcher = MctsSearcher {
                nodes: Vec::new(),
                random: SplitMix64(seed),
            };
            let rollout = searcher.rollout(position, Seat::Two, None, RolloutPolicy::Random);
            if rollout.moves == 81 {
                saw_swap = true;
                assert_eq!(rollout.trace.red, [0; 9]);
                assert_eq!(rollout.trace.blue, [0; 9]);
            } else {
                saw_decline = true;
                assert!(rollout.trace.red != [0; 9]);
                assert!(rollout.trace.blue != [0; 9]);
            }
        }
        assert!(saw_swap && saw_decline);
    }

    #[test]
    fn save_bridge_finds_the_other_carrier() {
        let moves = ["d4", "a1", "e5", "e4"].map(|mv| mv.parse().expect("valid move"));
        let position =
            Position::from_moves(BoardSize::new(9).unwrap(), SwapRule::Disabled, &moves).unwrap();
        let attacked = "e4".parse::<Move>().unwrap().cell().unwrap();
        let expected = "d5".parse::<Move>().unwrap().cell().unwrap();

        let (responses, count) =
            crate::patterns::bridge_responses(position, position.color_to_move(), attacked);

        assert_eq!(&responses[..count], &[expected]);
    }

    #[test]
    fn adjacent_stones_do_not_create_a_bridge_response() {
        let moves = ["d4", "a1", "e4", "d5"].map(|mv| mv.parse().expect("valid move"));
        let position =
            Position::from_moves(BoardSize::new(9).unwrap(), SwapRule::Disabled, &moves).unwrap();
        let attacked = "d5".parse::<Move>().unwrap().cell().unwrap();

        let (_, count) =
            crate::patterns::bridge_responses(position, position.color_to_move(), attacked);

        assert_eq!(count, 0);
    }

    #[test]
    fn tree_swap_is_an_amaf_boundary() {
        let cell = Cell::new(2, 3).unwrap();
        let mut trace = MoveTrace::default();
        trace.insert(Color::Red, cell);
        trace.prepend(Color::Blue, Move::Swap);
        assert!(!trace.contains(Color::Red, cell));

        trace.prepend(Color::Blue, Move::Place(cell));
        assert!(trace.contains(Color::Blue, cell));
    }

    #[test]
    fn solver_propagates_a_winning_child() {
        let position = Position::new(BoardSize::for_test(3), SwapRule::Disabled);
        let mut random = SplitMix64(1);
        let mut root = Node::new(position, None, None, &mut random);
        let mv = Move::Place(Cell::new(1, 1).unwrap());
        let mut child = Node::new(position.play(mv).unwrap(), Some(0), Some(mv), &mut random);
        child.proof = Some(Proof {
            winner: Seat::One,
            distance: Some(2),
        });
        root.children.push(1);
        let mut searcher = MctsSearcher {
            nodes: vec![root, child],
            random,
        };

        searcher.refresh_proof(0, &mut SearchCounters::default());

        assert_eq!(
            searcher.nodes[0].proof,
            Some(Proof {
                winner: Seat::One,
                distance: Some(3),
            })
        );
    }

    #[test]
    fn solver_proves_a_loss_only_after_every_move_is_proven() {
        let position = Position::new(BoardSize::for_test(3), SwapRule::Disabled);
        let mut random = SplitMix64(1);
        let mut root = Node::new(position, None, None, &mut random);
        root.next_slot = position.size().cell_count();
        let mut nodes = vec![root];
        for (distance, mv) in position.legal_moves().into_iter().enumerate() {
            let mut child = Node::new(position.play(mv).unwrap(), Some(0), Some(mv), &mut random);
            child.proof = Some(Proof {
                winner: Seat::Two,
                distance: Some(distance as u16),
            });
            nodes.push(child);
            let child = nodes.len() - 1;
            nodes[0].children.push(child);
        }
        let mut searcher = MctsSearcher { nodes, random };

        searcher.refresh_proof(0, &mut SearchCounters::default());

        assert_eq!(
            searcher.nodes[0].proof,
            Some(Proof {
                winner: Seat::Two,
                distance: Some(9),
            })
        );
    }

    #[test]
    fn solver_does_not_invent_a_distance_for_a_knowledge_proof() {
        let position = Position::new(BoardSize::for_test(3), SwapRule::Disabled);
        let mut random = SplitMix64(1);
        let mut root = Node::new(position, None, None, &mut random);
        let mv = Move::Place(Cell::new(1, 1).unwrap());
        let mut child = Node::new(position.play(mv).unwrap(), Some(0), Some(mv), &mut random);
        child.proof = Some(Proof {
            winner: Seat::One,
            distance: None,
        });
        root.children.push(1);
        let mut searcher = MctsSearcher {
            nodes: vec![root, child],
            random,
        };

        searcher.refresh_proof(0, &mut SearchCounters::default());

        assert_eq!(
            searcher.nodes[0].proof,
            Some(Proof {
                winner: Seat::One,
                distance: None,
            })
        );
    }

    #[test]
    fn virtual_connection_key_ends_a_solved_root_search() {
        let moves = ["b1", "c1", "a3", "c2"].map(|mv| mv.parse().unwrap());
        let position =
            Position::from_moves(BoardSize::for_test(3), SwapRule::Disabled, &moves).unwrap();
        let report = MctsSearcher::new().search(
            position,
            MctsOptions {
                max_simulations: 100,
                soft_time_ms: 0,
                knowledge_policy: KnowledgePolicy::InferiorCells { min_visits: 32 },
                ..MctsOptions::default()
            },
        );

        assert_eq!(report.best_move, Some("b2".parse().unwrap()));
        assert_eq!(report.proven_winner, Some(position.seat_to_move()));
        assert_eq!(report.proof_distance, None);
        assert_eq!(report.simulations, 1);
        assert!(report.virtual_connections > 0);
        assert!(report.semi_connections > 0);
    }

    #[test]
    fn rave_blends_the_complete_uct_score() {
        let position = Position::new(BoardSize::new(9).unwrap(), SwapRule::Disabled);
        let mut random = SplitMix64(1);
        let mut node = Node::new(position, None, None, &mut random);
        node.visits = 10;
        node.value_sum = 6.0;
        node.rave_visits = 10;
        node.rave_value_sum = 8.0;
        let options = MctsOptions {
            max_simulations: 1,
            soft_time_ms: 0,
            exploration: 1.0,
            strategy: MctsStrategy::UctRave,
            rave_equivalence: 40.0,
            rollout_policy: RolloutPolicy::Random,
            knowledge_policy: KnowledgePolicy::Disabled,
            use_virtual_connections: false,
            seed: 1,
        };

        let beta = 10.0 / (10.0 + 10.0 + 4.0 * 10.0 * 10.0 / 40.0);
        let exploration = (100.0_f64.ln() / 10.0).sqrt();
        let expected = (0.6 + exploration) * (1.0 - beta) + 0.8 * beta;
        let actual = selection_score(&node, true, 100.0_f64.ln(), options);

        assert!((actual - expected).abs() < 1e-12);
    }

    #[test]
    fn selection_orients_direct_and_rave_scores_for_the_opponent() {
        let position = Position::new(BoardSize::new(9).unwrap(), SwapRule::Disabled);
        let mut random = SplitMix64(1);
        let mut node = Node::new(position, None, None, &mut random);
        node.visits = 10;
        node.value_sum = 7.0;
        node.rave_visits = 10;
        node.rave_value_sum = 9.0;
        let options = MctsOptions {
            max_simulations: 1,
            soft_time_ms: 0,
            exploration: 0.0,
            strategy: MctsStrategy::UctRave,
            rave_equivalence: 40.0,
            rollout_policy: RolloutPolicy::Random,
            knowledge_policy: KnowledgePolicy::Disabled,
            use_virtual_connections: false,
            seed: 1,
        };

        let beta = 10.0 / (10.0 + 10.0 + 4.0 * 10.0 * 10.0 / 40.0);
        let expected = 0.3 * (1.0 - beta) + 0.1 * beta;
        let actual = selection_score(&node, false, 0.0, options);

        assert!((actual - expected).abs() < 1e-12);
    }
}
