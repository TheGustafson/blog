use ai_backgammon::{
    DICE_OUTCOMES, Dice, GameKind, Location, Play, Player, Point, Position, PositionError, Step,
};

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
        assert_eq!(points[usize::from(number - 1)], 0);
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

#[test]
fn initial_position_has_the_standard_setup() {
    let position = Position::new();
    assert_eq!(position.count(Player::White, point(24)), 2);
    assert_eq!(position.count(Player::White, point(13)), 5);
    assert_eq!(position.count(Player::White, point(8)), 3);
    assert_eq!(position.count(Player::White, point(6)), 5);
    assert_eq!(position.count(Player::Black, point(1)), 2);
    assert_eq!(position.count(Player::Black, point(12)), 5);
    assert_eq!(position.count(Player::Black, point(17)), 3);
    assert_eq!(position.count(Player::Black, point(19)), 5);
    position.validate().unwrap();
}

#[test]
fn standard_opening_moves_are_generated() {
    let openings = [
        (Dice::new(3, 1).unwrap(), [(8, 5, 3), (6, 5, 1)]),
        (Dice::new(4, 2).unwrap(), [(8, 4, 4), (6, 4, 2)]),
        (Dice::new(5, 3).unwrap(), [(8, 3, 5), (6, 3, 3)]),
        (Dice::new(6, 1).unwrap(), [(13, 7, 6), (8, 7, 1)]),
        (Dice::new(6, 5).unwrap(), [(24, 18, 6), (18, 13, 5)]),
    ];

    for (dice, steps) in openings {
        let play = Play::new(
            steps
                .into_iter()
                .map(|(from, to, die)| {
                    Step::new(
                        Location::Point(point(from)),
                        Location::Point(point(to)),
                        die,
                    )
                    .unwrap()
                })
                .collect(),
        );
        assert!(
            Position::new().legal_plays(dice).contains(&play),
            "missing standard opening {dice}: {play}"
        );
    }
}

#[test]
fn malformed_positions_are_rejected() {
    let mut overflow = [0_i8; 24];
    overflow[0] = 16;
    assert_eq!(
        Position::from_parts(overflow, [0, 0], [0, 15], Player::White),
        Err(PositionError::PointOverflow)
    );
    assert_eq!(
        Position::from_parts([0; 24], [0, 0], [15, 14], Player::White),
        Err(PositionError::CheckerCount {
            player: Player::Black,
            found: 14,
        })
    );
    assert_eq!(
        Position::from_parts([0; 24], [0, 0], [15, 15], Player::White),
        Err(PositionError::TwoWinners)
    );
}

#[test]
fn dice_outcomes_represent_all_thirty_six_rolls() {
    assert_eq!(DICE_OUTCOMES.len(), 21);
    assert_eq!(
        DICE_OUTCOMES
            .iter()
            .map(|outcome| outcome.weight)
            .sum::<u8>(),
        36
    );
    assert!(
        DICE_OUTCOMES
            .iter()
            .filter(|outcome| outcome.dice.is_double())
            .all(|outcome| outcome.weight == 1)
    );
    assert!(
        DICE_OUTCOMES
            .iter()
            .filter(|outcome| !outcome.dice.is_double())
            .all(|outcome| outcome.weight == 2)
    );
}

#[test]
fn bar_entry_is_mandatory_and_can_hit() {
    let position = position(&[], &[(24, 1), (23, 2)], [1, 0], Player::White);
    let plays = position.legal_plays(Dice::new(2, 1).unwrap());
    assert!(!plays.is_empty());
    assert!(plays.iter().all(|play| {
        play.steps()[0].from() == Location::Bar
            && play.steps()[0].to() == Location::Point(point(24))
    }));

    let next = position.play(Dice::new(2, 1).unwrap(), &plays[0]).unwrap();
    assert_eq!(next.bar(Player::Black), 1);
    assert_eq!(next.count(Player::White, point(22)), 1);
}

#[test]
fn black_enters_from_the_other_side_of_the_board() {
    let position = position(&[(1, 1), (2, 2)], &[], [0, 1], Player::Black);
    let plays = position.legal_plays(Dice::new(2, 1).unwrap());
    assert!(plays.iter().all(|play| {
        play.steps()[0].from() == Location::Bar && play.steps()[0].to() == Location::Point(point(1))
    }));
    let next = position.play(Dice::new(2, 1).unwrap(), &plays[0]).unwrap();
    assert_eq!(next.bar(Player::White), 1);
    assert_eq!(next.count(Player::Black, point(3)), 1);
}

