use ai_othello::Engine;

#[test]
fn protocol_exposes_flip_and_evaluation_data_from_one_session() {
    let mut engine = Engine::new();
    let handshake = engine.command("gai");
    assert_eq!(handshake.last().unwrap(), "gaiok");
    assert!(
        handshake.iter().all(|line| !line.contains("ExactEndgame")),
        "the exact-endgame threshold is a go limit, not a setoption"
    );
    assert!(engine.command("position startpos moves d3 c3").is_empty());
    assert!(
        engine
            .command("setoption name Evaluator value mobility")
            .is_empty()
    );
    let searched = engine.command("go depth 4 endgame 8");
    assert!(searched[0].starts_with("info depth 4 score eval "));
    assert!(searched[0].contains(" cutoffs "));
    assert!(searched[1].starts_with("bestmove "));
    let state = engine.command("state");
    assert!(state[0].starts_with("state {\"game\":\"othello\""));
    assert!(state[0].contains("\"lastFlips\""));
    assert!(state[0].contains("\"evaluation\":{\"profile\":\"mobility\""));
    assert!(state[0].contains("\"candidates\""));
}

#[test]
fn browser_snapshot_omits_developer_diagnostics() {
    let mut engine = Engine::new();
    assert!(engine.command("position startpos moves d3 c3").is_empty());
    assert!(
        engine
            .command("setoption name Evaluator value frontier")
            .is_empty()
    );
    let searched = engine.command("go depth 4 endgame 8");
    assert!(searched[1].starts_with("bestmove "));

    let snapshot = engine.snapshot_json();
    assert!(snapshot.starts_with("{\"board\":["));
    assert!(snapshot.contains("\"evaluator\":\"frontier\""));
    assert!(snapshot.contains("\"overlays\":{\"legal\":["));
    assert!(snapshot.contains("\"analysis\":{\"bestMove\":"));
    for diagnostic in [
        "\"game\"",
        "\"blackFrontier\"",
        "\"whiteFrontier\"",
        "\"evaluation\"",
        "\"candidates\"",
    ] {
        assert!(
            !snapshot.contains(diagnostic),
            "browser snapshot contains {diagnostic}"
        );
    }
    assert!(snapshot.len() < 1_500);

    let state = engine.command("state");
    assert!(state[0].starts_with("state {\"game\":\"othello\""));
    assert!(state[0].contains("\"blackFrontier\""));
    assert!(state[0].contains("\"evaluation\""));
    assert!(state[0].contains("\"candidates\""));
}

#[test]
fn malformed_protocol_input_is_structured() {
    let mut engine = Engine::new();
    for command in [
        "",
        "wat",
        "position",
        "position startpos wat",
        "position startpos moves z9",
        "position startpos moves a1",
        "setoption",
        "setoption name Evaluator value magic",
        "setoption name ExactEndgame value 8",
        "go",
        "go depth 17",
        "go depth many",
        "go depth 4 depth 5",
        "go nodes 20",
        "go depth 4 endgame 17",
        "perft",
        "perft many",
        "perft 13",
    ] {
        let response = engine.command(command);
        assert!(
            response
                .first()
                .is_some_and(|line| line.starts_with("error code ")),
            "{command:?}: {response:?}"
        );
    }
}
