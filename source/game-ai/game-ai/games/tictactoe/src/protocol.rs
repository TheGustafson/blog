use crate::mv::Move;
use crate::play::{DecisionReport, PlayStrategy, choose_move};
use crate::position::{GameResult, Position, Side};
use crate::search::{Algorithm, SearchReport, perft, search};
use crate::tablebase::Tablebase;
use crate::trace::{SearchTree, TreeEdge, build_tree};
use std::fmt::Write;
use std::str::FromStr;
#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;

/// One protocol session. This is shared by the native binary and WASM.
pub struct Engine {
    position: Position,
    history: Vec<Move>,
    algorithm: Algorithm,
    tablebase: Tablebase,
    last_report: Option<SearchReport>,
    last_decision: Option<DecisionReport>,
    last_tree: Option<SearchTree>,
}

impl Default for Engine {
    fn default() -> Self {
        Self::new()
    }
}

impl Engine {
    pub fn new() -> Self {
        Self {
            position: Position::start(),
            history: Vec::new(),
            algorithm: Algorithm::default(),
            tablebase: Tablebase::build(),
            last_report: None,
            last_decision: None,
            last_tree: None,
        }
    }

    pub fn command(&mut self, line: &str) -> Vec<String> {
        let words: Vec<_> = line.split_whitespace().collect();
        let Some(command) = words.first().copied() else {
            return vec![error("empty_command", "command line is empty")];
        };

        match command {
            "gai" => self.handshake(),
            "isready" => vec!["readyok".to_owned()],
            "newgame" => {
                self.reset();
                Vec::new()
            }
            "setoption" => self.set_option(&words[1..]),
            "position" => self.set_position(&words[1..]),
            "go" => self.go(&words[1..]),
            "play" => self.play(&words[1..]),
            "tree" => self.tree(&words[1..]),
            "legal" => vec![format!(
                "legalmoves {}",
                self.position
                    .legal_moves()
                    .map(|mv| mv.to_string())
                    .collect::<Vec<_>>()
                    .join(" ")
            )],
            "eval" => {
                let solved = self.tablebase.value(self.position);
                vec![format!(
                    "evaluation score wdl {} distance {}",
                    solved.outcome.as_i8(),
                    solved.distance
                )]
            }
            "perft" => self.run_perft(&words[1..]),
            "state" => vec![format!("state {}", self.diagnostic_snapshot_json())],
            "quit" => Vec::new(),
            _ => vec![error("unknown_command", command)],
        }
    }

    pub fn snapshot_json(&self) -> String {
        self.serialize_snapshot(false)
    }

    fn diagnostic_snapshot_json(&self) -> String {
        self.serialize_snapshot(true)
    }

