use ai_connect4::{Algorithm, Move, Position, ScoreKind, SearchLimits, iterative_search, search};

fn position(notation: &[&str]) -> Position {
    let moves: Vec<Move> = notation
        .iter()
        .map(|value| value.parse().expect("valid test move"))
        .collect();
    Position::from_moves(&moves).expect("legal test position")
}

#[test]
fn every_search_configuration_returns_the_same_fixed_depth_answer() {
    let position = position(&["d", "c", "d", "e", "b", "f"]);
    let algorithms = [
        Algorithm::PlainNegamax,
        Algorithm::AlphaBeta,
        Algorithm::OrderedAlphaBeta,
        Algorithm::TranspositionTable,
    ];
    let reports: Vec<_> = algorithms
        .into_iter()
        .map(|algorithm| search(position, algorithm, SearchLimits::depth(7)))
        .collect();
    for report in &reports {
        assert!(report.completed);
        assert_eq!(report.best_move, reports[0].best_move);
        assert_eq!(report.score, reports[0].score);
        assert_eq!(report.root_branches.len(), position.legal_moves().len());
    }
    assert!(reports[1].stats.nodes < reports[0].stats.nodes);
    assert!(reports[2].stats.nodes < reports[1].stats.nodes);
    assert!(reports[3].stats.nodes < reports[2].stats.nodes);
    assert!(reports[1].stats.cutoffs > 0);
    assert!(reports[3].stats.tt_hits > 0);
    assert_eq!(
        reports
            .iter()
            .map(|report| report.stats.nodes)
            .collect::<Vec<_>>(),
        [888_422, 103_512, 20_100, 13_913]
    );
    assert_eq!(reports[0].best_move.unwrap().to_string(), "d");
    assert_eq!(reports[0].score.kind(), ScoreKind::Estimate(132));
    assert_eq!(reports[3].stats.tt_hits, 1_065);
}

#[test]
fn terminal_scores_cannot_be_confused_with_frontier_estimates() {
    let tactical = position(&["a", "b", "a", "b", "a", "b"]);
    let report = search(
        tactical,
        Algorithm::OrderedAlphaBeta,
        SearchLimits::depth(1),
    );
    assert_eq!(report.best_move.unwrap().to_string(), "a");
    assert_eq!(report.score.kind(), ScoreKind::ForcedWin { plies: 1 });

    let quiet = search(
        Position::start(),
        Algorithm::OrderedAlphaBeta,
        SearchLimits::depth(1),
    );
    assert!(matches!(quiet.score.kind(), ScoreKind::Estimate(_)));
}

#[test]
fn node_limits_are_exact_reproducible_and_restore_the_position() {
    let position = Position::start();
    let report = search(
        position,
        Algorithm::TranspositionTable,
        SearchLimits::with_nodes(12, 1_000),
    );
    assert!(!report.completed);
    assert_eq!(report.stats.nodes, 1_000);
    assert!(!report.root_branches.is_empty());
    assert_eq!(position, Position::start());
}

#[test]
fn transposition_bounds_do_not_change_completed_scores() {
    for position in [
        Position::start(),
        position(&["d", "c", "d", "e"]),
        position(&["a", "g", "b", "f", "c", "e"]),
    ] {
        let ordered = search(
            position,
            Algorithm::OrderedAlphaBeta,
            SearchLimits::depth(7),
        );
        let table = search(
            position,
            Algorithm::TranspositionTable,
            SearchLimits::depth(7),
        );
        assert!(ordered.completed && table.completed);
        assert_eq!(table.best_move, ordered.best_move);
        assert_eq!(table.score, ordered.score);
    }
}

#[test]
fn iterative_deepening_keeps_the_last_complete_result_under_a_budget() {
    let position = position(&["d", "c", "d", "e"]);
    let report = iterative_search(
        position,
        Algorithm::TranspositionTable,
        SearchLimits::with_nodes(12, 50_000),
    );
    assert_eq!(report.total_nodes, 50_000);
    assert!(!report.completed);
    assert!(report.completed_depth > 0);
    assert!(report.completed_depth < report.requested_depth);
    assert!(report.result.completed);
    assert_eq!(report.result.requested_depth, report.completed_depth);
    assert_eq!(
        report.iterations.first().map(|iteration| iteration.depth),
        Some(1)
    );
    assert!(
        report
            .iterations
            .last()
            .is_some_and(|iteration| !iteration.completed)
    );
}

#[test]
fn unbounded_iterative_search_matches_a_fixed_completed_depth() {
    let position = position(&["d", "c", "d", "e", "b", "f"]);
    let fixed = search(
        position,
        Algorithm::TranspositionTable,
        SearchLimits::depth(7),
    );
    let iterative = iterative_search(
        position,
        Algorithm::TranspositionTable,
        SearchLimits::depth(7),
    );
    assert!(iterative.completed);
    assert_eq!(iterative.completed_depth, 7);
    assert_eq!(iterative.result.best_move, fixed.best_move);
    assert_eq!(iterative.result.score, fixed.score);
    assert_eq!(iterative.result.principal_variation.len(), 7);
    assert_eq!(
        iterative.result.principal_variation.first().copied(),
        iterative.result.best_move
    );
}
