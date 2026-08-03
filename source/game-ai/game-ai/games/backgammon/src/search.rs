use crate::{DICE_OUTCOMES, Dice, Equity, Play, PlayOutcome, Position, evaluate_position};
#[cfg(not(target_arch = "wasm32"))]
use std::time::{Duration, Instant};

#[cfg(all(target_arch = "wasm32", feature = "wasm"))]
use wasm_bindgen::prelude::*;

const DEFAULT_TT_ENTRIES: usize = 1 << 15;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// Limits for one iterative-deepening expectimax search.
pub struct SearchOptions {
    /// Maximum number of complete future checker turns.
    pub max_depth: u8,
    /// Maximum decision nodes; zero is treated as one to preserve a fallback.
    pub node_limit: u64,
    /// Soft wall-clock limit in milliseconds; zero disables it.
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
    preset("beginner", 0, 1, 20),
    preset("easy", 1, 1_000, 50),
    preset("medium", 2, 5_000, 120),
    preset("hard", 2, 20_000, 300),
    preset("expert", 3, 80_000, 650),
    preset("maximum", 4, 250_000, 1_000),
];

const fn preset(
    name: &'static str,
    max_depth: u8,
    node_limit: u64,
    soft_time_ms: u32,
) -> SearchPreset {
    SearchPreset {
        name,
        options: SearchOptions {
            max_depth,
            node_limit,
            soft_time_ms,
        },
    }
}

pub fn search_preset(name: &str) -> Option<SearchPreset> {
    SEARCH_PRESETS
        .iter()
        .copied()
        .find(|preset| preset.name == name)
}

#[derive(Clone, Debug, PartialEq)]
/// The last fully completed iteration and a legal fallback if search stopped early.
pub struct SearchReport {
    /// Selected complete play, or `None` when the supplied position is terminal.
    pub best_play: Option<Play>,
    /// Outcome probabilities from the root side-to-move's perspective.
    pub equity: Equity,
    /// Deepest fully completed iteration.
    pub depth: u8,
    /// Visited decision nodes.
    pub nodes: u64,
    /// Enumerated dice outcomes.
    pub chance_nodes: u64,
    /// Exact-depth transposition-table hits.
    pub tt_hits: u64,
    /// Whether a node, time, or caller cancellation limit interrupted search.
    pub stopped: bool,
}

#[derive(Clone, Copy, Debug)]
struct Entry {
    key: u64,
    depth: u8,
    equity: Equity,
    occupied: bool,
}

impl Default for Entry {
    fn default() -> Self {
        Self {
            key: 0,
            depth: 0,
            equity: Equity::zero(),
            occupied: false,
        }
    }
}

pub struct Searcher {
    table: Vec<Entry>,
    nodes: u64,
    node_limit: u64,
    chance_nodes: u64,
    tt_hits: u64,
    stopped: bool,
    deadline: Deadline,
}

impl Default for Searcher {
    fn default() -> Self {
        Self::new()
    }
}

impl Searcher {
    pub fn new() -> Self {
        Self::with_table_entries(DEFAULT_TT_ENTRIES)
    }

    /// Creates a searcher with a fixed-size table; zero disables the table.
    pub fn with_table_entries(entries: usize) -> Self {
        Self {
            table: vec![Entry::default(); entries],
            nodes: 0,
            node_limit: 0,
            chance_nodes: 0,
            tt_hits: 0,
            stopped: false,
            deadline: Deadline::disabled(),
        }
    }

    /// Searches complete plays for known `dice` using iterative deepening.
    pub fn search(
        &mut self,
        position: Position,
        dice: Dice,
        options: SearchOptions,
    ) -> SearchReport {
        self.search_until(position, dice, options, || false)
    }

    /// Searches until a normal limit or a caller-provided cancellation signal fires.
    pub fn search_until<F: Fn() -> bool>(
        &mut self,
        position: Position,
        dice: Dice,
        options: SearchOptions,
        should_stop: F,
    ) -> SearchReport {
        self.nodes = 0;
        self.node_limit = options.node_limit.max(1);
        self.chance_nodes = 0;
        self.tt_hits = 0;
        self.stopped = false;
        self.deadline = Deadline::after(options.soft_time_ms);
        self.table.fill(Entry::default());

        if position.game_outcome().is_some() {
            return self.report(None, evaluate_position(position), 0);
        }

        let outcomes = position.legal_outcomes(dice);
        let Some(fallback) = outcomes.first() else {
            return self.report(None, evaluate_position(position), 0);
        };
        let mut best_play = fallback.representative().clone();
        let mut best_equity = evaluate_position(fallback.position()).reversed();
        let mut completed_depth = 0;

        for depth in 1..=options.max_depth {
            if self.should_stop(&should_stop) {
                break;
            }
            let Some((play, equity)) = self.search_root(position, dice, depth, &should_stop) else {
                break;
            };
            best_play = play;
            best_equity = equity;
            completed_depth = depth;
            if best_equity.expected_points().abs() == 3.0 {
                break;
            }
        }

        self.report(Some(best_play), best_equity, completed_depth)
    }