    fn serialize_snapshot(&self, diagnostics: bool) -> String {
        let mut json = String::with_capacity(if diagnostics { 16_384 } else { 384 });
        if diagnostics {
            json.push_str("{\"game\":\"tictactoe\",\"board\":[");
        } else {
            json.push_str("{\"board\":[");
        }
        for index in 0..9 {
            if index > 0 {
                json.push(',');
            }
            match self.position.side_at(crate::Square::new(index)) {
                Some(Side::X) => json.push_str("\"X\""),
                Some(Side::O) => json.push_str("\"O\""),
                None => json.push_str("null"),
            }
        }
        write!(
            json,
            "],\"sideToMove\":\"{}\",\"result\":\"{}\",\"winner\":{},",
            self.position.side_to_move(),
            result_name(self.position.result()),
            winner_json(self.position.result())
        )
        .expect("writing to String cannot fail");

        if diagnostics {
            write!(
                json,
                "\"stateSpace\":{{\"boardPatterns\":{},\"reachablePositions\":{},\"canonicalPositions\":{}}},",
                Position::BOARD_STATES,
                self.tablebase.reachable_positions(),
                self.tablebase.canonical_positions()
            )
            .expect("writing to String cannot fail");
        }

        json.push_str("\"winningLine\":[");
        for (index, square) in self.position.winning_squares().iter().enumerate() {
            if index > 0 {
                json.push(',');
            }
            write!(json, "\"{square}\"").expect("writing to String cannot fail");
        }
        json.push_str("],");

        json.push_str("\"legalMoves\":[");
        for (index, mv) in self.position.legal_moves().enumerate() {
            if index > 0 {
                json.push(',');
            }
            write!(json, "\"{mv}\"").expect("writing to String cannot fail");
        }
        json.push_str("],\"history\":[");
        for (index, mv) in self.history.iter().enumerate() {
            if index > 0 {
                json.push(',');
            }
            write!(json, "\"{mv}\"").expect("writing to String cannot fail");
        }
        if diagnostics {
            write!(json, "],\"algorithm\":\"{}\",\"analysis\":", self.algorithm)
                .expect("writing to String cannot fail");
            if let Some(report) = &self.last_report {
                write_report_json(&mut json, report);
            } else {
                json.push_str("null");
            }
            json.push_str(",\"decision\":");
            if let Some(decision) = &self.last_decision {
                write_decision_json(&mut json, decision);
            } else {
                json.push_str("null");
            }
            json.push_str(",\"tree\":");
            if let Some(tree) = &self.last_tree {
                write_tree_json(&mut json, tree);
            } else {
                json.push_str("null");
            }
        } else {
            json.push_str("],\"decision\":");
            if let Some(decision) = &self.last_decision {
                write!(json, "{{\"bestMove\":\"{}\"}}", decision.best_move)
                    .expect("writing to String cannot fail");
            } else {
                json.push_str("null");
            }
        }
        json.push('}');
        json
    }

    fn reset(&mut self) {
        self.position = Position::start();
        self.history.clear();
        self.last_report = None;
        self.last_decision = None;
        self.last_tree = None;
    }

    fn handshake(&self) -> Vec<String> {
        vec![
            "id name gai-tictactoe".to_owned(),
            "id author Nick Gustafson".to_owned(),
            "id game tictactoe".to_owned(),
            "option name Algorithm type combo default tablebase var plain var memo var symmetry var tablebase"
                .to_owned(),
            "extension play random tactical plain memo symmetry tablebase".to_owned(),
            "gaiok".to_owned(),
        ]
    }

    fn set_option(&mut self, words: &[&str]) -> Vec<String> {
        if words.len() != 4
            || !words[0].eq_ignore_ascii_case("name")
            || !words[1].eq_ignore_ascii_case("algorithm")
            || !words[2].eq_ignore_ascii_case("value")
        {
            return vec![error(
                "bad_setoption",
                "expected: setoption name Algorithm value <algorithm>",
            )];
        }
        match Algorithm::from_str(words[3]) {
            Ok(algorithm) => {
                self.algorithm = algorithm;
                self.last_report = None;
                self.last_decision = None;
                Vec::new()
            }
            Err(message) => vec![error("bad_option_value", message)],
        }
    }

    fn set_position(&mut self, words: &[&str]) -> Vec<String> {
        if words.first().copied() != Some("startpos") {
            return vec![error(
                "bad_position",
                "tic-tac-toe supports: position startpos [moves ...]",
            )];
        }
        let move_words = match words.get(1) {
            None => &[][..],
            Some(&"moves") => &words[2..],
            Some(_) => {
                return vec![error("bad_position", "expected 'moves' after 'startpos'")];
            }
        };

        let mut moves = Vec::with_capacity(move_words.len());
        for word in move_words {
            match Move::from_str(word) {
                Ok(mv) => moves.push(mv),
                Err(message) => return vec![error("bad_move", message)],
            }
        }
        match Position::from_moves(&moves) {
            Ok(position) => {
                self.position = position;
                self.history = moves;
                self.last_report = None;
                self.last_decision = None;
                self.last_tree = None;
                Vec::new()
            }
            Err(move_error) => vec![error("illegal_move", &move_error.to_string())],
        }
    }

