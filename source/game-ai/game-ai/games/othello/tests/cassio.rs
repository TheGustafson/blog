use ai_othello::{CassioEngine, Position};
use std::cell::Cell;

#[test]
fn cassio_position_round_trips_both_side_token_forms() {
    let position = Position::start();
    let encoded = position.to_cassio();
    assert_eq!(encoded.len(), 65);
    assert_eq!(
        Position::from_cassio(&encoded[..64], &encoded[64..]),
        Ok(position)
    );

    let mut engine = CassioEngine::new();
    let contiguous = engine.command(&format!(
        "ENGINE-PROTOCOL midgame-search {encoded} -64 64 2 100"
    ));
    let split = engine.command(&format!(
        "ENGINE-PROTOCOL midgame-search {} {} -64 64 2 100",
        &encoded[..64],
        &encoded[64..]
    ));
    assert!(contiguous[0].starts_with(&encoded));
    assert!(split[0].starts_with(&encoded));
    assert_eq!(contiguous.last().unwrap(), "ready.");
    assert_eq!(split.last().unwrap(), "ready.");
}

#[test]
fn cassio_core_transcript_is_compatible_and_searches_the_real_position() {
    let mut engine = CassioEngine::new();
    assert_eq!(engine.command("ENGINE-PROTOCOL init"), ["ready."]);
    assert_eq!(
        engine.command("ENGINE-PROTOCOL get-version"),
        ["version: AI Othello 0.1.0", "ready."]
    );
    assert_eq!(engine.command("ENGINE-PROTOCOL new-position"), ["ready."]);

    let position = Position::start().to_cassio();
    let searched = engine.command(&format!(
        "ENGINE-PROTOCOL midgame-search {position} -64 64 4 100"
    ));
    assert!(searched[0].contains(", move d3, depth 4, @100%, X"));
    assert!(searched[0].contains("X+0.04 <= v <= X+0.04"));
    assert!(searched[0].contains(", d3c5d6e3, node 226,"));
    assert!(searched[0].contains(", time "));
    assert_eq!(searched[1], "ready.");
    assert_eq!(engine.command("ENGINE-PROTOCOL quit"), ["bye bye."]);
}

#[test]
fn cassio_search_can_be_stopped_cooperatively() {
    let mut engine = CassioEngine::new();
    let position = Position::start().to_cassio();
    let polls = Cell::new(0u64);
    let response = engine.command_until(
        &format!("ENGINE-PROTOCOL midgame-search {position} -64 64 16 100"),
        || {
            let next = polls.get() + 1;
            polls.set(next);
            next > 1_000
        },
    );
    assert!(polls.get() > 1_000);
    assert_eq!(response, ["ready."]);
}

#[test]
fn malformed_cassio_input_reports_errors_without_panicking() {
    let mut engine = CassioEngine::new();
    for command in [
        "",
        "init",
        "ENGINE-PROTOCOL",
        "ENGINE-PROTOCOL wat",
        "ENGINE-PROTOCOL init extra",
        "ENGINE-PROTOCOL midgame-search",
        "ENGINE-PROTOCOL midgame-search bad X -64 64 4 100",
        "ENGINE-PROTOCOL midgame-search ---------------------------------------------------------------- X -64 64 17 100",
        "ENGINE-PROTOCOL midgame-search ---------------------------------------------------------------- X 64 -64 4 100",
        "ENGINE-PROTOCOL endgame-search ---------------------------------------------------------------- X -64 64 100",
        "ENGINE-PROTOCOL feed-hash anything",
        "ENGINE-PROTOCOL get-search-infos",
    ] {
        let response = engine.command(command);
        assert!(
            response
                .first()
                .is_some_and(|line| line.starts_with("ERROR:")),
            "{command:?}: {response:?}"
        );
    }

    let split_inside_utf8 = format!(
        "ENGINE-PROTOCOL midgame-search {}é -64 64 4 100",
        "-".repeat(63)
    );
    let response = engine.command(&split_inside_utf8);
    assert_eq!(
        response,
        [
            "ERROR: midgame-search: expected a 64-cell board and X/O side",
            "ready.",
        ]
    );
}
