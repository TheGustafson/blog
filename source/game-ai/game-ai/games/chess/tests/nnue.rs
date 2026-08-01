use ai_chess::{
    Color, EvaluationProfile, FeatureDelta, FloatNnueNetwork, NNUE_FEATURES, NNUE_HIDDEN,
    NnueAccumulator, Piece, PieceKind, Position, QuantizedNnueNetwork, Square, builtin_nnue,
    evaluate, nnue_feature_index,
};

fn fixture() -> FloatNnueNetwork {
    let feature_bias = (0..NNUE_HIDDEN)
        .map(|lane| 0.22 + (lane % 9) as f32 * 0.008)
        .collect();
    let feature_weights = (0..NNUE_FEATURES * NNUE_HIDDEN)
        .map(|index| {
            let centered = ((index * 17 + index / NNUE_HIDDEN * 5) % 23) as i32 - 11;
            centered as f32 * 0.0015
        })
        .collect();
    let output_weights = (0..NNUE_HIDDEN * 2)
        .map(|index| {
            let centered = ((index * 7 + 3) % 17) as i32 - 8;
            centered as f32 * 0.65
        })
        .collect();
    FloatNnueNetwork::new(feature_bias, feature_weights, output_weights, 7.25).unwrap()
}

#[test]
fn feature_index_is_color_relative_and_stays_in_bounds() {
    let e4: Square = "e4".parse().unwrap();
    let e5: Square = "e5".parse().unwrap();
    let white_pawn = Piece::new(Color::White, PieceKind::Pawn);
    let black_pawn = Piece::new(Color::Black, PieceKind::Pawn);
    assert_eq!(
        nnue_feature_index(white_pawn, e4, Color::White),
        nnue_feature_index(black_pawn, e5, Color::Black)
    );
    assert_eq!(
        nnue_feature_index(black_pawn, e5, Color::White),
        nnue_feature_index(white_pawn, e4, Color::Black)
    );
    for color in Color::ALL {
        for kind in PieceKind::ALL {
            for square in Square::all() {
                assert!(
                    nnue_feature_index(Piece::new(color, kind), square, Color::White)
                        < NNUE_FEATURES
                );
                assert!(
                    nnue_feature_index(Piece::new(color, kind), square, Color::Black)
                        < NNUE_FEATURES
                );
            }
        }
    }
}

#[test]
fn quantized_scalar_stays_close_to_the_float_reference() {
    let float = fixture();
    let quantized = float.quantize();
    let positions = [
        Position::start(),
        Position::from_fen("r1bqkbnr/pppp1ppp/2n5/4p3/4P3/5N2/PPPP1PPP/RNBQKB1R w KQkq - 2 3")
            .unwrap(),
        Position::from_fen("4k3/8/4p3/3r4/8/8/8/3QK3 w - - 0 1").unwrap(),
        Position::from_fen("8/5pk1/6p1/7p/4P3/5KP1/5P2/8 w - - 0 43").unwrap(),
    ];
    for position in positions {
        let reference = float.evaluate(&position);
        let integer = quantized.evaluate_refresh(&position);
        assert!(
            (reference - integer as f32).abs() <= 1.5,
            "float={reference:.3}, integer={integer}, fen={}",
            position.fen()
        );
    }
}

#[test]
fn published_wrong_bishop_disagreement_is_locked_to_the_embedded_artifact() {
    assert_eq!(
        evaluate(&Position::start(), EvaluationProfile::TinyNnue).total,
        31
    );
    let position = Position::from_fen("k7/P3K3/8/8/3B4/8/8/8 w - - 67 130").unwrap();
    assert_eq!(evaluate(&position, EvaluationProfile::Material).total, 393);
    assert_eq!(
        evaluate(&position, EvaluationProfile::PieceSquare).total,
        616
    );
    assert_eq!(evaluate(&position, EvaluationProfile::TinyNnue).total, -33);
    assert_eq!(builtin_nnue().checksum(), 0xf315_9300_9090_9a44);
    assert_eq!(builtin_nnue().to_bytes().len(), 197_412);
}

