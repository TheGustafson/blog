use ai_othello::verification::{ReferencePosition, reference_perft};
use ai_othello::{GameResult, Move, MoveError, Position, Side, Square, perft};

#[test]
fn initial_position_and_notation_are_stable() {
    let position = Position::start();
    assert_eq!(position.disc_count(Side::Black), 2);
    assert_eq!(position.disc_count(Side::White), 2);
    assert_eq!(position.side_to_move(), Side::Black);
    let legal: Vec<String> = position
        .legal_moves()
        .into_iter()
        .map(|mv| mv.to_string())
        .collect();
    assert_eq!(legal, ["d3", "c4", "f5", "e6"]);
    assert_eq!("pass".parse(), Ok(Move::Pass));
    assert_eq!("h8".parse(), Ok(Move::Place(Square::new(63))));
}

#[test]
fn opening_move_flips_exactly_one_disc_and_unmakes() {
    let mut position = Position::start();
    let start = position;
    let mv: Move = "d3".parse().unwrap();
    let flips = position.flips_for(mv.square().unwrap());
    assert_eq!(flips, 1u64 << "d4".parse::<Square>().unwrap().index());
    let undo = position.make_move(mv).unwrap();
    assert_eq!(undo.flipped(), flips);
    assert_eq!(position.side_at("d4".parse().unwrap()), Some(Side::Black));
    position.unmake_move(undo);
    assert_eq!(position, start);
}

#[test]
fn pass_is_a_real_transition_but_only_when_forced() {
    let black = 1u64 << "b1".parse::<Square>().unwrap().index();
    let white = 1u64 << "a1".parse::<Square>().unwrap().index();
    let mut position = Position::from_bits(black, white, Side::Black).unwrap();
    assert_eq!(position.legal_moves().as_slice(), [Move::Pass]);
    let before = position;
    let undo = position.make_move(Move::Pass).unwrap();
    assert_eq!(position.side_to_move(), Side::White);
    assert_eq!(
        position.legal_moves().as_slice(),
        [Move::Place("c1".parse().unwrap())]
    );
    position.unmake_move(undo);
    assert_eq!(position, before);

    assert_eq!(
        Position::start().make_move(Move::Pass),
        Err(MoveError::PassNotAllowed)
    );
}

#[test]
fn neither_side_having_a_move_ends_the_game_without_two_synthetic_passes() {
    let black = 1u64 << "a1".parse::<Square>().unwrap().index();
    let white = 1u64 << "h8".parse::<Square>().unwrap().index();
    let position = Position::from_bits(black, white, Side::Black).unwrap();
    assert_eq!(position.legal_moves().len(), 0);
    assert_eq!(position.result(), GameResult::Draw { black: 1, white: 1 });
}

#[test]
fn edge_masks_prevent_rays_from_wrapping() {
    let black = 1u64 << "a1".parse::<Square>().unwrap().index();
    let white = 1u64 << "h1".parse::<Square>().unwrap().index();
    let position = Position::from_bits(black, white, Side::Black).unwrap();
    assert_eq!(position.legal_placement_bits(), 0);
}

#[test]
fn mirror_is_an_involution_and_preserves_rules() {
    let moves: Vec<Move> = ["d3", "c3", "c4", "c5", "b5"]
        .into_iter()
        .map(|mv| mv.parse().unwrap())
        .collect();
    let position = Position::from_moves(&moves).unwrap();
    let mirror = position.mirrored();
    assert_eq!(mirror.mirrored(), position);
    assert_eq!(mirror.result(), position.result());
    let mirrored_moves: Vec<_> = position
        .legal_moves()
        .into_iter()
        .map(Move::mirrored)
        .collect();
    let mut actual: Vec<_> = mirror.legal_moves().into_iter().collect();
    actual.sort_unstable();
    let mut expected = mirrored_moves;
    expected.sort_unstable();
    assert_eq!(actual, expected);
}

#[test]
fn bitboards_match_the_ray_walking_reference_tree() {
    fn compare(fast: &mut Position, reference: &mut ReferencePosition, depth: u8) {
        assert_eq!(fast.side_to_move(), reference.side_to_move());
        assert_eq!(fast.result(), reference.result());
        assert_eq!(
            fast.legal_moves().as_slice(),
            reference.legal_moves().as_slice()
        );
        for square in Square::all() {
            assert_eq!(fast.side_at(square), reference.side_at(square));
        }
        if depth == 0 {
            return;
        }
        let fast_before = *fast;
        let reference_before = *reference;
        for mv in fast.legal_moves() {
            let fast_undo = fast.make_move(mv).unwrap();
            let reference_undo = reference.make_move(mv).unwrap();
            compare(fast, reference, depth - 1);
            fast.unmake_move(fast_undo);
            reference.unmake_move(reference_undo);
            assert_eq!(*fast, fast_before);
            assert_eq!(*reference, reference_before);
        }
    }

    let mut fast = Position::start();
    let mut reference = ReferencePosition::start();
    compare(&mut fast, &mut reference, 5);
    assert_eq!(perft(&mut fast, 5), reference_perft(&mut reference, 5));
}

#[test]
fn standard_opening_perft_is_locked() {
    let expected = [1u64, 4, 12, 56, 244, 1_396, 8_200, 55_092, 390_216];
    let mut position = Position::start();
    for (depth, nodes) in expected.into_iter().enumerate() {
        assert_eq!(perft(&mut position, depth as u8), nodes, "depth {depth}");
        assert_eq!(position, Position::start());
    }
}

#[test]
fn deterministic_complete_game_round_trips_through_explicit_passes() {
    let mut position = Position::start();
    let mut reference = ReferencePosition::start();
    let mut history = Vec::new();
    let mut passes = 0;

    while position.result() == GameResult::Ongoing {
        let legal = position.legal_moves();
        assert_eq!(position.result(), reference.result());
        assert_eq!(legal.as_slice(), reference.legal_moves().as_slice());
        let mv = legal.as_slice()[0];
        passes += usize::from(mv == Move::Pass);
        history.push(mv);
        position.make_move(mv).unwrap();
        reference.make_move(mv).unwrap();
        assert_eq!(position.side_to_move(), reference.side_to_move());
        for square in Square::all() {
            assert_eq!(position.side_at(square), reference.side_at(square));
        }
    }

    let encoded = history
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(".");
    assert_eq!(passes, 4);
    assert_eq!(history.len(), 64);
    assert_eq!(
        history.len(),
        usize::from(position.occupied_count() - 4) + passes
    );
    assert!(position.legal_moves().is_empty());
    assert_eq!(position.result(), reference.result());
    assert_eq!(Position::from_moves(&history), Ok(position));
    assert_eq!(
        position.result(),
        GameResult::Win {
            winner: Side::White,
            black: 19,
            white: 45,
        }
    );
    assert_eq!(
        encoded,
        "d3.c3.b3.b2.b1.a1.c4.c1.c2.d2.d1.e1.a2.a3.f5.e2.f1.g1.pass.f2.pass.e3.pass.b5.b4.a5.a4.c5.a6.f4.f3.g3.g2.h2.h1.h3.h4.g4.c6.g5.h5.b6.c7.d6.e6.f6.g6.h6.h7.a7.pass.b7.a8.d7.e7.f7.g7.g8.b8.c8.d8.e8.f8.h8"
    );
}
