use ai_othello::{EvaluationProfile, Move, Position, ScoreKind, SearchConfig, search};

#[test]
fn alpha_beta_returns_a_stable_opening_signature_for_each_evaluator() {
    let expected = [
        (EvaluationProfile::Material, "d3"),
        (EvaluationProfile::Mobility, "d3"),
        (EvaluationProfile::Corners, "d3"),
        (EvaluationProfile::Frontier, "d3"),
        (EvaluationProfile::Phase, "d3"),
    ];
    for (profile, best_move) in expected {
        let report = search(Position::start(), SearchConfig::fixed_depth(5, profile));
        assert_eq!(report.best_move.unwrap().to_string(), best_move);
        assert_eq!(report.principal_variation.len(), 5);
        assert!(report.stats.nodes > 0);
        assert_eq!(report.evaluation.total, 0);
        assert!(!report.exact);
    }
}

#[test]
fn visible_evaluator_choices_can_change_the_move() {
    let moves = ["d3", "c3", "b3", "b2", "b1", "a1", "c4", "c1"]
        .map(|notation| notation.parse::<Move>().unwrap());
    let position = Position::from_moves(&moves).unwrap();

    let material = search(
        position,
        SearchConfig::fixed_depth(5, EvaluationProfile::Material),
    );
    let adaptive = search(
        position,
        SearchConfig::fixed_depth(5, EvaluationProfile::Phase),
    );

    assert_eq!(material.best_move.unwrap().to_string(), "f5");
    assert_eq!(adaptive.best_move.unwrap().to_string(), "c2");
    assert_ne!(material.best_move, adaptive.best_move);
}

#[test]
fn passes_survive_the_negamax_perspective_change() {
    let black = 1u64 << "b1".parse::<ai_othello::Square>().unwrap().index();
    let white = 1u64 << "a1".parse::<ai_othello::Square>().unwrap().index();
    let position = Position::from_bits(black, white, ai_othello::Side::Black).unwrap();
    let report = search(
        position,
        SearchConfig::fixed_depth(1, EvaluationProfile::Phase),
    );
    assert_eq!(report.best_move, Some(Move::Pass));
    assert_eq!(report.principal_variation.first(), Some(&Move::Pass));
    assert!(report.stats.passes > 0);
}

#[test]
fn exact_endgame_search_ignores_the_heuristic_horizon() {
    let mut position = Position::start();
    while position.empty_count() > 8 && position.result() == ai_othello::GameResult::Ongoing {
        let moves = position.legal_moves();
        let index = (usize::from(position.occupied_count()) * 7 + 3) % moves.len();
        position.make_move(moves.as_slice()[index]).unwrap();
    }
    assert_eq!(position.empty_count(), 8);

    let heuristic = search(
        position,
        SearchConfig::fixed_depth(0, EvaluationProfile::Material),
    );
    assert!(matches!(heuristic.score.kind(), ScoreKind::Estimate(_)));

    let material_exact = search(
        position,
        SearchConfig {
            depth: 0,
            evaluator: EvaluationProfile::Material,
            exact_endgame_empties: 8,
        },
    );
    let phase_exact = search(
        position,
        SearchConfig {
            depth: 0,
            evaluator: EvaluationProfile::Phase,
            exact_endgame_empties: 8,
        },
    );
    assert!(material_exact.exact && phase_exact.exact);
    assert_eq!(material_exact.best_move, phase_exact.best_move);
    assert_eq!(material_exact.score, phase_exact.score);
    assert!(material_exact.stats.exact_nodes > 0);
}
