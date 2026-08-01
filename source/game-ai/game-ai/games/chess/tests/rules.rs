use ai_chess::{
    CastlingRights, Color, Evaluation, EvaluationProfile, GameResult, MoveKind, Piece, PieceKind,
    Position, Square, evaluate, perft, piece_contributions,
};

#[test]
fn start_position_is_hybrid_and_fen_round_trips() {
    let mut position = Position::start();
    assert_eq!(position.side_to_move(), Color::White);
    assert_eq!(position.occupied().count_ones(), 32);
    assert_eq!(position.occupancy(Color::White).count_ones(), 16);
    assert_eq!(position.occupancy(Color::Black).count_ones(), 16);
    assert_eq!(position.legal_moves().len(), 20);
    assert_ne!(position.key(), 0);
    position.assert_consistent();
    assert_eq!(Position::from_fen(&position.fen()).unwrap(), position);
}

#[test]
fn every_opening_move_makes_and_unmakes_bit_for_bit() {
    let mut position = Position::start();
    let original = position.clone();
    let original_key = position.key();
    for mv in position.legal_moves() {
        let undo = position.make_move(mv).unwrap();
        assert_ne!(position.key(), original_key);
        position.assert_consistent();
        position.unmake_move(undo);
        assert_eq!(position, original);
        assert_eq!(position.key(), original_key);
    }
}

#[test]
fn standard_start_perft_is_locked_through_depth_four() {
    let expected = [1, 20, 400, 8_902, 197_281];
    for (depth, nodes) in expected.into_iter().enumerate() {
        assert_eq!(perft(&mut Position::start(), depth as u8), nodes);
    }
}

#[test]
fn kiwipete_exercises_castling_pins_and_slider_blockers() {
    let fen = "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1";
    let mut position = Position::from_fen(fen).unwrap();
    for (depth, nodes) in [(1, 48), (2, 2_039), (3, 97_862)] {
        assert_eq!(perft(&mut position, depth), nodes);
    }
}

#[test]
fn endgame_reference_exercises_checks_and_promotions() {
    let fen = "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1";
    let mut position = Position::from_fen(fen).unwrap();
    for (depth, nodes) in [(1, 14), (2, 191), (3, 2_812), (4, 43_238)] {
        assert_eq!(perft(&mut position, depth), nodes);
    }
}

#[test]
fn the_remaining_standard_perft_positions_match_at_fast_depths() {
    let cases = [
        (
            "r3k2r/Pppp1ppp/1b3nbN/nP6/BBP1P3/q4N2/Pp1P2PP/R2Q1RK1 w kq - 0 1",
            [6, 264, 9_467],
        ),
        (
            "rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8",
            [44, 1_486, 62_379],
        ),
        (
            "r4rk1/1pp1qppp/p1np1n2/2b1p1B1/2B1P1b1/P1NP1N2/1PP1QPPP/R4RK1 w - - 0 10",
            [46, 2_079, 89_890],
        ),
    ];
    for (fen, expected) in cases {
        let mut position = Position::from_fen(fen).unwrap();
        for (index, nodes) in expected.into_iter().enumerate() {
            assert_eq!(perft(&mut position, index as u8 + 1), nodes, "{fen}");
        }
    }
}

#[test]
fn special_moves_have_explicit_kinds_and_round_trip() {
    let mut castling = Position::from_fen("r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1").unwrap();
    let castle_moves = castling.legal_moves();
    assert!(
        castle_moves
            .as_slice()
            .iter()
            .any(|mv| { mv.to_string() == "e1g1" && mv.kind() == MoveKind::CastleKingSide })
    );
    assert!(
        castle_moves
            .as_slice()
            .iter()
            .any(|mv| { mv.to_string() == "e1c1" && mv.kind() == MoveKind::CastleQueenSide })
    );
    let original = castling.clone();
    let mv = castling.find_move("e1g1").unwrap();
    let undo = castling.make_move(mv).unwrap();
    assert_eq!(
        castling.piece_at("f1".parse().unwrap()),
        Some(Piece::new(Color::White, PieceKind::Rook))
    );
    assert!(!castling.castling_rights().has(CastlingRights::WHITE_KING));
    castling.unmake_move(undo);
    assert_eq!(castling, original);

    let mut promotion = Position::from_fen("4k3/P7/8/8/8/8/8/4K3 w - - 0 1").unwrap();
    let promotions: Vec<_> = promotion
        .legal_moves()
        .into_iter()
        .filter(|mv| mv.from() == "a7".parse::<Square>().unwrap())
        .collect();
    assert_eq!(promotions.len(), 4);
    assert_eq!(
        promotions
            .iter()
            .filter_map(|mv| mv.promotion())
            .collect::<Vec<_>>(),
        PieceKind::PROMOTIONS
    );
}