#[test]
fn as_many_bar_checkers_as_possible_must_enter() {
    let position = position(&[], &[(23, 2)], [2, 0], Player::White);
    let plays = position.legal_plays(Dice::new(2, 1).unwrap());
    assert!(plays.iter().all(|play| play.len() == 1));
    assert!(plays.iter().all(|play| play.steps()[0].die() == 1));
    let next = position.play(Dice::new(2, 1).unwrap(), &plays[0]).unwrap();
    assert_eq!(next.bar(Player::White), 1);
}

#[test]
fn a_fully_blocked_bar_entry_passes() {
    let position = position(&[], &[(24, 2), (23, 2)], [1, 0], Player::White);
    let plays = position.legal_plays(Dice::new(2, 1).unwrap());
    assert_eq!(plays.len(), 1);
    assert!(plays[0].is_empty());
}

#[test]
fn the_generator_uses_the_maximum_number_of_dice() {
    let position = position(&[(6, 1)], &[(4, 2)], [0, 0], Player::White);
    let plays = position.legal_plays(Dice::new(2, 1).unwrap());
    assert!(plays.iter().all(|play| play.len() == 2));
    assert!(
        plays
            .iter()
            .all(|play| { play.steps()[0].die() == 1 && play.steps()[1].die() == 2 })
    );
}

#[test]
fn a_checker_cannot_jump_over_blocked_intermediate_points() {
    let position = position(&[(8, 1)], &[(5, 2), (3, 2)], [0, 0], Player::White);
    let plays = position.legal_plays(Dice::new(5, 3).unwrap());
    assert_eq!(plays.len(), 1);
    assert!(plays[0].is_empty());
}

#[test]
fn the_higher_die_is_forced_when_only_one_can_be_used() {
    let position = position(&[(1, 1)], &[(24, 1)], [0, 0], Player::White);
    let plays = position.legal_plays(Dice::new(6, 5).unwrap());
    assert!(plays.iter().all(|play| play.len() == 1));
    assert!(plays.iter().all(|play| play.steps()[0].die() == 6));
}

#[test]
fn doubles_provide_four_moves_when_all_are_legal() {
    let position = position(&[(4, 1)], &[(24, 1)], [0, 0], Player::White);
    let plays = position.legal_plays(Dice::new(1, 1).unwrap());
    assert_eq!(plays.len(), 1);
    assert_eq!(plays[0].len(), 4);
    assert_eq!(plays[0].steps().last().unwrap().to(), Location::Off);
}

#[test]
fn bearing_off_requires_every_active_checker_in_the_home_board() {
    let position = position(&[(7, 1)], &[(24, 1)], [0, 0], Player::White);
    assert!(
        position
            .legal_steps(6)
            .iter()
            .all(|step| step.to() != Location::Off)
    );
}

#[test]
fn oversized_bear_off_uses_only_the_farthest_checker() {
    let blocked = position(&[(6, 1), (4, 1)], &[(24, 1)], [0, 0], Player::White);
    assert!(
        blocked.legal_steps(5).iter().all(|step| {
            !(step.from() == Location::Point(point(4)) && step.to() == Location::Off)
        })
    );

    let clear = position(&[(4, 1)], &[(24, 1)], [0, 0], Player::White);
    assert!(
        clear
            .legal_steps(5)
            .iter()
            .any(|step| { step.from() == Location::Point(point(4)) && step.to() == Location::Off })
    );
}

#[test]
fn black_bearing_off_mirrors_white_bearing_off() {
    let blocked = position(&[(1, 1)], &[(19, 1), (21, 1)], [0, 0], Player::Black);
    assert!(blocked.legal_steps(5).iter().all(|step| {
        !(step.from() == Location::Point(point(21)) && step.to() == Location::Off)
    }));

    let clear = position(&[(1, 1)], &[(21, 1)], [0, 0], Player::Black);
    assert!(
        clear.legal_steps(5).iter().any(|step| {
            step.from() == Location::Point(point(21)) && step.to() == Location::Off
        })
    );
}

#[test]
fn game_outcomes_distinguish_single_gammon_and_backgammon() {
    let single = position(&[], &[(19, 14)], [0, 0], Player::Black);
    assert_eq!(single.game_outcome().unwrap().kind, GameKind::Single);

    let gammon = position(&[], &[(19, 15)], [0, 0], Player::Black);
    assert_eq!(gammon.game_outcome().unwrap().kind, GameKind::Gammon);

    let backgammon = position(&[], &[(3, 1), (19, 14)], [0, 0], Player::Black);
    assert_eq!(
        backgammon.game_outcome().unwrap().kind,
        GameKind::Backgammon
    );

    let bar_backgammon = position(&[], &[(19, 14)], [0, 1], Player::Black);
    assert_eq!(
        bar_backgammon.game_outcome().unwrap().kind,
        GameKind::Backgammon
    );
}
