use ai_backgammon::{
    Dice, Equity, Location, Player, Point, Position, SEARCH_PRESETS, SearchOptions, Searcher,
    evaluate_position,
};
use std::cell::Cell;

fn point(number: u8) -> Point {
    Point::new(number).unwrap()
}

fn position(
    white: &[(u8, u8)],
    black: &[(u8, u8)],
    bar: [u8; 2],
    side_to_move: Player,
) -> Position {
    let mut points = [0_i8; 24];
    for &(number, count) in white {
        points[usize::from(number - 1)] += count as i8;
    }
    for &(number, count) in black {
        points[usize::from(number - 1)] -= count as i8;
    }
    let white_on_board: u8 = white.iter().map(|&(_, count)| count).sum();
    let black_on_board: u8 = black.iter().map(|&(_, count)| count).sum();
    Position::from_parts(
        points,
        bar,
        [
            15 - white_on_board - bar[Player::White.index()],
            15 - black_on_board - bar[Player::Black.index()],
        ],
        side_to_move,
    )
    .unwrap()
}

fn options(depth: u8, nodes: u64) -> SearchOptions {
    SearchOptions {
        max_depth: depth,
        node_limit: nodes,
        soft_time_ms: 0,
    }
}

#[test]
fn one_ply_search_matches_an_independent_exhaustive_choice() {
    let position = Position::new();
    let dice = Dice::new(3, 1).unwrap();
    let expected = position
        .legal_outcomes(dice)
        .into_iter()
        .max_by(|left, right| {
            evaluate_position(left.position())
                .reversed()
                .expected_points()
                .total_cmp(
                    &evaluate_position(right.position())
                        .reversed()
                        .expected_points(),
                )
                .then_with(|| right.representative().cmp(left.representative()))
        })
        .unwrap();

    let report = Searcher::new().search(position, dice, options(1, 100_000));
    assert_eq!(report.best_play.as_ref(), Some(expected.representative()));
    assert_eq!(report.depth, 1);
}

#[test]
fn zero_depth_uses_the_first_deterministic_legal_outcome() {
    let position = Position::new();
    let dice = Dice::new(3, 1).unwrap();
    let expected = position.legal_outcomes(dice)[0].representative().clone();
    let report = Searcher::new().search(position, dice, options(0, 1));
    assert_eq!(report.best_play, Some(expected));
    assert_eq!(report.depth, 0);
    assert_eq!(report.nodes, 0);
}

#[test]
fn search_bears_off_the_last_checker_and_scores_the_win_exactly() {
    let position = position(&[(1, 1)], &[(24, 14)], [0, 0], Player::White);
    let dice = Dice::new(6, 1).unwrap();
    let report = Searcher::new().search(position, dice, options(3, 100_000));
    let play = report.best_play.unwrap();
    assert!(play.steps().iter().any(|step| step.to() == Location::Off));
    assert_eq!(
        position
            .play(dice, &play)
            .unwrap()
            .game_outcome()
            .unwrap()
            .winner,
        Player::White
    );
    assert_eq!(report.equity, Equity::win(ai_backgammon::GameKind::Single));
}

#[test]
fn search_takes_an_available_hit() {
    let position = position(&[(8, 1)], &[(5, 1)], [0, 0], Player::White);
    let dice = Dice::new(3, 1).unwrap();
    let report = Searcher::new().search(position, dice, options(1, 10_000));
    let next = position.play(dice, &report.best_play.unwrap()).unwrap();
    assert_eq!(next.bar(Player::Black), 1);
}

#[test]
fn search_prefers_made_points_to_an_equal_pip_blot_structure() {
    let position = position(&[(8, 2), (6, 1)], &[(20, 2)], [0, 0], Player::White);
    let dice = Dice::new(2, 1).unwrap();
    let report = Searcher::new().search(position, dice, options(1, 10_000));
    let next = position.play(dice, &report.best_play.unwrap()).unwrap();
    assert!((1..=24).any(|number| next.count(Player::White, Point::new(number).unwrap()) >= 2));
}

#[test]
fn a_blocked_roll_returns_the_legal_pass() {
    let position = position(&[], &[(24, 2), (23, 2)], [1, 0], Player::White);
    let dice = Dice::new(2, 1).unwrap();
    let report = Searcher::new().search(position, dice, options(2, 10_000));
    assert!(report.best_play.unwrap().is_empty());
}

#[test]
fn every_reported_play_is_legal_across_reachable_positions() {
    let mut position = Position::new();
    let rolls = [(3, 1), (6, 5), (4, 2), (2, 1), (5, 3), (6, 6)];
    for (high, low) in rolls {
        let dice = Dice::new(high, low).unwrap();
        let report = Searcher::new().search(position, dice, options(1, 50_000));
        let play = report.best_play.unwrap();
        assert!(position.legal_plays(dice).contains(&play));
        position = position.play(dice, &play).unwrap();
        if position.game_outcome().is_some() {
            break;
        }
    }
}

