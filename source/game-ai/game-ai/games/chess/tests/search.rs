use ai_chess::{
    EvaluationProfile, GameResult, Position, ScoreKind, SearchConfig, iterative_search, search,
    search_with_history,
};

#[test]
fn published_search_presets_increase_monotonically() {
    use ai_chess::{SEARCH_PRESETS, search_preset};

    assert_eq!(
        SEARCH_PRESETS.map(|preset| preset.name),
        ["beginner", "easy", "medium", "hard", "expert", "maximum"]
    );
    assert_eq!(
        SEARCH_PRESETS.map(|preset| (
            preset.config.depth,
            preset.config.nodes,
            preset.config.time_millis,
        )),
        [
            (2, Some(1_000), Some(25)),
            (3, Some(5_000), Some(50)),
            (4, Some(15_000), Some(100)),
            (5, Some(40_000), Some(250)),
            (7, Some(120_000), Some(500)),
            (64, Some(300_000), Some(1_000)),
        ]
    );
    assert_eq!(
        SEARCH_PRESETS.map(|preset| (
            preset.config.quiescence,
            preset.config.move_ordering,
            preset.config.transposition_table,
        )),
        [
            (false, false, false),
            (false, true, true),
            (true, true, true),
            (true, true, true),
            (true, true, true),
            (true, true, true),
        ]
    );
    for pair in SEARCH_PRESETS.windows(2) {
        assert!(pair[0].config.depth < pair[1].config.depth);
        assert!(pair[0].config.nodes < pair[1].config.nodes);
        assert!(pair[0].config.time_millis < pair[1].config.time_millis);
    }
    assert!(
        SEARCH_PRESETS
            .iter()
            .all(|preset| preset.config.time_millis <= Some(1_000))
    );
    assert!(
        SEARCH_PRESETS
            .iter()
            .all(|preset| preset.config.evaluator == EvaluationProfile::TinyNnue)
    );
    for preset in SEARCH_PRESETS {
        assert_eq!(search_preset(preset.name), Some(preset));
    }
    assert_eq!(search_preset("unknown"), None);
}

fn config(depth: u8, evaluator: EvaluationProfile) -> SearchConfig {
    SearchConfig::classical(depth, evaluator)
}

#[test]
fn material_and_piece_square_make_a_controlled_opening_disagreement() {
    let material = search(Position::start(), config(1, EvaluationProfile::Material));
    let piece_square = search(Position::start(), config(1, EvaluationProfile::PieceSquare));
    assert_eq!(material.best_move.unwrap().to_string(), "b1a3");
    assert_eq!(piece_square.best_move.unwrap().to_string(), "d2d4");
    assert_eq!(material.stats.nodes, 21);
    assert_eq!(piece_square.stats.nodes, 21);
    assert_eq!(material.score.kind(), ScoreKind::Centipawns(0));
    assert_eq!(piece_square.score.kind(), ScoreKind::Centipawns(41));
    assert_ne!(material.best_move, piece_square.best_move);
}

#[test]
fn mate_scores_remain_distinct_from_large_static_scores() {
    let position = Position::from_fen("7k/5Q2/6K1/8/8/8/8/8 w - - 0 1").unwrap();
    let report = search(position.clone(), config(2, EvaluationProfile::PieceSquare));
    assert!(matches!(report.score.kind(), ScoreKind::MateIn { .. }));
    let mv = report.best_move.expect("white has a mating move");
    let mut after = position;
    after.make_move(mv).unwrap();
    assert!(matches!(after.result(), GameResult::Checkmate { .. }));
}

#[test]
fn insufficient_material_is_an_exact_search_draw() {
    let position = Position::from_fen("7k/8/8/8/8/8/8/KN6 w - - 0 1").unwrap();
    let report = search(position, config(4, EvaluationProfile::PieceSquare));
    assert_eq!(report.score.kind(), ScoreKind::Draw);
    assert!(report.best_move.is_none());
    assert_eq!(report.stats.nodes, 1);
}

#[test]
fn quiescence_sees_the_recapture_beyond_the_nominal_horizon() {
    let position = Position::from_fen("4k3/8/4p3/3r4/8/8/8/3QK3 w - - 0 1").unwrap();
    let mut without = config(1, EvaluationProfile::Material);
    without.quiescence = false;
    without.transposition_table = false;
    let mut with = without;
    with.quiescence = true;
    let horizon = search(position.clone(), without);
    let quiet = search(position, with);
    assert_eq!(horizon.best_move.unwrap().to_string(), "d1d5");
    assert_eq!(quiet.best_move.unwrap().to_string(), "d1a1");
    assert_eq!(horizon.score.kind(), ScoreKind::Centipawns(858));
    assert_eq!(quiet.score.kind(), ScoreKind::Centipawns(364));
    assert_eq!((horizon.stats.nodes, horizon.stats.qnodes), (18, 0));
    assert_eq!((quiet.stats.nodes, quiet.stats.qnodes), (48, 47));
}

#[test]
fn move_ordering_preserves_the_published_answer_and_reduces_work() {
    let position =
        Position::from_fen("r1bqkbnr/pppp1ppp/2n5/4p3/4P3/5N2/PPPP1PPP/RNBQKB1R w KQkq - 2 3")
            .unwrap();
    let mut natural = config(3, EvaluationProfile::PieceSquare);
    natural.quiescence = false;
    natural.move_ordering = false;
    natural.transposition_table = false;
    let mut ordered = natural;
    ordered.move_ordering = true;

    let baseline = iterative_search(position.clone(), natural);
    let improved = iterative_search(position, ordered);
    assert_eq!(baseline.result.best_move.unwrap().to_string(), "f1b5");
    assert_eq!(improved.result.best_move, baseline.result.best_move);
    assert_eq!(baseline.result.score.kind(), ScoreKind::Centipawns(129));
    assert_eq!(improved.result.score, baseline.result.score);
    assert_eq!(baseline.total_nodes, 14_387);
    assert_eq!(improved.total_nodes, 4_492);
}

