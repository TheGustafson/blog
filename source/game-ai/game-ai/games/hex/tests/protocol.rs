use ai_hex::Engine;

fn snapshot(engine: &Engine) -> serde_json::Value {
    serde_json::from_str(&engine.snapshot_json()).unwrap()
}

#[test]
fn handshake_identifies_the_engine() {
    assert_eq!(
        Engine::new().command("gai"),
        ["id name ai-hex", "id game hex", "gaiok"]
    );
}

#[test]
fn position_round_trips_swap_state() {
    let mut engine = Engine::new();
    assert!(
        engine
            .command("position size 15 swap on moves h8 swap g8")
            .is_empty()
    );
    let state = snapshot(&engine);
    assert_eq!(state["size"], 15);
    assert_eq!(state["seatToMove"], "two");
    assert_eq!(state["colorToMove"], "R");
    assert_eq!(state["seatColors"], serde_json::json!(["B", "R"]));
    assert_eq!(state["history"], serde_json::json!(["h8", "swap", "g8"]));
}

#[test]
fn mcts_does_not_apply_its_answer() {
    let mut engine = Engine::new();
    engine.command("newgame size 9 swap on");
    let response = engine.command(
        "mcts simulations 25 softtime 0 exploration 0.2 strategy uct-rave rave 750 rollout save-bridge knowledge 32 connections on seed 3",
    );
    assert!(response[0].starts_with("bestmove "));
    let state = snapshot(&engine);
    assert_eq!(state["history"], serde_json::json!([]));
    assert_eq!(state["decision"]["simulations"], 25);
    assert_eq!(state["decision"]["algorithm"], "uct");
    assert_eq!(state["decision"]["strategy"], "uct-rave");
    assert_eq!(state["decision"]["raveEquivalence"], 750.0);
    assert_eq!(state["decision"]["rolloutPolicy"], "save-bridge");
    assert_eq!(state["decision"]["knowledgeThreshold"], 32);
    assert_eq!(state["decision"]["virtualConnectionsEnabled"], true);
    assert!(state["decision"]["bridgeReplies"].is_number());
    assert!(state["decision"]["prunedMoves"].is_number());
    assert!(state["decision"]["virtualConnections"].is_number());
    assert!(state["decision"]["semiConnections"].is_number());
    assert!(state["decision"]["provenNodes"].is_number());
    assert!(
        state["decision"]["provenWinner"].is_null()
            || state["decision"]["provenWinner"].is_string()
    );
    assert!(state["decision"]["rootMoves"][0]["raveVisits"].is_number());
    assert!(
        state["decision"]["rootMoves"][0]["provenWinner"].is_null()
            || state["decision"]["rootMoves"][0]["provenWinner"].is_string()
    );
}

#[test]
fn malformed_commands_return_errors() {
    let mut engine = Engine::new();
    assert!(engine.command("newgame size 8")[0].contains("error"));
    assert!(engine.command("position size 9 moves z1")[0].contains("error"));
    assert!(engine.command("mcts simulations 0")[0].contains("error"));
    assert!(engine.command("mcts softtime 2001")[0].contains("error"));
    assert!(engine.command("mcts strategy rave")[0].contains("error"));
    assert!(engine.command("mcts rave 0")[0].contains("error"));
    assert!(engine.command("mcts rollout bridge")[0].contains("error"));
    assert!(engine.command("mcts knowledge nope")[0].contains("error"));
    assert!(engine.command("mcts connections maybe")[0].contains("error"));
}
