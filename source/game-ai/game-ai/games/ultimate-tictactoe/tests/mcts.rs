#![cfg(feature = "mcts")]

use ai_ultimate_tictactoe::{
    GameResult, MCTS_PRESETS, MctsOptions, MctsSearcher, MctsStrategy, Move, Player, Position,
};

fn macro_win() -> Position {
    let mut x = [0; 9];
    let mut o = [0; 9];
    x[0] = 0b000_000_111;
    x[1] = 0b000_000_111;
    x[2] = 0b000_000_011;
    o[0] = 0b000_011_000;
    o[1] = 0b000_011_000;
    o[2] = 0b000_011_000;
    o[3] = 0b000_000_011;
    Position::from_cells(x, o, Some(2), Player::X).unwrap()
}

fn options(max_simulations: u32, seed: u64) -> MctsOptions {
    MctsOptions {
        max_simulations,
        soft_time_ms: 0,
        exploration: std::f64::consts::SQRT_2,
        seed,
        strategy: MctsStrategy::UctRandom,
    }
}

fn puct_options(max_simulations: u32) -> MctsOptions {
    MctsOptions {
        max_simulations,
        soft_time_ms: 0,
        exploration: std::f64::consts::SQRT_2,
        seed: 1,
        strategy: MctsStrategy::PuctHandcrafted,
    }
}

fn learned_policy_options(max_simulations: u32) -> MctsOptions {
    MctsOptions {
        strategy: MctsStrategy::PuctLearned,
        ..puct_options(max_simulations)
    }
}

#[test]
fn fixed_seed_search_is_deterministic_and_preserves_the_position() {
    let position = Position::start();
    let first = MctsSearcher::new().search(position, options(500, 41));
    let second = MctsSearcher::new().search(position, options(500, 41));

    assert_eq!(position, Position::start());
    assert_eq!(first.best_move, second.best_move);
    assert_eq!(first.simulations, second.simulations);
    assert_eq!(first.tree_nodes, second.tree_nodes);
    assert_eq!(first.root_visits, second.root_visits);
    assert_eq!(first.expected_score, second.expected_score);
    assert_eq!(first.rollout_moves, second.rollout_moves);
    assert_eq!(first.root_moves, second.root_moves);
}

#[test]
fn tactical_rollouts_are_deterministic_and_report_their_work() {
    let mut tactical = options(500, 41);
    tactical.strategy = MctsStrategy::UctTactical;
    let first = MctsSearcher::new().search(Position::start(), tactical);
    let second = MctsSearcher::new().search(Position::start(), tactical);

    assert_eq!(first.best_move, second.best_move);
    assert_eq!(first.root_moves, second.root_moves);
    assert_eq!(first.rollout_moves, second.rollout_moves);
    assert_eq!(first.strategy, MctsStrategy::UctTactical);
    assert!(first.rollout_moves > u64::from(first.simulations));
}

#[test]
fn every_simulation_updates_the_root_and_one_root_move() {
    let position = Position::start();
    let report = MctsSearcher::new().search(position, options(250, 7));

    assert_eq!(report.simulations, 250);
    assert_eq!(report.root_visits, 250);
    assert_eq!(
        report
            .root_moves
            .iter()
            .map(|stats| stats.visits)
            .sum::<u32>(),
        250
    );
    assert!(report.tree_nodes <= report.simulations + 1);
    assert!(position.legal_moves().contains(report.best_move.unwrap()));
    assert!((0.0..=1.0).contains(&report.expected_score));
}

#[test]
fn random_rollout_uct_learns_an_immediate_macro_win() {
    let position = macro_win();
    let report = MctsSearcher::new().search(position, options(4_000, 9));

    assert_eq!(report.best_move, Some(Move::new(2, 2)));
    assert_eq!(
        position.play(report.best_move.unwrap()).unwrap().result(),
        GameResult::Win(Player::X)
    );
}

