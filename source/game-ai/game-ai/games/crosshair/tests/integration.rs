use ai_crosshair::{
    BOTTOM_EDGE_MASK, Color, Engine, GameResult, LEFT_EDGE_MASK, MctsOptions, MctsSearcher, Move,
    MoveError, PolicyValueNetwork, Position, RIGHT_EDGE_MASK, Strategy, TOP_EDGE_MASK, WinReason,
    canonical_index, encode, perft, transform_index, transpose_index,
};

fn mv(coord: &str) -> Move {
    coord.parse().expect("valid coordinate")
}

#[test]
fn perft_counts_opening_and_routed_plies() {
    let start = Position::start();
    assert_eq!(perft(start, 0), 1);
    assert_eq!(perft(start, 1), 49);
    // Every opening move leaves 12 legal crosshair cells on ply 1 -> 49 * 12 = 588
    assert_eq!(perft(start, 2), 49 * 12);
}

#[test]
fn linear_and_2x2_crosscut_pinch_flips_match_h2a_rules() {
    // 1. 2x2 Crosscut: R at c3, B at d3, R at d4, B at c4 -> B completes diagonal (d3, c4)
    // against R's diagonal (c3, d4), flipping both c3 and d4 to Blue!
    let pos = Position::from_moves(&[mv("c3"), mv("d3"), mv("d4"), mv("c4")]).unwrap();
    assert_eq!(pos.color_at(mv("c3")), Some(Color::Blue));
    assert_eq!(pos.color_at(mv("d3")), Some(Color::Blue));
    assert_eq!(pos.color_at(mv("c4")), Some(Color::Blue));
    assert_eq!(pos.color_at(mv("d4")), Some(Color::Blue));
    assert_eq!(pos.counts(), (0, 4));
    assert_eq!(
        pos.last_flips().iter().collect::<Vec<_>>(),
        vec![mv("c3"), mv("d4")]
    );

    // 2. Verify flips_for_move agrees with play across multi-step sequences
    let mut cur = Position::start();
    let mut rng = 0x1234_5678_9abc_def0_u64;
    for _ in 0..20 {
        let legal: Vec<Move> = cur.legal_moves().iter().collect();
        if legal.is_empty() {
            break;
        }
        rng = rng
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let chosen = legal[(rng as usize) % legal.len()];
        let expected_flips = cur.flips_for_move(chosen);
        let next = cur.play(chosen).unwrap();
        assert_eq!(next.last_flips(), expected_flips);
        cur = next;
    }
}

#[test]
fn one_thousand_random_games_all_terminate_with_connection_win_and_zero_draws() {
    for seed in 0..1_000_u64 {
        let mut pos = Position::start();
        let mut state = seed
            .wrapping_mul(0x9e37_79b9_7f4a_7c15)
            .wrapping_add(0xbf58_476d_1ce4_e5b9);
        while pos.result() == GameResult::Ongoing {
            assert!(pos.ply() < 49, "ongoing game must have ply < 49");
            let legal: Vec<Move> = pos.legal_moves().iter().collect();
            assert!(!legal.is_empty(), "ongoing game must have legal moves");
            state = state
                .wrapping_mul(0x5851_f42d_4c95_7f2d)
                .wrapping_add(0x1405_7b7e_f767_814f);
            let chosen = legal[(state as usize) % legal.len()];
            pos = pos.play(chosen).unwrap();
        }
        assert!(pos.ply() <= 49);
        match pos.result() {
            GameResult::Win { winner, reason } => {
                assert_eq!(reason, WinReason::Connection);
                let line = pos.winning_line();
                assert!(
                    line.len() >= 7,
                    "shortest 4-orthogonal path across 7x7 board has at least 7 stones (got {})",
                    line.len()
                );
                // Every stone in winning_line must belong to winner and touch both goal edges
                let mask = line.mask();
                assert_eq!(mask & pos.stones_for(winner), mask);
                match winner {
                    Color::Red => {
                        assert_ne!(mask & TOP_EDGE_MASK, 0);
                        assert_ne!(mask & BOTTOM_EDGE_MASK, 0);
                    }
                    Color::Blue => {
                        assert_ne!(mask & LEFT_EDGE_MASK, 0);
                        assert_ne!(mask & RIGHT_EDGE_MASK, 0);
                    }
                }
            }
            GameResult::Ongoing => unreachable!(),
        }
    }
}