#[test]
fn transposition_table_preserves_the_fixed_depth_answer() {
    // Two independent knights produce real move-order transpositions without
    // making this correctness test pay the cost of a full opening tree.
    let position = Position::from_fen("4k3/8/8/8/8/8/P6P/1N2K1N1 w - - 0 1").unwrap();
    let mut without = config(5, EvaluationProfile::PieceSquare);
    without.transposition_table = false;
    without.quiescence = false;
    let mut with = without;
    with.transposition_table = true;
    let baseline = search(position.clone(), without);
    let table = search(position, with);
    assert!(baseline.completed && table.completed);
    assert_eq!(table.best_move, baseline.best_move);
    assert_eq!(table.score, baseline.score);
    assert!(table.stats.tt_hits > 0);
}

#[test]
fn iterative_budget_is_exact_and_returns_the_last_complete_iteration() {
    let report = iterative_search(
        Position::start(),
        config(6, EvaluationProfile::PieceSquare).with_nodes(500),
    );
    assert_eq!(report.total_nodes, 500);
    assert!(report.completed_depth >= 1);
    assert!(report.completed_depth < report.requested_depth);
    assert_eq!(
        report.result.config.depth, report.completed_depth,
        "partial work must not replace the last complete answer"
    );
    assert_eq!(
        report
            .iterations
            .iter()
            .map(|iteration| (iteration.nodes, iteration.completed))
            .collect::<Vec<_>>(),
        [(21, true), (429, true), (50, false)]
    );
    assert_eq!(report.result.best_move.unwrap().to_string(), "d2d4");
    assert_eq!(report.result.score.kind(), ScoreKind::Centipawns(0));
    assert_eq!(
        report
            .result
            .principal_variation
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>(),
        ["d2d4", "d7d5"]
    );
}

#[test]
fn supplied_history_detects_exact_threefold_without_hiding_checkmate() {
    let position = Position::start();
    let key = position.key();
    let report = search_with_history(
        position,
        config(3, EvaluationProfile::PieceSquare),
        &[key, 123, key],
    );
    assert_eq!(report.score.kind(), ScoreKind::Draw);
    assert!(report.best_move.is_none());

    let mate = Position::from_fen("7k/6Q1/6K1/8/8/8/8/8 b - - 0 1").unwrap();
    let mate_key = mate.key();
    let report = search_with_history(
        mate,
        config(1, EvaluationProfile::PieceSquare),
        &[mate_key, mate_key],
    );
    assert!(matches!(
        report.score.kind(),
        ScoreKind::MatedIn { plies: 0 }
    ));
}

#[test]
fn incremental_nnue_is_search_equivalent_to_refreshing_every_leaf() {
    let position = Position::start();
    let mut refreshed = config(3, EvaluationProfile::TinyNnue);
    refreshed.incremental_nnue = false;
    let mut incremental = refreshed;
    incremental.incremental_nnue = true;

    let refresh_search = iterative_search(position.clone(), refreshed);
    let incremental_search = iterative_search(position, incremental);
    let refresh_report = refresh_search.result;
    let incremental_report = incremental_search.result;
    assert!(refresh_report.completed && incremental_report.completed);
    assert_eq!(incremental_report.best_move, refresh_report.best_move);
    assert_eq!(incremental_report.score, refresh_report.score);
    assert_eq!(
        incremental_report.principal_variation,
        refresh_report.principal_variation
    );
    assert_eq!(incremental_report.stats.nodes, refresh_report.stats.nodes);
    assert_eq!(
        refresh_report.stats.nnue_refreshes,
        refresh_report.stats.evaluations
    );
    assert_eq!(incremental_report.stats.nnue_refreshes, 1);
    assert_eq!(refresh_report.best_move.unwrap().to_string(), "d2d4");
    assert_eq!(refresh_report.score.kind(), ScoreKind::Centipawns(33));
    assert_eq!(refresh_search.total_nodes, 2_747);
    assert_eq!(incremental_search.total_nodes, 2_747);
    assert_eq!(refresh_report.stats.nnue_refreshes, 1_839);
    assert_eq!(incremental_report.stats.nnue_updates, 2_287);
    assert_eq!(incremental_report.stats.nnue_feature_changes, 4_916);
    assert_eq!(refresh_report.stats.nnue_accumulator_ops, 14_922_496);
    assert_eq!(incremental_report.stats.nnue_accumulator_ops, 2_525_184);
    assert_eq!(
        refresh_report
            .principal_variation
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>(),
        ["d2d4", "d7d5", "b2b4"]
    );
}

#[test]
fn wrong_bishop_search_disagreement_is_locked_to_the_same_one_ply_tree() {
    let position = Position::from_fen("k7/P3K3/8/8/3B4/8/8/8 w - - 67 130").unwrap();
    let expected = [
        (EvaluationProfile::Material, "d4a1", 393),
        (EvaluationProfile::PieceSquare, "e7f6", 636),
        (EvaluationProfile::TinyNnue, "d4a1", 176),
    ];
    for (evaluator, best_move, score) in expected {
        let mut settings = config(1, evaluator);
        settings.quiescence = false;
        let report = search(position.clone(), settings);
        assert_eq!(report.best_move.unwrap().to_string(), best_move);
        assert_eq!(report.score.kind(), ScoreKind::Centipawns(score));
        assert_eq!(report.stats.nodes, 21);
    }
}
