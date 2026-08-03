use ai_backgammon::{
    DICE_OUTCOMES, Dice, Location, Play, PlayError, Player, Point, Position, Step,
    verification::reference_legal_plays,
};

fn mirror_position(position: Position) -> Position {
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

fn mirror_location(location: Location) -> Location {
    match location {
        Location::Bar => Location::Bar,
        Location::Point(point) => Location::Point(Point::new(25 - point.number()).unwrap()),
        Location::Off => Location::Off,
    }
}

fn mirror_play(play: &Play) -> Play {
    Play::new(
        play.steps()
            .iter()
            .map(|step| {
                Step::new(
                    mirror_location(step.from()),
                    mirror_location(step.to()),
                    step.die(),
                )
                .unwrap()
            })
            .collect(),
    )
}

#[test]
fn make_and_unmake_restore_every_bit_of_state() {
    let mut position = Position::new();
    let original = position;
    let original_hash = position.hash();
    let dice = Dice::new(3, 1).unwrap();
    let play = position.legal_plays(dice).remove(0);
    let undo = position.make_play(dice, &play).unwrap();
    assert_ne!(position, original);
    position.unmake(undo);
    assert_eq!(position, original);
    assert_eq!(position.hash(), original_hash);
}

#[test]
fn equivalent_step_orders_are_grouped_for_search() {
    let position = Position::new();
    let dice = Dice::new(3, 1).unwrap();
    let plays = position.legal_plays(dice);
    let outcomes = position.legal_outcomes(dice);
    assert!(outcomes.len() < plays.len());
    assert_eq!(
        outcomes
            .iter()
            .map(|outcome| outcome.sequences().len())
            .sum::<usize>(),
        plays.len()
    );
}

#[test]
fn optimized_and_reference_generators_agree_on_reachable_positions() {
    for trajectory in 0..6_u64 {
        let mut seed = 0x7a5f_31d2_9864_c0ab_u64 ^ trajectory.wrapping_mul(0x9e37_79b9_7f4a_7c15);
        let mut position = Position::new();

        for _ in 0..32 {
            for outcome in DICE_OUTCOMES {
                assert_eq!(
                    position.legal_plays(outcome.dice),
                    reference_legal_plays(position, outcome.dice),
                    "generator mismatch at hash {:016x} for {}",
                    position.hash(),
                    outcome.dice,
                );
            }
            position.validate().unwrap();
            if position.game_outcome().is_some() {
                break;
            }
            seed = seed
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            let dice = DICE_OUTCOMES[(seed as usize) % DICE_OUTCOMES.len()].dice;
            let plays = position.legal_plays(dice);
            seed ^= seed >> 27;
            let play = &plays[(seed as usize) % plays.len()];
            position = position.play(dice, play).unwrap();
        }
    }
}

#[test]
fn colors_and_directions_are_exact_mirrors() {
    let mut seed = 0x38d7_4a91_bfc2_650e_u64;
    let mut position = Position::new();

    for _ in 0..36 {
        let mirrored = mirror_position(position);
        for outcome in DICE_OUTCOMES {
            let mut expected: Vec<Play> = position
                .legal_plays(outcome.dice)
                .iter()
                .map(mirror_play)
                .collect();
            expected.sort();
            assert_eq!(
                mirrored.legal_plays(outcome.dice),
                expected,
                "mirror mismatch at hash {:016x} for {}",
                position.hash(),
                outcome.dice,
            );
        }
        if position.game_outcome().is_some() {
            break;
        }
        seed = seed
            .wrapping_mul(2_862_933_555_777_941_757)
            .wrapping_add(3_037_000_493);
        let dice = DICE_OUTCOMES[(seed as usize) % DICE_OUTCOMES.len()].dice;
        let plays = position.legal_plays(dice);
        let play = &plays[((seed >> 32) as usize) % plays.len()];
        position = position.play(dice, play).unwrap();
    }
}

#[test]
fn illegal_plays_do_not_mutate_the_position() {
    let mut position = Position::new();
    let original = position;
    let result = position.make_play(Dice::new(3, 1).unwrap(), &Play::pass());
    assert_eq!(result, Err(PlayError::IllegalPlay));
    assert_eq!(position, original);
}

#[test]
fn a_finished_position_rejects_further_play() {
    let mut points = [0_i8; 24];
    points[18] = -15;
    let mut position = Position::from_parts(points, [0, 0], [15, 0], Player::Black).unwrap();
    let dice = Dice::new(6, 1).unwrap();
    assert!(position.legal_plays(dice).is_empty());
    assert_eq!(
        position.make_play(dice, &Play::pass()),
        Err(PlayError::GameOver)
    );
}

#[test]
fn every_generated_play_preserves_position_invariants() {
    let position = Position::new();
    for outcome in DICE_OUTCOMES {
        for play in position.legal_plays(outcome.dice) {
            position
                .play(outcome.dice, &play)
                .unwrap()
                .validate()
                .unwrap();
        }
    }
}
