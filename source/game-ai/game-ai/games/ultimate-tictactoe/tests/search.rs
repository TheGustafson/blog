use ai_ultimate_tictactoe::{GameResult, Move, Player, Position, SearchOptions, Searcher};

fn macro_threat(player: Player) -> Position {
    let mut x = [0; 9];
    let mut o = [0; 9];
    match player {
        Player::X => {
            x[0] = 0b000_000_111;
            x[1] = 0b000_000_111;
            x[2] = 0b000_000_011;
            o[0] = 0b000_011_000;
            o[1] = 0b000_011_000;
            o[2] = 0b000_011_000;
            o[3] = 0b000_000_011;
            Position::from_cells(x, o, Some(2), Player::X).unwrap()
        }
        Player::O => {
            o[0] = 0b000_000_111;
            o[1] = 0b000_000_111;
            o[2] = 0b000_000_011;
            x[0] = 0b000_011_000;
            x[1] = 0b000_011_000;
            x[2] = 0b010_001_000;
            x[3] = 0b000_000_011;
            Position::from_cells(x, o, Some(2), Player::X).unwrap()
        }
    }
}

#[test]
fn search_takes_an_immediate_macro_win() {
    let position = macro_threat(Player::X);
    let report = Searcher::new().search(
        position,
        SearchOptions {
            max_depth: 2,
            node_limit: 20_000,
            soft_time_ms: 0,
        },
    );
    assert_eq!(report.best_move, Some(Move::new(2, 2)));
    assert_eq!(
        position.play(report.best_move.unwrap()).unwrap().result(),
        GameResult::Win(Player::X)
    );
    assert!(report.score > 29_000);
}

#[test]
fn search_does_not_route_the_opponent_into_an_immediate_macro_win() {
    let position = macro_threat(Player::O);
    let report = Searcher::new().search(
        position,
        SearchOptions {
            max_depth: 3,
            node_limit: 60_000,
            soft_time_ms: 0,
        },
    );
    let child = position.play(report.best_move.unwrap()).unwrap();
    assert!(
        child
            .legal_moves()
            .iter()
            .all(|reply| { child.play(reply).unwrap().result() != GameResult::Win(Player::O) })
    );
}

#[test]
fn iterative_deepening_keeps_the_last_completed_result_at_the_node_limit() {
    let report = Searcher::new().search(
        Position::start(),
        SearchOptions {
            max_depth: 20,
            node_limit: 2_000,
            soft_time_ms: 0,
        },
    );
    assert!(report.best_move.is_some());
    assert!(report.depth >= 1);
    assert!(report.depth < 20);
    assert!(report.nodes <= 2_000);
    assert!(
        Position::start()
            .legal_moves()
            .contains(report.best_move.unwrap())
    );
}

#[test]
fn transposition_table_is_reused_across_iterative_depths() {
    let mut searcher = Searcher::new();
    let report = searcher.search(
        Position::start(),
        SearchOptions {
            max_depth: 5,
            node_limit: 100_000,
            soft_time_ms: 0,
        },
    );
    assert!(report.depth >= 4);
    assert!(report.tt_hits > 0);
    assert!(!report.principal_variation.is_empty());
    assert_eq!(report.principal_variation[0], report.best_move.unwrap());
}

#[test]
fn fixed_limits_are_deterministic() {
    let options = SearchOptions {
        max_depth: 4,
        node_limit: 50_000,
        soft_time_ms: 0,
    };
    let first = Searcher::new().search(Position::start(), options);
    let second = Searcher::new().search(Position::start(), options);
    assert_eq!(first.best_move, second.best_move);
    assert_eq!(first.score, second.score);
    assert_eq!(first.depth, second.depth);
    assert_eq!(first.nodes, second.nodes);
    assert_eq!(first.principal_variation, second.principal_variation);
}

#[test]
fn browser_strength_limits_increase_monotonically() {
    use ai_ultimate_tictactoe::SEARCH_PRESETS;

    assert_eq!(
        SEARCH_PRESETS.map(|preset| preset.name),
        ["beginner", "easy", "medium", "hard", "expert", "maximum"]
    );
    assert_eq!(
        SEARCH_PRESETS.map(|preset| {
            (
                preset.options.max_depth,
                preset.options.node_limit,
                preset.options.soft_time_ms,
            )
        }),
        [
            (1, 500, 25),
            (2, 2_000, 40),
            (3, 10_000, 80),
            (5, 75_000, 250),
            (7, 300_000, 650),
            (20, 900_000, 1_000),
        ]
    );

    for pair in SEARCH_PRESETS.windows(2) {
        assert!(pair[0].options.max_depth < pair[1].options.max_depth);
        assert!(pair[0].options.node_limit < pair[1].options.node_limit);
        assert!(pair[0].options.soft_time_ms < pair[1].options.soft_time_ms);
    }
}

#[test]
fn soft_time_interrupts_a_deep_iteration() {
    let started = std::time::Instant::now();
    let report = Searcher::new().search(
        Position::start(),
        SearchOptions {
            max_depth: 20,
            node_limit: 10_000_000,
            soft_time_ms: 2,
        },
    );
    assert!(report.best_move.is_some());
    assert!(report.depth < 20);
    assert!(report.nodes < 10_000_000);
    assert!(started.elapsed() < std::time::Duration::from_secs(1));
}

#[test]
fn engine_can_play_a_complete_legal_game_against_itself() {
    let mut position = Position::start();
    let mut searcher = Searcher::new();
    let mut history = Vec::new();
    while position.result() == GameResult::Ongoing {
        let report = searcher.search(
            position,
            SearchOptions {
                max_depth: 3,
                node_limit: 20_000,
                soft_time_ms: 0,
            },
        );
        let mv = report.best_move.expect("ongoing games have legal moves");
        assert!(position.legal_moves().contains(mv));
        history.push(mv);
        position = position.play(mv).unwrap();
        assert!(position.ply() <= 81);
    }
    assert_eq!(history.len(), usize::from(position.ply()));
    assert!(!position.result().eq(&GameResult::Ongoing));
}
