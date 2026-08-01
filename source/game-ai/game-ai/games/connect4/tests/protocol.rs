use ai_connect4::Engine;

#[test]
fn handshake_position_search_and_snapshot_share_one_session() {
    let mut engine = Engine::new();
    assert_eq!(engine.command("gai").last().unwrap(), "gaiok");
    assert!(engine.command("position startpos moves d c d e").is_empty());
    assert!(
        engine
            .command("setoption name Algorithm value ordered")
            .is_empty()
    );
    let search = engine.command("go depth 6");
    assert!(search[0].starts_with("info depth 6 score eval "));
    assert!(search[0].contains(" nodes "));
    assert!(search[0].contains(" cutoffs "));
    assert!(search[1].starts_with("bestmove "));

    let state = engine.command("state");
    assert!(state[0].starts_with("state {\"game\":\"connect4\""));
    assert!(state[0].contains("\"history\":[\"d\",\"c\",\"d\",\"e\"]"));
    assert!(state[0].contains("\"algorithm\":\"ordered\""));
    assert!(state[0].contains("\"rootBranches\""));

    let iterative = engine.command("go iterative depth 9 nodes 20000");
    assert!(iterative[0].starts_with("info depth "));
    assert!(iterative[0].contains(" nodes 20000 "));
    assert!(iterative[0].contains(" iterations "));

    assert!(engine.command("position startpos moves d d e e").is_empty());
    assert_eq!(
        engine.command("oracle"),
        ["oracle source gamesolver-tutorial notation 4455 score 18 outcome win"]
    );
    assert!(engine.command("state")[0].contains("\"oracle\":{\"status\":\"hit\""));
}

#[test]
fn browser_snapshot_omits_developer_diagnostics() {
    let mut engine = Engine::new();
    assert!(engine.command("position startpos moves d c d e").is_empty());
    assert!(
        engine
            .command("setoption name Algorithm value ordered")
            .is_empty()
    );
    let search = engine.command("go depth 6");
    assert!(search[1].starts_with("bestmove "));

    let snapshot = engine.snapshot_json();
    assert!(snapshot.starts_with("{\"columns\":["));
    assert!(snapshot.contains("\"history\":[\"d\",\"c\",\"d\",\"e\"]"));
    assert!(snapshot.contains("\"analysis\":{\"bestMove\":"));
    for diagnostic in [
        "\"game\"",
        "\"algorithm\"",
        "\"oracle\"",
        "\"depth\"",
        "\"score\"",
        "\"stats\"",
        "\"rootBranches\"",
    ] {
        assert!(
            !snapshot.contains(diagnostic),
            "browser snapshot contains {diagnostic}"
        );
    }
    assert!(snapshot.len() < 1_000);

    let state = engine.command("state");
    assert!(state[0].starts_with("state {\"game\":\"connect4\""));
    assert!(state[0].contains("\"algorithm\":\"ordered\""));
    assert!(state[0].contains("\"depth\":6"));
    assert!(state[0].contains("\"score\""));
    assert!(state[0].contains("\"stats\""));
    assert!(state[0].contains("\"rootBranches\""));
}

#[test]
fn malformed_commands_return_structured_errors_without_panicking() {
    let mut engine = Engine::new();
    for command in [
        "",
        "wat",
        "position",
        "position startpos wat",
        "position startpos moves z",
        "setoption",
        "setoption name Algorithm value magic",
        "go",
        "go depth 0",
        "go depth 43",
        "go nodes 100",
        "go depth seven",
        "go depth 7 depth 8",
        "go depth 8 nodes 0",
        "oracle extra",
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