#[test]
fn en_passant_can_be_legal_or_illegal_because_of_a_pin() {
    let mut legal = Position::from_fen("4k3/8/8/3pP3/8/8/8/4K3 w - d6 0 1").unwrap();
    let mv = legal.find_move("e5d6").unwrap();
    assert_eq!(mv.kind(), MoveKind::EnPassant);
    let original = legal.clone();
    let undo = legal.make_move(mv).unwrap();
    assert!(legal.piece_at("d5".parse().unwrap()).is_none());
    legal.unmake_move(undo);
    assert_eq!(legal, original);

    let mut pinned = Position::from_fen("k3r3/8/8/3pP3/8/8/8/4K3 w - d6 0 1").unwrap();
    assert!(pinned.find_move("e5d6").is_err());
}

#[test]
fn terminal_states_distinguish_mate_stalemate_and_rule_fifty() {
    let mut mate = Position::from_fen("7k/6Q1/6K1/8/8/8/8/8 b - - 0 1").unwrap();
    assert_eq!(
        mate.result(),
        GameResult::Checkmate {
            winner: Color::White
        }
    );
    let mut stalemate = Position::from_fen("7k/5Q2/6K1/8/8/8/8/8 b - - 0 1").unwrap();
    assert_eq!(stalemate.result(), GameResult::Stalemate);
    let mut fifty = Position::from_fen("7k/8/8/8/8/8/8/K7 w - - 100 51").unwrap();
    assert_eq!(fifty.result(), GameResult::FiftyMoveDraw);
    let mut mate_at_fifty = Position::from_fen("7k/6Q1/6K1/8/8/8/8/8 b - - 100 51").unwrap();
    assert_eq!(
        mate_at_fifty.result(),
        GameResult::Checkmate {
            winner: Color::White
        }
    );
}

#[test]
fn insufficient_material_is_conservative_and_color_aware() {
    for fen in [
        "7k/8/8/8/8/8/8/K7 w - - 0 1",
        "7k/8/8/8/8/8/8/KB6 w - - 0 1",
        "7k/8/8/8/8/8/8/KN6 w - - 0 1",
        "7k/8/8/8/4b3/8/8/KB6 w - - 0 1",
    ] {
        let mut position = Position::from_fen(fen).unwrap();
        assert!(position.has_insufficient_material(), "{fen}");
        assert_eq!(position.result(), GameResult::InsufficientMaterialDraw);
    }

    for fen in [
        "7k/8/8/8/8/4b3/8/KB6 w - - 0 1",
        "7k/8/8/8/8/8/8/KNN5 w - - 0 1",
        "7k/8/8/8/8/8/8/KBN5 w - - 0 1",
        "7k/8/8/8/8/8/P7/K7 w - - 0 1",
    ] {
        let mut position = Position::from_fen(fen).unwrap();
        assert!(!position.has_insufficient_material(), "{fen}");
        assert_eq!(position.result(), GameResult::Ongoing);
    }
}

#[test]
fn zobrist_includes_side_rights_and_en_passant_and_restores_exactly() {
    let white = Position::from_fen("4k3/8/8/8/8/8/8/4K3 w - - 0 1").unwrap();
    let black = Position::from_fen("4k3/8/8/8/8/8/8/4K3 b - - 0 1").unwrap();
    let rights = Position::from_fen("4k2r/8/8/8/8/8/8/4K2R w Kk - 0 1").unwrap();
    let ep = Position::from_fen("4k3/8/8/3pP3/8/8/8/4K3 w - d6 0 1").unwrap();
    let no_ep = Position::from_fen("4k3/8/8/4p3/8/8/8/4K3 w - - 0 1").unwrap();
    let phantom_ep = Position::from_fen("4k3/8/8/4p3/8/8/8/4K3 w - e6 0 1").unwrap();
    assert_ne!(white.key(), black.key());
    assert_ne!(white.key(), rights.key());
    assert_ne!(white.key(), ep.key());
    assert_eq!(no_ep.key(), phantom_ep.key());
}

