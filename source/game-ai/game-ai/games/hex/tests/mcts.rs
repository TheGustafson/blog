use ai_hex::{
    BoardSize, GameResult, KnowledgePolicy, MCTS_PRESETS, MctsOptions, MctsSearcher, MctsStrategy,
    Move, Position, RolloutPolicy, SwapRule,
};

fn start(size: u8) -> Position {
    Position::new(BoardSize::new(size).unwrap(), SwapRule::Enabled)
}

#[test]
fn presets_expose_the_intended_monotonic_work_limits() {
    let limits =
        MCTS_PRESETS.map(|preset| (preset.options.max_simulations, preset.options.soft_time_ms));
    assert_eq!(
        limits,
        [
            (200, 50),
            (1_000, 100),
            (4_000, 200),
            (20_000, 500),
            (80_000, 1_300),
            (200_000, 2_000),
        ]
    );
    assert!(
        limits
            .windows(2)
            .all(|pair| { pair[0].0 < pair[1].0 && pair[0].1 < pair[1].1 })
    );
    assert!(MCTS_PRESETS.iter().all(|preset| {
        preset.options.exploration == 0.2
            && preset.options.rollout_policy == RolloutPolicy::SaveBridge
            && preset.options.knowledge_policy == KnowledgePolicy::InferiorCells { min_visits: 32 }
    }));
}

#[test]
fn fixed_simulation_search_is_reproducible() {
    let position = start(9).play("e5".parse().unwrap()).unwrap();
    let options = MctsOptions {
        max_simulations: 300,
        soft_time_ms: 0,
        exploration: std::f64::consts::SQRT_2,
        strategy: MctsStrategy::UctRave,
        rave_equivalence: 1_000.0,
        rollout_policy: RolloutPolicy::SaveBridge,
        knowledge_policy: KnowledgePolicy::InferiorCells { min_visits: 128 },
        use_virtual_connections: true,
        seed: 42,
    };
    let first = MctsSearcher::new().search(position, options);
    let second = MctsSearcher::new().search(position, options);
    assert_eq!(first.best_move, second.best_move);
    assert_eq!(first.simulations, 300);
    assert_eq!(first.root_visits, 300);
    assert_eq!(first.tree_nodes, second.tree_nodes);
    assert_eq!(first.root_moves, second.root_moves);
    assert_eq!(first.rollout_moves, second.rollout_moves);
}

#[test]
fn search_expands_every_legal_root_action() {
    let position = start(9).play("e5".parse().unwrap()).unwrap();
    let legal = position.legal_moves();
    let report = MctsSearcher::new().search(
        position,
        MctsOptions {
            max_simulations: legal.len() as u32,
            soft_time_ms: 0,
            exploration: std::f64::consts::SQRT_2,
            strategy: MctsStrategy::UctRave,
            rave_equivalence: 1_000.0,
            rollout_policy: RolloutPolicy::SaveBridge,
            knowledge_policy: KnowledgePolicy::InferiorCells { min_visits: 128 },
            use_virtual_connections: true,
            seed: 7,
        },
    );
    assert_eq!(report.root_moves.len(), legal.len());
    assert!(report.root_moves.iter().any(|stats| stats.mv == Move::Swap));
    assert!(report.root_moves.iter().all(|stats| stats.visits == 1));
}

#[test]
fn report_contains_a_legal_move_and_consistent_counts() {
    let position = start(15);
    let report = MctsSearcher::new().search(
        position,
        MctsOptions {
            max_simulations: 250,
            soft_time_ms: 0,
            exploration: 1.2,
            strategy: MctsStrategy::UctRave,
            rave_equivalence: 1_000.0,
            rollout_policy: RolloutPolicy::SaveBridge,
            knowledge_policy: KnowledgePolicy::InferiorCells { min_visits: 128 },
            use_virtual_connections: true,
            seed: 9,
        },
    );
    assert!(position.is_legal(report.best_move.unwrap()));
    assert_eq!(report.simulations, 250);
    assert_eq!(report.root_visits, report.simulations);
    assert!(report.tree_nodes > 1);
    assert!((0.0..=1.0).contains(&report.expected_score));
    assert!(report.rollout_moves > 0);
}