#[test]
fn puct_policy_prior_and_search_find_an_immediate_macro_win() {
    let position = macro_win();
    let report = MctsSearcher::new().search(position, puct_options(32));
    let winning = report
        .root_moves
        .iter()
        .find(|stats| stats.mv == Move::new(2, 2))
        .unwrap();

    assert_eq!(report.best_move, Some(Move::new(2, 2)));
    assert_eq!(
        winning.prior,
        report
            .root_moves
            .iter()
            .map(|stats| stats.prior)
            .fold(f64::NEG_INFINITY, f64::max)
    );
    assert_eq!(report.rollout_moves, 0);
    assert_eq!(report.leaf_evaluations, report.simulations);
}

#[test]
fn puct_priors_are_normalized_and_search_is_deterministic() {
    let first = MctsSearcher::new().search(Position::start(), puct_options(500));
    let second = MctsSearcher::new().search(Position::start(), puct_options(500));

    assert_eq!(first.best_move, second.best_move);
    assert_eq!(first.root_moves, second.root_moves);
    assert_eq!(first.strategy, MctsStrategy::PuctHandcrafted);
    assert!(
        (first
            .root_moves
            .iter()
            .map(|stats| stats.prior)
            .sum::<f64>()
            - 1.0)
            .abs()
            < 1e-9
    );
    assert!(
        first
            .root_moves
            .iter()
            .all(|stats| stats.prior > 0.0 && stats.visits > 0)
    );
}

#[test]
fn learned_policy_puct_is_deterministic_and_uses_no_rollouts() {
    let first = MctsSearcher::new().search(Position::start(), learned_policy_options(500));
    let second = MctsSearcher::new().search(Position::start(), learned_policy_options(500));

    assert_eq!(first.best_move, second.best_move);
    assert_eq!(first.root_moves, second.root_moves);
    assert_eq!(first.strategy, MctsStrategy::PuctLearned);
    assert_eq!(first.rollout_moves, 0);
    assert_eq!(first.leaf_evaluations, 500);
    assert!(
        (first
            .root_moves
            .iter()
            .map(|stats| stats.prior)
            .sum::<f64>()
            - 1.0)
            .abs()
            < 1e-6
    );
}

#[test]
fn terminal_positions_do_not_run_simulations() {
    let terminal = macro_win().play(Move::new(2, 2)).unwrap();
    let report = MctsSearcher::new().search(terminal, options(1_000, 1));

    assert_eq!(report.best_move, None);
    assert_eq!(report.simulations, 0);
    assert_eq!(report.root_visits, 0);
    assert_eq!(report.tree_nodes, 1);
    assert!(report.root_moves.is_empty());
}

#[test]
fn soft_time_interrupts_a_large_simulation_budget() {
    let started = std::time::Instant::now();
    let report = MctsSearcher::new().search(
        Position::start(),
        MctsOptions {
            max_simulations: 1_000_000,
            soft_time_ms: 2,
            exploration: std::f64::consts::SQRT_2,
            seed: 1,
            strategy: MctsStrategy::UctTactical,
        },
    );

    assert!(report.simulations > 0);
    assert!(report.simulations < 1_000_000);
    assert!(started.elapsed() < std::time::Duration::from_secs(1));
}

#[test]
fn browser_presets_lock_monotonic_simulation_and_time_limits() {
    assert_eq!(
        MCTS_PRESETS.map(|preset| preset.name),
        ["beginner", "easy", "medium", "hard", "expert", "maximum"]
    );
    assert_eq!(
        MCTS_PRESETS.map(|preset| (preset.options.max_simulations, preset.options.soft_time_ms)),
        [
            (100, 25),
            (500, 50),
            (2_000, 100),
            (10_000, 250),
            (40_000, 650),
            (100_000, 1_000),
        ]
    );
    for pair in MCTS_PRESETS.windows(2) {
        assert!(pair[0].options.max_simulations < pair[1].options.max_simulations);
        assert!(pair[0].options.soft_time_ms < pair[1].options.soft_time_ms);
    }
    assert!(
        MCTS_PRESETS
            .iter()
            .all(|preset| preset.options.strategy == MctsStrategy::PuctLearned)
    );
}