    fn go(&mut self, words: &[&str]) -> Vec<String> {
        if !words.is_empty() {
            return vec![error("bad_go", "expected: go")];
        }
        if self.position.result() != GameResult::Ongoing {
            return vec![
                "info string game is already over".to_owned(),
                "bestmove (none)".to_owned(),
            ];
        }

        #[cfg(target_arch = "wasm32")]
        let started = js_sys::Date::now();
        #[cfg(not(target_arch = "wasm32"))]
        let started = Instant::now();

        let report = search(self.position, self.algorithm, &self.tablebase);

        #[cfg(target_arch = "wasm32")]
        let elapsed_millis = (js_sys::Date::now() - started).max(0.0) as u128;
        #[cfg(not(target_arch = "wasm32"))]
        let elapsed_millis = started.elapsed().as_millis();

        let millis = elapsed_millis.max(1);
        let nps = u128::from(report.stats.nodes) * 1000 / millis;
        let pv = report
            .pv
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(" ");
        let best_move = report
            .best_move
            .map_or_else(|| "(none)".to_owned(), |mv| mv.to_string());
        let lines = vec![
            format!(
                "info depth {} score wdl {} nodes {} nps {} time {} cachehits {} pv {}",
                report.distance,
                report.outcome.as_i8(),
                report.stats.nodes,
                nps,
                elapsed_millis,
                report.stats.cache_hits,
                pv
            ),
            format!("bestmove {best_move}"),
        ];
        self.last_decision = None;
        self.last_report = Some(report);
        lines
    }

    fn play(&mut self, words: &[&str]) -> Vec<String> {
        if self.position.result() != GameResult::Ongoing {
            return vec![
                "info string game is already over".to_owned(),
                "bestmove (none)".to_owned(),
            ];
        }

        let (strategy_word, random_seed) = match words {
            [strategy] => (*strategy, 0),
            [strategy, "seed", seed] => {
                let Ok(seed) = seed.parse::<u64>() else {
                    return vec![error("bad_seed", "seed must be an unsigned integer")];
                };
                (*strategy, seed)
            }
            _ => {
                return vec![error(
                    "bad_play",
                    "expected: play <strategy> [seed <unsigned integer>]",
                )];
            }
        };
        let Ok(strategy) = PlayStrategy::from_str(strategy_word) else {
            return vec![error(
                "bad_strategy",
                "strategy must be random, tactical, plain, memo, symmetry, or tablebase",
            )];
        };

        #[cfg(target_arch = "wasm32")]
        let started = js_sys::Date::now();
        #[cfg(not(target_arch = "wasm32"))]
        let started = Instant::now();

        let decision = choose_move(self.position, strategy, random_seed, &self.tablebase)
            .expect("an ongoing position has a legal move");

        #[cfg(target_arch = "wasm32")]
        let elapsed_millis = (js_sys::Date::now() - started).max(0.0) as u128;
        #[cfg(not(target_arch = "wasm32"))]
        let elapsed_millis = started.elapsed().as_millis();

        let mut info = format!(
            "info strategy {} reason {} nodes {} time {}",
            decision.strategy, decision.reason, decision.nodes, elapsed_millis
        );
        if let (Some(outcome), Some(distance)) = (decision.outcome, decision.distance) {
            write!(
                info,
                " score wdl {} distance {} cachehits {}",
                outcome.as_i8(),
                distance,
                decision.cache_hits
            )
            .expect("writing to String cannot fail");
        }
        write!(info, " pv {}", decision.best_move).expect("writing to String cannot fail");

        let best_move = decision.best_move;
        self.last_report = None;
        self.last_decision = Some(decision);
        vec![info, format!("bestmove {best_move}")]
    }

    fn tree(&mut self, words: &[&str]) -> Vec<String> {
        if words.len() != 1 {
            return vec![error("bad_tree", "expected: tree <depth 0..3>")];
        }
        let Ok(depth) = words[0].parse::<u8>() else {
            return vec![error("bad_tree", "depth must be an integer from 0 to 3")];
        };
        if depth > 3 {
            return vec![error("bad_tree", "depth must be between 0 and 3")];
        }
        let tree = build_tree(self.position, depth, &self.tablebase);
        let nodes = tree.nodes;
        self.last_tree = Some(tree);
        vec![format!("tree depth {depth} nodes {nodes}")]
    }

