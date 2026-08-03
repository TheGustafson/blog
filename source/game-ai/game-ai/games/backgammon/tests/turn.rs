use ai_backgammon::{
    DICE_OUTCOMES, Dice, Location, Player, Point, Position, Step, Turn, TurnError,
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
fn every_complete_play_can_be_entered_step_by_step() {
    let mut seed = 0x749d_e821_0bb6_25a7_u64;
    let mut position = Position::new();

    for _ in 0..30 {
        for outcome in DICE_OUTCOMES {
            for play in position.legal_plays(outcome.dice) {
                let mut turn = Turn::new(position, outcome.dice).unwrap();
                for &step in play.steps() {
                    assert!(turn.legal_steps().contains(&step));
                    turn.select(step).unwrap();
                }
                assert!(turn.is_complete());
                assert_eq!(turn.completed_play(), Some(&play));
                assert_eq!(turn.finish().unwrap(), play);
            }
        }
        if position.game_outcome().is_some() {
            break;
        }
        seed = seed
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        let dice = DICE_OUTCOMES[(seed as usize) % DICE_OUTCOMES.len()].dice;
        let plays = position.legal_plays(dice);
        position = position
            .play(dice, &plays[((seed >> 32) as usize) % plays.len()])
            .unwrap();
    }
}

#[test]
fn legal_continuations_always_belong_to_a_complete_play() {
    let dice = Dice::new(3, 1).unwrap();
    let position = Position::new();
    let plays = position.legal_plays(dice);
    let mut turn = Turn::new(position, dice).unwrap();

    while !turn.is_complete() {
        let legal = turn.legal_steps();
        assert!(!legal.is_empty());
        for step in &legal {
            let mut prefix = turn.steps().to_vec();
            prefix.push(*step);
            assert!(plays.iter().any(|play| play.steps().starts_with(&prefix)));
        }
        turn.select(legal[0]).unwrap();
    }
}

#[test]
fn preview_and_undo_restore_partial_turns_exactly() {
    let position = Position::new();
    let mut turn = Turn::new(position, Dice::new(3, 1).unwrap()).unwrap();
    let first = turn.legal_steps()[0];
    turn.select(first).unwrap();

    assert_ne!(turn.preview_position(), position);
    assert_eq!(turn.preview_position().side_to_move(), Player::White);
    assert_eq!(turn.remaining_dice().len(), 1);
    assert!(turn.undo());
    assert_eq!(turn.preview_position(), position);
    assert_eq!(turn.remaining_dice().len(), 2);
    assert!(!turn.undo());
}

#[test]
fn a_blocked_turn_is_immediately_complete_as_a_pass() {
    let position = position(&[], &[(24, 2), (23, 2)], [1, 0], Player::White);
    let turn = Turn::new(position, Dice::new(2, 1).unwrap()).unwrap();

    assert!(turn.is_pass());
    assert!(turn.is_complete());
    assert!(turn.legal_steps().is_empty());
    assert!(turn.finish().unwrap().is_empty());
}

#[test]
fn identical_bear_off_destinations_keep_the_die_choice() {
    let position = position(&[(3, 1), (1, 1)], &[(24, 1)], [0, 0], Player::White);
    let turn = Turn::new(position, Dice::new(4, 3).unwrap()).unwrap();
    let choices = turn
        .legal_steps()
        .into_iter()
        .filter(|step| step.from() == Location::Point(point(3)) && step.to() == Location::Off)
        .collect::<Vec<_>>();

    assert_eq!(choices.len(), 2);
    assert_eq!(choices[0].die(), 3);
    assert_eq!(choices[1].die(), 4);
}

#[test]
fn illegal_and_late_steps_are_rejected_without_mutation() {
    let position = Position::new();
    let mut turn = Turn::new(position, Dice::new(3, 1).unwrap()).unwrap();
    let illegal = Step::new(Location::Point(point(24)), Location::Point(point(22)), 1).unwrap();

    assert_eq!(turn.select(illegal), Err(TurnError::IllegalStep));
    assert_eq!(turn.preview_position(), position);
    while !turn.is_complete() {
        let step = turn.legal_steps()[0];
        turn.select(step).unwrap();
    }
    assert_eq!(turn.select(illegal), Err(TurnError::IllegalStep));
}

#[test]
fn finished_positions_cannot_start_a_turn() {
    let mut points = [0_i8; 24];
    points[18] = -15;
    let position = Position::from_parts(points, [0, 0], [15, 0], Player::Black).unwrap();
    assert_eq!(
        Turn::new(position, Dice::new(6, 1).unwrap()),
        Err(TurnError::GameOver)
    );
}