#[test]
fn versioned_network_round_trips_and_enforces_every_header_gate() {
    let network = fixture().quantize();
    let bytes = network.to_bytes();
    assert_eq!(QuantizedNnueNetwork::from_bytes(&bytes).unwrap(), network);
    assert_ne!(network.checksum(), 0);

    let reject = |damaged: &[u8], expected: &str| {
        assert_eq!(
            QuantizedNnueNetwork::from_bytes(damaged)
                .expect_err("damaged network must be rejected")
                .to_string(),
            expected
        );
    };

    reject(&bytes[..31], "network file is truncated");

    let mut wrong_magic = bytes.clone();
    wrong_magic[0] ^= 1;
    reject(&wrong_magic, "network magic does not match GAINNUE");

    let mut wrong_version = bytes.clone();
    wrong_version[8..10].copy_from_slice(&2u16.to_le_bytes());
    reject(&wrong_version, "unsupported network version 2");

    let mut wrong_dimensions = bytes.clone();
    wrong_dimensions[10..12].copy_from_slice(&0u16.to_le_bytes());
    reject(
        &wrong_dimensions,
        "network dimensions or payload length do not match",
    );

    let mut wrong_activation = bytes.clone();
    wrong_activation[14] = 2;
    reject(&wrong_activation, "unsupported network activation 2");

    let mut wrong_scale = bytes.clone();
    wrong_scale[16..18].copy_from_slice(&1u16.to_le_bytes());
    reject(&wrong_scale, "network quantization scales do not match");

    let mut wrong_payload_length = bytes.clone();
    wrong_payload_length[20..24].copy_from_slice(&0u32.to_le_bytes());
    reject(
        &wrong_payload_length,
        "network dimensions or payload length do not match",
    );

    let mut trailing_byte = bytes.clone();
    trailing_byte.push(0);
    reject(
        &trailing_byte,
        "network dimensions or payload length do not match",
    );

    let mut damaged = bytes.clone();
    let last = damaged.len() - 1;
    damaged[last] ^= 0x80;
    assert!(
        QuantizedNnueNetwork::from_bytes(&damaged)
            .expect_err("checksum damage must be rejected")
            .to_string()
            .starts_with("network checksum mismatch:")
    );
}

#[test]
fn incremental_accumulator_matches_refresh_for_every_move_shape() {
    assert_line(
        Position::start(),
        &["e2e4", "d7d5", "e4d5", "d8d5", "b1c3", "d5d8"],
    );
    assert_line(
        Position::from_fen("r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1").unwrap(),
        &["e1g1", "e8c8"],
    );
    assert_line(
        Position::from_fen("4k3/8/8/3pP3/8/8/8/4K3 w - d6 0 1").unwrap(),
        &["e5d6"],
    );
    assert_line(
        Position::from_fen("4k3/P7/8/8/8/8/8/4K3 w - - 0 1").unwrap(),
        &["a7a8q"],
    );
}

#[test]
fn incremental_accumulator_survives_a_deterministic_game_walk_and_unmake() {
    let network = fixture().quantize();
    let mut position = Position::start();
    let mut accumulator = NnueAccumulator::refresh(&position, &network);
    let mut stack = Vec::new();

    for ply in 0..64 {
        let legal = position.legal_moves();
        if legal.is_empty() {
            break;
        }
        let index = (position.key() as usize ^ (ply * 0x9e37)) % legal.len();
        let mv = legal.as_slice()[index];
        let delta = FeatureDelta::from_move(&position, mv).unwrap();
        accumulator.apply(&network, &delta);
        let undo = position.make_move(mv).unwrap();
        assert_eq!(
            accumulator,
            NnueAccumulator::refresh(&position, &network),
            "incremental drift after ply {ply}, move {mv}"
        );
        assert_eq!(
            accumulator.evaluate(position.side_to_move(), &network),
            network.evaluate_refresh(&position)
        );
        stack.push((undo, delta));
    }

    while let Some((undo, delta)) = stack.pop() {
        position.unmake_move(undo);
        accumulator.revert(&network, &delta);
        assert_eq!(accumulator, NnueAccumulator::refresh(&position, &network));
    }
    assert_eq!(position, Position::start());
}

fn assert_line(mut position: Position, moves: &[&str]) {
    let network = fixture().quantize();
    let mut accumulator = NnueAccumulator::refresh(&position, &network);
    let mut stack = Vec::new();
    for notation in moves {
        let mv = position.find_move(notation).unwrap();
        let delta = FeatureDelta::from_move(&position, mv).unwrap();
        let before = accumulator.clone();
        accumulator.apply(&network, &delta);
        let undo = position.make_move(mv).unwrap();
        assert_eq!(accumulator, NnueAccumulator::refresh(&position, &network));

        accumulator.revert(&network, &delta);
        assert_eq!(accumulator, before);
        accumulator.apply(&network, &delta);
        stack.push((undo, delta));
    }
    while let Some((undo, delta)) = stack.pop() {
        position.unmake_move(undo);
        accumulator.revert(&network, &delta);
        assert_eq!(accumulator, NnueAccumulator::refresh(&position, &network));
    }
}