    fn run_perft(&self, words: &[&str]) -> Vec<String> {
        if words.len() != 1 {
            return vec![error("bad_perft", "expected: perft <depth>")];
        }
        let Ok(depth) = words[0].parse::<u8>() else {
            return vec![error("bad_perft", "depth must be an integer")];
        };
        let mut position = self.position;
        let nodes = perft(&mut position, depth);
        vec![format!("perft depth {depth} nodes {nodes}")]
    }
}

fn error(code: &str, message: &str) -> String {
    format!("error {code} {message}")
}

fn result_name(result: GameResult) -> &'static str {
    match result {
        GameResult::Ongoing => "ongoing",
        GameResult::Draw => "draw",
        GameResult::Win(_) => "win",
    }
}

fn winner_json(result: GameResult) -> &'static str {
    match result {
        GameResult::Win(Side::X) => "\"X\"",
        GameResult::Win(Side::O) => "\"O\"",
        GameResult::Ongoing | GameResult::Draw => "null",
    }
}

fn write_report_json(json: &mut String, report: &SearchReport) {
    write!(
        json,
        "{{\"algorithm\":\"{}\",\"bestMove\":{},\"outcome\":\"{}\",\"distance\":{},\"nodes\":{},\"cacheHits\":{},\"pv\":[",
        report.algorithm,
        report
            .best_move
            .map_or_else(|| "null".to_owned(), |mv| format!("\"{mv}\"")),
        report.outcome,
        report.distance,
        report.stats.nodes,
        report.stats.cache_hits
    )
    .expect("writing to String cannot fail");
    for (index, mv) in report.pv.iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
        write!(json, "\"{mv}\"").expect("writing to String cannot fail");
    }
    json.push_str("],\"candidates\":[");
    for (index, candidate) in report.candidates.iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
        write!(
            json,
            "{{\"move\":\"{}\",\"outcome\":\"{}\",\"distance\":{},\"nodes\":{}}}",
            candidate.mv, candidate.outcome, candidate.distance, candidate.nodes
        )
        .expect("writing to String cannot fail");
    }
    json.push_str("]}");
}

fn write_decision_json(json: &mut String, decision: &DecisionReport) {
    write!(
        json,
        "{{\"strategy\":\"{}\",\"bestMove\":\"{}\",\"reason\":\"{}\",\"nodes\":{},\"cacheHits\":{},\"outcome\":{},\"distance\":{}}}",
        decision.strategy,
        decision.best_move,
        decision.reason,
        decision.nodes,
        decision.cache_hits,
        decision
            .outcome
            .map_or_else(|| "null".to_owned(), |outcome| format!("\"{outcome}\"")),
        decision
            .distance
            .map_or_else(|| "null".to_owned(), |distance| distance.to_string())
    )
    .expect("writing to String cannot fail");
}

fn write_tree_json(json: &mut String, tree: &SearchTree) {
    write!(
        json,
        "{{\"depth\":{},\"nodes\":{},\"sideToMove\":\"{}\",\"outcome\":\"{}\",\"distance\":{},\"children\":[",
        tree.depth, tree.nodes, tree.side_to_move, tree.outcome, tree.distance
    )
    .expect("writing to String cannot fail");
    write_tree_edges(json, &tree.children);
    json.push_str("]}");
}

fn write_tree_edges(json: &mut String, edges: &[TreeEdge]) {
    for (index, edge) in edges.iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
        write!(
            json,
            "{{\"move\":\"{}\",\"outcome\":\"{}\",\"distance\":{},\"canonicalKey\":{},\"children\":[",
            edge.mv, edge.outcome, edge.distance, edge.canonical_key
        )
        .expect("writing to String cannot fail");
        write_tree_edges(json, &edge.children);
        json.push_str("]}");
    }
}