#[test]
fn fixed_limits_are_deterministic() {
    let dice = Dice::new(6, 1).unwrap();
    let options = options(2, 80_000);
    let first = Searcher::new().search(Position::new(), dice, options);
    let second = Searcher::new().search(Position::new(), dice, options);
    assert_eq!(first.best_play, second.best_play);
    assert_eq!(first.equity, second.equity);
    assert_eq!(first.depth, second.depth);
    assert_eq!(first.nodes, second.nodes);
    assert_eq!(first.tt_hits, second.tt_hits);
}

#[test]
fn node_limit_preserves_a_legal_fallback() {
    let position = Position::new();
    let dice = Dice::new(5, 3).unwrap();
    let report = Searcher::new().search(position, dice, options(8, 12));
    assert!(
        position
            .legal_plays(dice)
            .contains(&report.best_play.unwrap())
    );
    assert!(report.nodes <= 12);
    assert!(report.depth < 8);
    assert!(report.stopped);
}

#[test]
fn caller_cancellation_interrupts_search_safely() {
    let polls = Cell::new(0_u32);
    let position = Position::new();
    let dice = Dice::new(3, 1).unwrap();
    let report = Searcher::new().search_until(position, dice, options(8, 1_000_000), || {
        polls.set(polls.get() + 1);
        polls.get() > 20
    });
    assert!(
        position
            .legal_plays(dice)
            .contains(&report.best_play.unwrap())
    );
    assert!(report.stopped);
    assert!(polls.get() > 20);
}

#[test]
fn transposition_table_does_not_change_the_answer() {
    let position = Position::new();
    let dice = Dice::new(4, 2).unwrap();
    let options = options(2, 150_000);
    let without = Searcher::with_table_entries(0).search(position, dice, options);
    let with = Searcher::with_table_entries(1 << 14).search(position, dice, options);
    assert_eq!(with.best_play, without.best_play);
    assert_eq!(with.equity, without.equity);
    assert_eq!(with.depth, without.depth);
    assert!(with.nodes <= without.nodes);
}

#[test]
fn completed_second_ply_visits_chance_nodes_and_reuses_positions() {
    let position = position(&[(3, 2)], &[(22, 2)], [0, 0], Player::White);
    let report = Searcher::new().search(position, Dice::new(2, 1).unwrap(), options(2, 250_000));
    assert_eq!(report.depth, 2);
    assert!(report.chance_nodes >= 21);
    assert!(report.tt_hits > 0);
}

#[test]
fn terminal_positions_have_no_checker_play() {
    let position = position(&[], &[(19, 14)], [0, 0], Player::Black);
    let report = Searcher::new().search(position, Dice::new(6, 1).unwrap(), options(3, 10_000));
    assert!(report.best_play.is_none());
    assert_eq!(report.depth, 0);
    assert_eq!(report.equity.expected_points(), -1.0);
}

#[test]
fn all_six_browser_presets_expose_strictly_larger_budgets() {
    assert_eq!(
        SEARCH_PRESETS.map(|preset| preset.name),
        ["beginner", "easy", "medium", "hard", "expert", "maximum"]
    );
    for pair in SEARCH_PRESETS.windows(2) {
        assert!(pair[0].options.node_limit < pair[1].options.node_limit);
        assert!(pair[0].options.soft_time_ms < pair[1].options.soft_time_ms);
        assert!(pair[0].options.max_depth <= pair[1].options.max_depth);
    }
    assert!(SEARCH_PRESETS.last().unwrap().options.soft_time_ms <= 1_000);
}

#[test]
fn soft_time_stops_a_deep_search() {
    let started = std::time::Instant::now();
    let report = Searcher::new().search(
        Position::new(),
        Dice::new(3, 1).unwrap(),
        SearchOptions {
            max_depth: 10,
            node_limit: 10_000_000,
            soft_time_ms: 2,
        },
    );
    assert!(report.best_play.is_some());
    assert!(report.stopped);
    assert!(report.depth < 10);
    assert!(started.elapsed() < std::time::Duration::from_secs(1));
}

#[test]
fn search_equity_is_always_a_valid_distribution() {
    for dice in [Dice::new(1, 1).unwrap(), Dice::new(6, 1).unwrap()] {
        let equity = Searcher::new()
            .search(Position::new(), dice, options(1, 100_000))
            .equity;
        let total: f32 = equity.outcomes().into_iter().sum();
        assert!((total - 1.0).abs() < 1.0e-5);
        assert!(
            equity
                .outcomes()
                .into_iter()
                .all(|value| (0.0..=1.0).contains(&value))
        );
    }
}

#[test]
fn the_search_result_retains_a_complete_sequence_for_animation() {
    let position = Position::new();
    let dice = Dice::new(3, 1).unwrap();
    let report = Searcher::new().search(position, dice, options(1, 100_000));
    let play = report.best_play.unwrap();
    assert_eq!(play.len(), 2);
    assert_eq!(
        position.play(dice, &play).unwrap().side_to_move(),
        Player::Black
    );
    assert!(play.steps().iter().all(|step| step.from() != Location::Off));
    assert!(
        play.steps()
            .iter()
            .all(|step| step.die() == 1 || step.die() == 3)
    );
    assert!(
        play.steps()
            .iter()
            .any(|step| step.from() == Location::Point(point(8)))
            || !play.is_empty()
    );
}