#[test]
fn wildcard_activates_when_crosshair_is_completely_occupied() {
    // Fill b1..g1 on row 1 and a2..a7 on col a, then play a1 so both row 1 and col a are 100% full
    let seq = [
        mv("b1"),
        mv("c1"),
        mv("e1"),
        mv("f1"),
        mv("g1"),
        mv("d1"),
        mv("d2"),
        mv("a2"),
        mv("a3"),
        mv("a5"),
        mv("a6"),
        mv("a7"),
        mv("a4"),
        mv("a1"),
    ];
    let pos = Position::from_moves(&seq).unwrap();
    assert_eq!(pos.result(), GameResult::Ongoing);
    assert!(pos.is_wildcard());
    assert_eq!(pos.active_row(), None);
    assert_eq!(pos.active_col(), None);
    assert_eq!(pos.legal_moves().len(), 49 - seq.len());
}

#[test]
fn move_previews_match_actual_play_transitions() {
    let pos = Position::from_moves(&[mv("a1"), mv("b1"), mv("b2")]).unwrap();
    let previews = pos.move_previews();
    assert_eq!(previews.len(), pos.legal_moves().len());

    for preview in previews {
        let next = pos.play(preview.mv).unwrap();
        assert_eq!(preview.flips, next.last_flips());
        if next.result() == GameResult::Ongoing {
            assert_eq!(preview.next_active_row, next.active_row());
            assert_eq!(preview.next_active_col, next.active_col());
            assert_eq!(preview.next_wildcard, next.is_wildcard());
        }
    }

    // Playing a2 completes a 2x2 crosscut for Blue against Red's a1 and b2
    let a2_preview = pos
        .move_previews()
        .into_iter()
        .find(|p| p.mv == mv("a2"))
        .unwrap();
    assert_eq!(
        a2_preview.flips.iter().collect::<Vec<_>>(),
        vec![mv("a1"), mv("b2")]
    );
}

#[test]
fn engine_protocol_handles_all_five_strategies_and_error_recovery() {
    let mut engine = Engine::new();
    assert_eq!(
        engine.command("gai"),
        vec![
            "id name ai-crosshair",
            "id author Nick Gustafson",
            "id game crosshair",
            "gaiok",
        ]
    );
    assert!(engine.command("newgame").is_empty());
    assert!(engine.command("play d4").is_empty());
    assert_eq!(engine.position().last_move(), Some(mv("d4")));

    let occupied_err = engine.command("play d4");
    assert_eq!(
        occupied_err,
        vec![format!("error message {}", MoveError::CellOccupied)]
    );
    assert_eq!(engine.position().ply(), 1);

    for strat in [
        "az-puct",
        "handcrafted-puct",
        "tactical-uct",
        "random-uct",
        "alpha-beta",
    ] {
        let out = engine.command(&format!(
            "search strategy {strat} simulations 32 depth 3 nodes 2000 softtime 0 seed 9"
        ));
        assert_eq!(out.len(), 1);
        assert!(out[0].starts_with("bestmove "));
        let snap: serde_json::Value = serde_json::from_str(&engine.snapshot_json()).unwrap();
        assert_eq!(snap["decision"]["strategy"], strat);
        let best_str = snap["decision"]["bestMove"].as_str().unwrap();
        let best_mv: Move = best_str.parse().unwrap();
        assert!(engine.position().legal_moves().contains(best_mv));
    }
}

#[test]
fn symmetry_and_blue_transpose_canonicalization_round_trip() {
    for pos in [
        Position::from_moves(&[mv("d4"), mv("d2"), mv("b2")]).unwrap(), // Blue to move
        Position::from_moves(&[mv("d4"), mv("d2"), mv("b2"), mv("b5")]).unwrap(), // Red to move
    ] {
        let side = pos.side_to_move();
        let base = encode(pos, 0);
        for sym in 0..4_u8 {
            let transformed = encode(pos, sym);
            for c in 0..6 {
                for idx in 0..49_u8 {
                    let c_base = canonical_index(idx, side, 0) as usize;
                    let c_sym = canonical_index(idx, side, sym) as usize;
                    assert_eq!(c_sym, transform_index(c_base as u8, sym) as usize);
                    assert_eq!(
                        base[c * 49 + c_base],
                        transformed[c * 49 + c_sym],
                        "channel {c} cell {idx} mismatch under symmetry {sym}"
                    );
                }
            }
        }
    }

    // Verify transpose_index is an involution
    for idx in 0..49_u8 {
        assert_eq!(transpose_index(transpose_index(idx)), idx);
    }

    let net = PolicyValueNetwork::embedded().unwrap();
    let mut searcher = MctsSearcher::with_network(net);
    let report = searcher.search(
        Position::from_moves(&[mv("c2")]).unwrap(),
        MctsOptions {
            max_simulations: 50,
            strategy: Strategy::AzPuct,
            seed: 5,
            ..MctsOptions::default()
        },
    );
    assert!(report.best_move.is_some());
}
