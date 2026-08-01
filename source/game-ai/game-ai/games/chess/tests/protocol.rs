use ai_chess::Engine;
use std::cell::Cell;

#[test]
fn uci_position_search_and_snapshot_share_one_session() {
    let mut engine = Engine::new();
    assert_eq!(
        engine.command("uci"),
        [
            "id name AI Chess",
            "id author Nick Gustafson",
            "option name Evaluator type combo default piece-square var material var piece-square var tiny-nnue",
            "option name Quiescence type check default true",
            "option name NNUE Accumulator type check default true",
            "option name Move Ordering type check default true",
            "option name Transposition Table type check default true",
            "uciok",
        ]
    );
    assert_eq!(engine.command("isready"), ["readyok"]);
    assert!(
        engine
            .command("position startpos moves e2e4 e7e5 g1f3")
            .is_empty()
    );
    assert!(
        engine
            .command("setoption name Evaluator value material")
            .is_empty()
    );
    assert!(
        engine
            .command("setoption name Quiescence value false")
            .is_empty()
    );

    let searched = engine.command("go depth 5 nodes 500");
    assert!(searched[0].starts_with("info depth "));
    assert!(searched[0].contains(" score cp "));
    assert!(searched[0].contains(" nodes 500 "));
    assert!(searched[1].starts_with("bestmove "));

    let state = engine.command("state");
    assert!(state[0].starts_with("state {\"game\":\"chess\""));
    assert!(state[0].contains("\"history\":[\"e2e4\",\"e7e5\",\"g1f3\"]"));
    assert!(state[0].contains("\"evaluator\":\"material\""));
    assert!(state[0].contains("\"quiescence\":false"));
    assert!(state[0].contains("\"pieces\""));
    assert!(state[0].contains("\"checksum\":\"f315930090909a44\""));
    assert!(state[0].contains("\"featureCount\":768,\"hiddenSize\":128"));
    assert!(state[0].contains(
        "\"lastDelta\":[{\"piece\":\"N\",\"color\":\"white\",\"kind\":\"knight\",\"square\":\"g1\",\"sign\":-1,\"whiteFeature\":70,\"blackFeature\":510}"
    ));
    assert!(state[0].contains(
        "{\"piece\":\"N\",\"color\":\"white\",\"kind\":\"knight\",\"square\":\"f3\",\"sign\":1,\"whiteFeature\":85,\"blackFeature\":493}]"
    ));
    assert!(state[0].contains("\"candidates\""));
    assert!(state[0].contains("\"iterations\""));
}

#[test]
fn browser_snapshot_omits_developer_diagnostics() {
    let mut engine = Engine::new();
    assert!(
        engine
            .command("position startpos moves e2e4 e7e5")
            .is_empty()
    );
    let searched = engine.command("go depth 3");
    assert!(searched[1].starts_with("bestmove "));

    let snapshot = engine.snapshot_json();
    assert!(snapshot.starts_with("{\"board\":["));
    assert!(snapshot.contains("\"analysis\":{\"bestMove\":"));
    for diagnostic in [
        "\"game\"",
        "\"evaluation\"",
        "\"nnue\"",
        "\"options\"",
        "\"candidates\"",
        "\"iterations\"",
    ] {
        assert!(
            !snapshot.contains(diagnostic),
            "browser snapshot contains {diagnostic}"
        );
    }
    assert!(snapshot.len() < 2_500);

    let state = engine.command("state");
    assert!(state[0].starts_with("state {\"game\":\"chess\""));
    assert!(state[0].contains("\"evaluation\""));
    assert!(state[0].contains("\"nnue\""));
}

#[test]
fn full_fen_special_move_and_developer_commands_are_supported() {
    let mut engine = Engine::new();
    assert!(
        engine
            .command("position fen 4k3/P7/8/8/8/8/8/4K3 w - - 0 1 moves a7a8n")
            .is_empty()
    );
    assert!(engine.command("d")[0].contains("N3k3/8/8/8/8/8/8/4K3 b - - 0 1"));
    assert!(engine.command("eval")[0].starts_with("info string eval profile "));
    let perft = engine.command("perft 1");
    assert!(perft[0].starts_with("info string perft depth 1 nodes 5 time "));
    let legal = engine.command("legal");
    assert!(legal[0].starts_with("legalmoves "));
}

#[test]
fn snapshot_names_an_insufficient_material_draw() {
    let mut engine = Engine::new();
    assert!(
        engine
            .command("position fen 7k/8/8/8/8/8/8/KN6 w - - 0 1")
            .is_empty()
    );
    let state = engine.command("state");
    assert!(state[0].contains("\"result\":\"insufficient-material\""));
    assert!(state[0].contains("\"winner\":null"));

    let searched = engine.command("go depth 4");
    assert!(searched[0].contains(" score cp 0 "));
    assert_eq!(searched[1], "bestmove 0000");
}

#[test]
fn cooperative_stop_returns_the_last_complete_infinite_iteration() {
    let mut engine = Engine::new();
    let polls = Cell::new(0u64);
    let searched = engine.command_until("go infinite", || {
        let next = polls.get() + 1;
        polls.set(next);
        next > 2_500
    });
    assert!(polls.get() > 2_500);
    assert!(searched[0].starts_with("info depth "));
    assert!(!searched[0].starts_with("info depth 64 "));
    assert!(searched[1].starts_with("bestmove "));
    assert_ne!(searched[1], "bestmove 0000");

    let state = engine.command("state");
    assert!(state[0].contains("\"requestedDepth\":64"));
    assert!(state[0].contains("\"completed\":false"));
    assert!(state[0].contains("\"completed\":true"));
}

#[test]
fn malformed_uci_input_reports_errors_without_panicking() {
    let mut engine = Engine::new();
    for command in [
        "",
        "wat",
        "position",
        "position state nope",
        "position fen 8/8/8",
        "position fen 4k3/8/8/8/8/8/8/4K3 w - e6 0 1",
        "position fen 4k3/8/8/8/8/8/4R3/4K3 w - - 0 1",
        "position startpos wat",
        "position startpos moves e2e5",
        "setoption",
        "setoption name Evaluator value magic",
        "setoption name Quiescence value perhaps",
        "go depth 0",
        "go depth many",
        "go nodes 0",
        "go movetime 0",
        "go movestogo 0",
        "go depth 2 depth 3",
        "go infinite infinite",
        "go searchmoves e2e4",
        "perft",
        "perft 7",
        "bench extra",
    ] {
        let response = engine.command(command);
        assert!(
            response
                .first()
                .is_some_and(|line| line.starts_with("info string error ")),
            "{command:?}: {response:?}"
        );
    }

    assert_eq!(
        engine.command("position fen 4k3/8/8/8/8/8/8/4K3 w - e6 0 1"),
        ["info string error FEN en-passant target has no capturable pawn"]
    );
}