#[test]
fn fen_rejects_impossible_rule_state_before_move_generation() {
    for fen in [
        "P3k3/8/8/8/8/8/8/4K3 w - - 0 1",
        "4k3/8/8/8/8/8/8/PPPPPPPP/PP2K3 w - - 0 1",
        "4k3/8/8/8/QQQQQQQQ/QQQQQQQQ/PPPPPPPP/4K3 w - - 0 1",
        "4k3/8/8/8/8/8/PPPPPPPP/QQ2K3 w - - 0 1",
        "4k3/8/8/4p3/8/8/8/4K3 w - e3 0 1",
        "4k3/8/8/8/8/8/8/4K3 w - e6 0 1",
        "4k3/4r3/8/4p3/8/8/8/4K3 w - e6 0 1",
        "4k3/8/8/4p3/8/8/8/4K3 w - e6 1 1",
        "4k3/8/8/8/8/8/4R3/4K3 w - - 0 1",
        "8/8/8/8/8/8/4k3/4K3 w - - 0 1",
        "4k3/8/8/8/8/8/8/4K3 w K - 0 1",
        "r3k2r/8/8/8/8/8/8/R3K2R w KK - 0 1",
    ] {
        assert!(Position::from_fen(fen).is_err(), "{fen}");
    }
    assert_eq!(
        Position::from_fen("4k3/8/8/4p3/8/8/8/4K3 w - e6 1 1"),
        Err("FEN en-passant target requires a zero halfmove clock")
    );

    assert!(
        Position::from_fen("4k3/8/8/4p3/8/8/8/4K3 w - e6 0 1").is_ok(),
        "an uncapturable but historically valid target remains legal"
    );
    assert!(
        Position::from_fen("4k3/8/8/8/8/8/1PPPPPPP/QQ2K3 w - - 0 1").is_ok(),
        "one missing pawn can account for one promoted queen"
    );
    assert!(
        Position::from_fen("4k3/8/8/8/8/8/4r3/4K3 w - - 0 1").is_ok(),
        "the side to move may legally be in check"
    );
}

#[test]
fn evaluation_is_traceable_tapered_and_color_symmetric() {
    for profile in EvaluationProfile::ALL {
        let start = evaluate(&Position::start(), profile);
        assert_eq!(start.phase, 24);
        if profile != EvaluationProfile::TinyNnue {
            assert_eq!(start.total, 0);
        } else {
            assert!(start.total.abs() < 200);
        }
        assert_eq!(start.total, start.terms_sum());
    }

    let white = Position::from_fen("4k3/8/8/8/3N4/2P5/8/4K3 w - - 0 1").unwrap();
    let black = Position::from_fen("4k3/8/2p5/3n4/8/8/8/4K3 b - - 0 1").unwrap();
    for profile in EvaluationProfile::ALL {
        let white_score = evaluate(&white, profile);
        let black_score = evaluate(&black, profile);
        assert_eq!(
            white_score,
            Evaluation {
                side_to_move: Color::White,
                ..black_score
            }
        );
        assert_eq!(white_score.total, white_score.terms_sum());
    }

    let pawn = Position::from_fen("4k3/8/8/8/8/2P5/8/4K3 w - - 0 1").unwrap();
    let material = evaluate(&pawn, EvaluationProfile::Material);
    let piece_square = evaluate(&pawn, EvaluationProfile::PieceSquare);
    assert_eq!(material.phase, 0);
    assert_eq!(material.total, 94);
    assert!(piece_square.total > material.total);
    assert_eq!(
        piece_contributions(&pawn, EvaluationProfile::PieceSquare)
            .iter()
            .map(|piece| piece.total)
            .sum::<i32>(),
        piece_square.total
    );
}

#[test]
fn move_parser_rejects_non_ascii_without_panicking() {
    assert!("é2e4".parse::<ai_chess::Move>().is_err());
}