    fn search_root<F: Fn() -> bool>(
        &mut self,
        position: Position,
        dice: Dice,
        depth: u8,
        should_stop: &F,
    ) -> Option<(Play, Equity)> {
        let outcomes = ordered_outcomes(position, dice);
        let mut best: Option<(Play, Equity)> = None;
        for outcome in outcomes {
            if !self.visit(should_stop) {
                return None;
            }
            let equity = self
                .chance_value(outcome.position(), depth - 1, should_stop)?
                .reversed();
            if better(equity, outcome.representative(), best.as_ref()) {
                best = Some((outcome.representative().clone(), equity));
            }
        }
        best
    }

    fn chance_value<F: Fn() -> bool>(
        &mut self,
        position: Position,
        depth: u8,
        should_stop: &F,
    ) -> Option<Equity> {
        let key = position.hash();
        if let Some(equity) = self.probe(key, depth) {
            return Some(equity);
        }
        if position.game_outcome().is_some() || depth == 0 {
            let equity = evaluate_position(position);
            self.store(key, depth, equity);
            return Some(equity);
        }

        let mut total = Equity::zero();
        for outcome in DICE_OUTCOMES {
            if self.should_stop(should_stop) {
                return None;
            }
            self.chance_nodes += 1;
            let equity = self.decision_value(position, outcome.dice, depth, should_stop)?;
            total.add_weighted(equity, f32::from(outcome.weight) / 36.0);
        }
        self.store(key, depth, total);
        Some(total)
    }

    fn decision_value<F: Fn() -> bool>(
        &mut self,
        position: Position,
        dice: Dice,
        depth: u8,
        should_stop: &F,
    ) -> Option<Equity> {
        let mut best: Option<(Play, Equity)> = None;
        for outcome in ordered_outcomes(position, dice) {
            if !self.visit(should_stop) {
                return None;
            }
            let equity = self
                .chance_value(outcome.position(), depth - 1, should_stop)?
                .reversed();
            if better(equity, outcome.representative(), best.as_ref()) {
                best = Some((outcome.representative().clone(), equity));
            }
        }
        best.map(|(_, equity)| equity)
    }

    fn visit<F: Fn() -> bool>(&mut self, should_stop: &F) -> bool {
        if self.nodes >= self.node_limit || self.should_stop(should_stop) {
            self.stopped = true;
            return false;
        }
        self.nodes += 1;
        true
    }

    fn should_stop<F: Fn() -> bool>(&mut self, should_stop: &F) -> bool {
        if self.stopped || self.deadline.expired() || should_stop() {
            self.stopped = true;
            true
        } else {
            false
        }
    }

    fn probe(&mut self, key: u64, depth: u8) -> Option<Equity> {
        if self.table.is_empty() {
            return None;
        }
        let entry = self.table[key as usize % self.table.len()];
        if entry.occupied && entry.key == key && entry.depth == depth {
            self.tt_hits += 1;
            Some(entry.equity)
        } else {
            None
        }
    }

    fn store(&mut self, key: u64, depth: u8, equity: Equity) {
        if self.table.is_empty() {
            return;
        }
        let index = key as usize % self.table.len();
        self.table[index] = Entry {
            key,
            depth,
            equity,
            occupied: true,
        };
    }

    fn report(&self, best_play: Option<Play>, equity: Equity, depth: u8) -> SearchReport {
        SearchReport {
            best_play,
            equity,
            depth,
            nodes: self.nodes,
            chance_nodes: self.chance_nodes,
            tt_hits: self.tt_hits,
            stopped: self.stopped,
        }
    }
}

fn ordered_outcomes(position: Position, dice: Dice) -> Vec<PlayOutcome> {
    let mut outcomes = position.legal_outcomes(dice);
    outcomes.sort_by(|left, right| {
        evaluate_position(right.position())
            .reversed()
            .expected_points()
            .total_cmp(
                &evaluate_position(left.position())
                    .reversed()
                    .expected_points(),
            )
            .then_with(|| left.representative().cmp(right.representative()))
    });
    outcomes
}

fn better(equity: Equity, play: &Play, best: Option<&(Play, Equity)>) -> bool {
    let Some((best_play, best_equity)) = best else {
        return true;
    };
    equity.expected_points() > best_equity.expected_points()
        || (equity.expected_points() == best_equity.expected_points() && play < best_play)
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

#[cfg(all(target_arch = "wasm32", feature = "wasm"))]
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = performance, js_name = now)]
    fn performance_now() -> f64;
}

#[cfg(all(target_arch = "wasm32", feature = "wasm"))]
struct Deadline(Option<f64>);

#[cfg(all(target_arch = "wasm32", feature = "wasm"))]
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

#[cfg(all(target_arch = "wasm32", not(feature = "wasm")))]
struct Deadline;

#[cfg(all(target_arch = "wasm32", not(feature = "wasm")))]
impl Deadline {
    const fn disabled() -> Self {
        Self
    }

    const fn after(_milliseconds: u32) -> Self {
        Self
    }

    const fn expired(&self) -> bool {
        false
    }
}
