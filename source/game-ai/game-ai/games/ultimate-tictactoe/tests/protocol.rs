use ai_ultimate_tictactoe::Engine;

#[test]
fn browser_snapshot_is_small_and_contains_the_play_contract() {
    let engine = Engine::new();
    let snapshot = engine.snapshot_json();
    assert!(snapshot.starts_with("{\"board\":["));
    assert!(snapshot.contains("\"activeBoard\":null"));
    assert!(snapshot.contains("\"sideToMove\":\"X\""));
    assert!(snapshot.contains("\"legalMoves\":[\"a9\""));
    assert!(snapshot.contains("\"decision\":null"));
    assert!(snapshot.len() < 2_500);
}

#[test]
fn commands_round_trip_a_routed_position() {
    let mut engine = Engine::new();
    assert_eq!(
        engine.command("position startpos moves e5 d6"),
        Vec::<String>::new()
    );
    let snapshot = engine.snapshot_json();
    assert!(snapshot.contains("\"activeBoard\":0"));
    assert!(snapshot.contains("\"history\":[\"e5\",\"d6\"]"));
    assert!(snapshot.contains("\"sideToMove\":\"X\""));
}

#[test]
fn malformed_and_illegal_commands_report_errors_without_mutating_state() {
    let mut engine = Engine::new();
    let before = engine.snapshot_json();
    for command in [
        "position fen nonsense",
        "position startpos moves z0",
        "position startpos moves e5 b9",
        "play depth 0",
        "play nodes nope",
        "play softtime 1001",
    ] {
        assert!(engine.command(command)[0].starts_with("info string error "));
        assert_eq!(engine.snapshot_json(), before);
    }
}

#[test]
fn play_returns_search_metadata_without_playing_the_move() {
    let mut engine = Engine::new();
    let response = engine.command("play depth 2 nodes 10000 softtime 100");
    assert!(response[0].starts_with("bestmove "));
    let snapshot = engine.snapshot_json();
    assert!(snapshot.contains("\"decision\":{\"bestMove\":"));
    assert!(snapshot.contains("\"history\":[]"));
}