#[test]
fn rave_accumulates_amaf_evidence_without_treating_swap_as_a_stone() {
    let position = start(9).play("e5".parse().unwrap()).unwrap();
    let report = MctsSearcher::new().search(
        position,
        MctsOptions {
            max_simulations: 500,
            soft_time_ms: 0,
            exploration: std::f64::consts::SQRT_2,
            strategy: MctsStrategy::UctRave,
            rave_equivalence: 1_000.0,
            rollout_policy: RolloutPolicy::SaveBridge,
            knowledge_policy: KnowledgePolicy::InferiorCells { min_visits: 128 },
            use_virtual_connections: true,
            seed: 19,
        },
    );
    let swap = report
        .root_moves
        .iter()
        .find(|stats| stats.mv == Move::Swap)
        .unwrap();
    assert_eq!(swap.rave_visits, 0);
    assert!(
        report
            .root_moves
            .iter()
            .filter(|stats| stats.mv != Move::Swap)
            .all(|stats| stats.rave_visits > 0)
    );
}

#[test]
fn plain_uct_keeps_the_amaf_channel_empty() {
    let report = MctsSearcher::new().search(
        start(9),
        MctsOptions {
            max_simulations: 250,
            soft_time_ms: 0,
            exploration: std::f64::consts::SQRT_2,
            strategy: MctsStrategy::PlainUct,
            rave_equivalence: 1_000.0,
            rollout_policy: RolloutPolicy::SaveBridge,
            knowledge_policy: KnowledgePolicy::InferiorCells { min_visits: 128 },
            use_virtual_connections: true,
            seed: 23,
        },
    );
    assert_eq!(report.strategy, MctsStrategy::PlainUct);
    assert!(report.root_moves.iter().all(|stats| stats.rave_visits == 0));
}

#[test]
fn terminal_position_has_no_search_move() {
    let mut moves = Vec::new();
    for rank in 1..=9 {
        moves.push(format!("a{rank}").parse::<Move>().unwrap());
        if rank < 9 {
            moves.push(format!("h{rank}").parse::<Move>().unwrap());
        }
    }
    let position =
        Position::from_moves(BoardSize::new(9).unwrap(), SwapRule::Disabled, &moves).unwrap();
    assert!(matches!(position.result(), GameResult::Win(_)));
    let report = MctsSearcher::new().search(position, MctsOptions::default());
    assert_eq!(report.best_move, None);
    assert_eq!(report.simulations, 0);
    assert_eq!(report.tree_nodes, 1);
}

#[test]
fn root_knowledge_finds_an_immediate_win_with_one_simulation() {
    let moves = [
        "a1", "h1", "a2", "h2", "a3", "h3", "a4", "h4", "a5", "h5", "a6", "h6", "a7", "h7", "a8",
        "h8",
    ]
    .map(|mv| mv.parse().unwrap());
    let position =
        Position::from_moves(BoardSize::new(9).unwrap(), SwapRule::Disabled, &moves).unwrap();
    let report = MctsSearcher::new().search(
        position,
        MctsOptions {
            max_simulations: 1,
            soft_time_ms: 0,
            exploration: 0.4,
            strategy: MctsStrategy::UctRave,
            rave_equivalence: 1_000.0,
            rollout_policy: RolloutPolicy::SaveBridge,
            knowledge_policy: KnowledgePolicy::InferiorCells { min_visits: 128 },
            use_virtual_connections: true,
            seed: 1,
        },
    );

    assert_eq!(report.best_move, Some("a9".parse().unwrap()));
    assert_eq!(report.root_must_play_moves, 1);
    assert_eq!(report.root_moves.len(), 1);
}

#[test]
fn root_knowledge_does_not_expand_a_neighborhood_dominated_move() {
    let moves = ["a9", "d5", "b9", "e4", "c9", "d6"].map(|mv| mv.parse().unwrap());
    let position =
        Position::from_moves(BoardSize::new(9).unwrap(), SwapRule::Disabled, &moves).unwrap();
    let report = MctsSearcher::new().search(
        position,
        MctsOptions {
            max_simulations: position.legal_moves().len() as u32,
            soft_time_ms: 0,
            exploration: 0.4,
            strategy: MctsStrategy::UctRave,
            rave_equivalence: 1_000.0,
            rollout_policy: RolloutPolicy::SaveBridge,
            knowledge_policy: KnowledgePolicy::InferiorCells { min_visits: 128 },
            use_virtual_connections: true,
            seed: 7,
        },
    );

    assert!(report.root_pruned_moves > 0);
    assert!(
        !report
            .root_moves
            .iter()
            .any(|stats| stats.mv == "e5".parse().unwrap())
    );
    assert!(
        report
            .root_moves
            .iter()
            .any(|stats| stats.mv == "f5".parse().unwrap())
    );
    assert_eq!(
        report.root_moves.len() + report.root_pruned_moves as usize,
        position.legal_moves().len()
    );
}
