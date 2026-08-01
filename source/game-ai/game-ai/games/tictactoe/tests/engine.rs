use ai_tictactoe::{
    Algorithm, DecisionReason, Engine, GameResult, Move, Outcome, PlayStrategy, Position, Side,
    Tablebase, build_tree, choose_move, search,
};
use std::collections::HashSet;

#[test]
fn all_reachable_positions_are_consistent_and_reversible() {
    fn visit(position: &mut Position, seen: &mut HashSet<Position>) {
        if !seen.insert(*position) || position.result() != GameResult::Ongoing {
            return;
        }
        let moves: Vec<_> = position.legal_moves().collect();
        for mv in moves {
            let before = *position;
            position.make_move(mv).unwrap();
            visit(position, seen);
            position.unmake_move(mv);
            assert_eq!(*position, before);
        }
    }

    let mut seen = HashSet::new();
    visit(&mut Position::start(), &mut seen);
    assert_eq!(seen.len(), 5_478);
}

#[test]
fn every_solver_agrees_on_every_reachable_position() {
    fn visit(position: Position, seen: &mut HashSet<Position>, tablebase: &Tablebase) {
        if !seen.insert(position) {
            return;
        }
        if position.result() == GameResult::Ongoing {
            let expected = search(position, Algorithm::Tablebase, tablebase).outcome;
            for algorithm in [Algorithm::Memo, Algorithm::Symmetry] {
                assert_eq!(
                    search(position, algorithm, tablebase).outcome,
                    expected,
                    "{algorithm} disagreed at {position:?}"
                );
            }
            for mv in position.legal_moves() {
                let mut child = position;
                child.make_move(mv).unwrap();
                visit(child, seen, tablebase);
            }
        }
    }

    let tablebase = Tablebase::build();
    let mut seen = HashSet::new();
    visit(Position::start(), &mut seen, &tablebase);
    assert_eq!(seen.len(), 5_478);
}

#[test]
fn the_empty_board_is_a_draw_with_perfect_play() {
    let tablebase = Tablebase::build();
    assert_eq!(tablebase.reachable_positions(), 5_478);
    assert_eq!(tablebase.canonical_positions(), 765);

    let expected = [
        (Algorithm::Plain, 549_945, 0),
        (Algorithm::Memo, 16_167, 10_690),
        (Algorithm::Symmetry, 2_270, 1_506),
        (Algorithm::Tablebase, 9, 0),
    ];
    for (algorithm, nodes, cache_hits) in expected {
        let report = search(Position::start(), algorithm, &tablebase);
        assert_eq!(report.outcome, Outcome::Draw);
        assert_eq!(report.best_move.unwrap().to_string(), "b2");
        assert_eq!(
            (report.stats.nodes, report.stats.cache_hits),
            (nodes, cache_hits),
            "published work signature changed for {algorithm}",
        );
    }
}

#[test]
fn finds_an_immediate_win() {
    let moves: Vec<Move> = ["a1", "a2", "b1", "b2"]
        .into_iter()
        .map(|value| value.parse().unwrap())
        .collect();
    let position = Position::from_moves(&moves).unwrap();
    assert_eq!(position.side_to_move(), Side::X);

    let tablebase = Tablebase::build();
    let report = search(position, Algorithm::Tablebase, &tablebase);
    assert_eq!(report.outcome, Outcome::Win);
    assert_eq!(report.best_move.unwrap().to_string(), "c1");
    assert_eq!(report.distance, 1);

    let won = position_from(&["a1", "a2", "b1", "b2", "c1"]);
    let line: Vec<_> = won
        .winning_squares()
        .into_iter()
        .map(|square| square.to_string())
        .collect();
    assert_eq!(line, ["a1", "b1", "c1"]);
}

#[test]
fn tactical_play_wins_blocks_and_then_prefers_the_center() {
    let tablebase = Tablebase::build();

    let winning = position_from(&["a1", "a2", "b1", "b2"]);
    let win = choose_move(winning, PlayStrategy::Tactical, 0, &tablebase).unwrap();
    assert_eq!(win.best_move.to_string(), "c1");
    assert_eq!(win.reason, DecisionReason::ImmediateWin);
    assert_eq!(win.outcome, None);

    let blocking = position_from(&["a1", "b2", "b1"]);
    let block = choose_move(blocking, PlayStrategy::Tactical, 0, &tablebase).unwrap();
    assert_eq!(block.best_move.to_string(), "c1");
    assert_eq!(block.reason, DecisionReason::ImmediateBlock);

    let fallback = choose_move(Position::start(), PlayStrategy::Tactical, 0, &tablebase).unwrap();
    assert_eq!(fallback.best_move.to_string(), "b2");
    assert_eq!(fallback.reason, DecisionReason::PositionalFallback);
}

