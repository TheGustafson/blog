use ai_backgammon::{Equity, GameKind, Player, Point, Position, evaluate_position, pip_count};

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

fn mirror(position: Position) -> Position {
    let mut points = [0_i8; 24];
    for (index, count) in position.points().into_iter().enumerate() {
        points[23 - index] = -count;
    }
    Position::from_parts(
        points,
        [position.bar(Player::Black), position.bar(Player::White)],
        [position.off(Player::Black), position.off(Player::White)],
        position.side_to_move().other(),
    )
    .unwrap()
}

fn assert_close(left: f32, right: f32) {
    assert!((left - right).abs() < 1.0e-5, "{left} != {right}");
}

fn assert_equity_close(left: Equity, right: Equity) {
    for (left, right) in left.outcomes().into_iter().zip(right.outcomes()) {
        assert_close(left, right);
    }
}

#[test]
fn initial_position_is_evaluated_as_balanced() {
    let equity = evaluate_position(Position::new());
    assert_close(equity.win_probability(), 0.5);
    assert_close(equity.expected_points(), 0.0);
}

#[test]
fn pip_counts_follow_each_players_direction() {
    let opening = Position::new();
    assert_eq!(pip_count(opening, Player::White), 167);
    assert_eq!(pip_count(opening, Player::Black), 167);

    let position = position(&[(1, 1)], &[(24, 1)], [0, 0], Player::White);
    assert_eq!(pip_count(position, Player::White), 1);
    assert_eq!(pip_count(position, Player::Black), 1);
}

#[test]
fn checkers_on_the_bar_count_as_twenty_five_pips() {
    let position = position(&[], &[(24, 1)], [1, 0], Player::White);
    assert_eq!(pip_count(position, Player::White), 25);
}

#[test]
fn a_large_racing_lead_is_good_for_the_side_on_roll() {
    let ahead = position(&[(1, 5)], &[(19, 15)], [0, 0], Player::White);
    let behind = Position::from_parts(
        ahead.points(),
        [0, 0],
        [ahead.off(Player::White), ahead.off(Player::Black)],
        Player::Black,
    )
    .unwrap();

    assert!(evaluate_position(ahead).expected_points() > 0.8);
    assert!(evaluate_position(behind).expected_points() < -0.8);
}

#[test]
fn putting_a_checker_on_the_bar_improves_contact_equity() {
    let no_hit = position(
        &[(13, 13), (8, 2)],
        &[(12, 13), (17, 2)],
        [0, 0],
        Player::White,
    );
    let hit = position(
        &[(13, 13), (8, 2)],
        &[(12, 13), (17, 1)],
        [0, 1],
        Player::White,
    );

    assert!(evaluate_position(hit).expected_points() > evaluate_position(no_hit).expected_points());
}

#[test]
fn every_nonterminal_evaluation_is_a_probability_distribution() {
    let positions = [
        Position::new(),
        position(&[(1, 5)], &[(19, 15)], [0, 0], Player::White),
        position(
            &[(13, 13), (8, 2)],
            &[(12, 13), (17, 1)],
            [0, 1],
            Player::Black,
        ),
    ];
    for position in positions {
        let equity = evaluate_position(position);
        assert_close(equity.outcomes().into_iter().sum(), 1.0);
        assert!(
            equity
                .outcomes()
                .into_iter()
                .all(|value| (0.0..=1.0).contains(&value))
        );
        assert!((-3.0..=3.0).contains(&equity.expected_points()));
    }
}

#[test]
fn color_and_direction_mirroring_preserves_equity() {
    let positions = [
        Position::new(),
        position(
            &[(20, 2), (8, 2)],
            &[(5, 1), (17, 2)],
            [1, 2],
            Player::White,
        ),
        position(&[(1, 5)], &[(19, 15)], [0, 0], Player::Black),
    ];
    for position in positions {
        assert_equity_close(
            evaluate_position(position),
            evaluate_position(mirror(position)),
        );
    }
}

#[test]
fn changing_only_the_perspective_reverses_equity() {
    let white = position(
        &[(20, 2), (8, 2)],
        &[(5, 1), (17, 2)],
        [1, 2],
        Player::White,
    );
    let black = Position::from_parts(
        white.points(),
        [white.bar(Player::White), white.bar(Player::Black)],
        [white.off(Player::White), white.off(Player::Black)],
        Player::Black,
    )
    .unwrap();
    assert_equity_close(
        evaluate_position(white).reversed(),
        evaluate_position(black),
    );
}

#[test]
fn terminal_single_is_exact() {
    let position = position(&[], &[(19, 14)], [0, 0], Player::Black);
    let equity = evaluate_position(position);
    assert_eq!(position.game_outcome().unwrap().kind, GameKind::Single);
    assert_eq!(equity.outcomes(), [0.0, 0.0, 0.0, 1.0, 0.0, 0.0]);
    assert_eq!(equity.expected_points(), -1.0);
}

#[test]
fn terminal_gammon_is_exact() {
    let position = position(&[], &[(19, 15)], [0, 0], Player::Black);
    let equity = evaluate_position(position);
    assert_eq!(position.game_outcome().unwrap().kind, GameKind::Gammon);
    assert_eq!(equity.outcomes(), [0.0, 0.0, 0.0, 0.0, 1.0, 0.0]);
    assert_eq!(equity.expected_points(), -2.0);
}

#[test]
fn terminal_backgammon_is_exact() {
    let position = position(&[], &[(19, 14)], [0, 1], Player::Black);
    let equity = evaluate_position(position);
    assert_eq!(position.game_outcome().unwrap().kind, GameKind::Backgammon);
    assert_eq!(equity.outcomes(), [0.0, 0.0, 0.0, 0.0, 0.0, 1.0]);
    assert_eq!(equity.expected_points(), -3.0);
}

#[test]
fn a_single_trapped_checker_creates_backgammon_equity() {
    let position = position(&[(3, 10)], &[(1, 1), (19, 14)], [0, 0], Player::White);
    let equity = evaluate_position(position);
    assert_eq!(position.off(Player::Black), 0);
    assert!(equity.outcomes()[2] > 0.0);
}

#[test]
fn reversing_terminal_equity_swaps_all_six_outcomes() {
    let win = Equity::win(GameKind::Backgammon);
    assert_eq!(win.reversed(), Equity::loss(GameKind::Backgammon));
    assert_eq!(win.reversed().reversed(), win);
}

#[test]
fn point_access_used_by_the_evaluator_remains_public_and_exact() {
    assert_eq!(Position::new().count(Player::White, point(24)), 2);
}
