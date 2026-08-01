use ai_othello::{EvaluationProfile, Move, Position, Side, evaluate};

fn position(moves: &[&str]) -> Position {
    let moves: Vec<Move> = moves.iter().map(|mv| mv.parse().unwrap()).collect();
    Position::from_moves(&moves).unwrap()
}

#[test]
fn every_breakdown_sums_exactly_and_the_opening_is_symmetric() {
    for profile in EvaluationProfile::ALL {
        let evaluation = evaluate(Position::start(), profile);
        assert_eq!(evaluation.total, evaluation.terms_sum());
        assert_eq!(evaluation.total, 0);
        assert_eq!(evaluation.phase, 0);
    }
}

#[test]
fn every_profile_is_color_relative_and_mirror_invariant() {
    let position = position(&["d3", "c3", "c4", "c5", "b5", "d2"]);
    let color_swapped = Position::from_bits(
        position.bits(Side::White),
        position.bits(Side::Black),
        position.side_to_move().other(),
    )
    .unwrap();
    for profile in EvaluationProfile::ALL {
        let original = evaluate(position, profile);
        let mirrored = evaluate(position.mirrored(), profile);
        let swapped = evaluate(color_swapped, profile);
        assert_eq!(original, mirrored);
        assert_eq!(original, swapped);
        assert_eq!(original.total, original.terms_sum());
    }
}

#[test]
fn phase_taper_moves_weight_from_mobility_toward_material() {
    let opening = evaluate(Position::start(), EvaluationProfile::Phase);
    let almost_full = Position::from_bits(u64::MAX ^ (1u64 << 63), 0, Side::White).unwrap();
    let endgame = evaluate(almost_full, EvaluationProfile::Phase);
    assert!(opening.weights.mobility > endgame.weights.mobility);
    assert!(opening.weights.material < endgame.weights.material);
    assert_eq!(endgame.phase, 59);
}