#[test]
fn random_play_is_seeded_and_exact_play_reports_an_exact_result() {
    let tablebase = Tablebase::build();
    let first = choose_move(Position::start(), PlayStrategy::Random, 42, &tablebase).unwrap();
    let repeated = choose_move(Position::start(), PlayStrategy::Random, 42, &tablebase).unwrap();
    assert_eq!(first, repeated);
    assert_eq!(first.reason, DecisionReason::RandomChoice);
    assert_eq!(first.outcome, None);

    let exact = choose_move(Position::start(), PlayStrategy::Symmetry, 0, &tablebase).unwrap();
    let searched = search(Position::start(), Algorithm::Symmetry, &tablebase);
    assert_eq!(exact.best_move, searched.best_move.unwrap());
    assert_eq!(exact.nodes, searched.stats.nodes);
    assert_eq!(exact.outcome, Some(Outcome::Draw));
    assert_eq!(exact.reason, DecisionReason::ExactSearch);
}

#[test]
fn shallow_tree_is_real_and_exposes_symmetry_classes() {
    let tablebase = Tablebase::build();
    let tree = build_tree(Position::start(), 2, &tablebase);
    assert_eq!(tree.nodes, 82);
    assert_eq!(tree.children.len(), 9);

    let root_groups: HashSet<_> = tree
        .children
        .iter()
        .map(|edge| edge.canonical_key)
        .collect();
    assert_eq!(root_groups.len(), 3, "center, corner, and edge");

    let corners: HashSet<_> = tree
        .children
        .iter()
        .filter(|edge| [0, 2, 6, 8].contains(&edge.mv.square().index()))
        .map(|edge| edge.canonical_key)
        .collect();
    assert_eq!(corners.len(), 1);
}

#[test]
fn protocol_transcript_is_stable() {
    let mut engine = Engine::new();
    assert_eq!(engine.command("gai").last().unwrap(), "gaiok");
    assert_eq!(engine.command("isready"), ["readyok"]);
    assert!(
        engine
            .command("setoption name Algorithm value symmetry")
            .is_empty()
    );
    assert!(engine.command("position startpos moves b2 a1").is_empty());
    let lines = engine.command("go");
    assert!(lines[0].starts_with("info depth "));
    assert!(lines[0].contains(" score wdl 0 "));
    assert!(lines[1].starts_with("bestmove "));

    let state = engine.command("state");
    assert!(state[0].starts_with("state {\"game\":\"tictactoe\""));
    assert!(state[0].contains("\"algorithm\":\"symmetry\""));

    let play = engine.command("play tactical");
    assert!(play[0].starts_with("info strategy tactical reason "));
    assert!(play[1].starts_with("bestmove "));
    let state = engine.command("state");
    assert!(state[0].contains("\"decision\":{\"strategy\":\"tactical\""));

    assert_eq!(engine.command("tree 2"), ["tree depth 2 nodes 50"]);
    let state = engine.command("state");
    assert!(state[0].contains("\"tree\":{\"depth\":2"));
}

#[test]
fn browser_snapshot_omits_solver_diagnostics() {
    let mut engine = Engine::new();
    assert!(engine.command("position startpos moves b2 a1").is_empty());
    assert!(
        engine
            .command("setoption name Algorithm value symmetry")
            .is_empty()
    );
    let play = engine.command("play tactical");
    assert!(play[1].starts_with("bestmove "));
    assert_eq!(engine.command("tree 2"), ["tree depth 2 nodes 50"]);

    let snapshot = engine.snapshot_json();
    assert!(snapshot.starts_with("{\"board\":["));
    assert!(snapshot.contains("\"history\":[\"b2\",\"a1\"]"));
    assert!(snapshot.contains("\"decision\":{\"bestMove\":"));
    for diagnostic in [
        "\"game\"",
        "\"stateSpace\"",
        "\"algorithm\"",
        "\"analysis\"",
        "\"strategy\"",
        "\"reason\"",
        "\"nodes\"",
        "\"tree\"",
    ] {
        assert!(
            !snapshot.contains(diagnostic),
            "browser snapshot contains {diagnostic}"
        );
    }
    assert!(snapshot.len() < 500);

    let state = engine.command("state");
    assert!(state[0].starts_with("state {\"game\":\"tictactoe\""));
    assert!(state[0].contains("\"stateSpace\""));
    assert!(state[0].contains("\"algorithm\":\"symmetry\""));
    assert!(state[0].contains("\"decision\":{\"strategy\":\"tactical\""));
    assert!(state[0].contains("\"tree\":{\"depth\":2"));
}

#[test]
fn malformed_protocol_input_never_panics() {
    let mut engine = Engine::new();
    for command in [
        "",
        "wat",
        "position",
        "position fen nope",
        "position startpos nope",
        "position startpos moves z9",
        "position startpos moves a1 a1",
        "setoption",
        "setoption name Algorithm value nope",
        "go depth 2",
        "play",
        "play nope",
        "play random seed nope",
        "play random extra words",
        "tree",
        "tree nope",
        "tree 4",
        "perft",
        "perft nope",
    ] {
        let result = engine.command(command);
        assert_eq!(result.len(), 1, "{command:?}");
        assert!(result[0].starts_with("error "), "{command:?}: {result:?}");
    }
}

fn position_from(moves: &[&str]) -> Position {
    let moves: Vec<Move> = moves.iter().map(|value| value.parse().unwrap()).collect();
    Position::from_moves(&moves).unwrap()
}
