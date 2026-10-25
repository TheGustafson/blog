use crate::{
    CELLS, Color, GameResult, MctsOptions, MctsReport, MctsSearcher, Move, Position, SIZE,
    Strategy, perft,
};
use std::fmt::Write;
use std::str::FromStr;

/// Stateful text-protocol session shared by native tools, tests, and the WebAssembly worker.
pub struct Engine {
    position: Position,
    history: Vec<Move>,
    searcher: MctsSearcher,
    last_decision: Option<MctsReport>,
}

impl Default for Engine {
    fn default() -> Self {
        Self::new()
    }
}

impl Engine {
    /// Creates a new protocol engine at the starting position.
    pub fn new() -> Self {
        Self {
            position: Position::start(),
            history: Vec::new(),
            searcher: MctsSearcher::new(),
            last_decision: None,
        }
    }

    /// Returns the current [`Position`].
    pub const fn position(&self) -> Position {
        self.position
    }

    /// Returns the move history played since the start of the game.
    pub fn history(&self) -> &[Move] {
        &self.history
    }

    /// Executes one line-oriented command and returns any response lines.
    pub fn command(&mut self, line: &str) -> Vec<String> {
        let words: Vec<_> = line.split_whitespace().collect();
        let Some(cmd) = words.first().copied() else {
            return vec![error("empty command")];
        };
        match cmd {
            "gai" => vec![
                "id name ai-crosshair".to_owned(),
                "id author Nick Gustafson".to_owned(),
                "id game crosshair".to_owned(),
                "gaiok".to_owned(),
            ],
            "isready" => vec!["readyok".to_owned()],
            "newgame" => {
                if words.len() > 1 {
                    return vec![error("newgame takes no arguments")];
                }
                self.position = Position::start();
                self.history.clear();
                self.last_decision = None;
                Vec::new()
            }
            "position" => self.set_position(&words[1..]),
            "play" => self.play_move(&words[1..]),
            "search" => self.search(&words[1..]),
            "legal" => vec![format!(
                "legalmoves {}",
                self.position
                    .legal_moves()
                    .iter()
                    .map(|mv| mv.to_string())
                    .collect::<Vec<_>>()
                    .join(" ")
            )],
            "perft" => self.run_perft(&words[1..]),
            "state" => vec![format!("state {}", self.snapshot_json())],
            "quit" => Vec::new(),
            _ => vec![error(&format!("unknown command: {cmd}"))],
        }
    }

    /// Serializes the complete board state, previews, and last decision to JSON.
    pub fn snapshot_json(&self) -> String {
        let mut json = String::with_capacity(4_096);
        write!(json, "{{\"size\":{SIZE},\"board\":[").expect("writing to String cannot fail");
        for idx in 0..CELLS as u8 {
            if idx > 0 {
                json.push(',');
            }
            match self.position.color_at(Move::from_index(idx)) {
                Some(Color::Red) => json.push_str("\"R\""),
                Some(Color::Blue) => json.push_str("\"B\""),
                None => json.push_str("null"),
            }
        }
        let (red_count, blue_count) = self.position.counts();
        write!(
            json,
            "],\"sideToMove\":\"{}\",\"ply\":{},\"counts\":{{\"red\":{},\"blue\":{}}},\"activeRow\":{},\"activeCol\":{},\"wildcard\":{},\"result\":\"{}\",\"winner\":{},\"winReason\":{},",
            self.position.side_to_move(),
            self.position.ply(),
            red_count,
            blue_count,
            optional_u8_json(self.position.active_row()),
            optional_u8_json(self.position.active_col()),
            self.position.is_wildcard(),
            result_name(self.position.result()),
            winner_json(self.position.result()),
            win_reason_json(self.position.result()),
        )
        .expect("writing to String cannot fail");

        json.push_str("\"winningLine\":[");
        write_move_iter(&mut json, self.position.winning_line().iter());
        json.push_str("],\"legalMoves\":[");
        write_move_iter(&mut json, self.position.legal_moves().iter());
        json.push_str("],\"movePreviews\":[");
        for (i, preview) in self.position.move_previews().iter().enumerate() {
            if i > 0 {
                json.push(',');
            }
            write!(json, "{{\"move\":\"{}\",\"flips\":[", preview.mv)
                .expect("writing to String cannot fail");
            write_move_iter(&mut json, preview.flips.iter());
            write!(
                json,
                "],\"nextActiveRow\":{},\"nextActiveCol\":{},\"nextWildcard\":{},\"immediateWin\":{}}}",
                optional_u8_json(preview.next_active_row),
                optional_u8_json(preview.next_active_col),
                preview.next_wildcard,
                preview.immediate_win,
            )
            .expect("writing to String cannot fail");
        }
        json.push_str("],\"history\":[");
        write_move_iter(&mut json, self.history.iter().copied());
        json.push_str("],\"lastMove\":");
        if let Some(last) = self.position.last_move() {
            write!(json, "\"{last}\"").expect("writing to String cannot fail");
        } else {
            json.push_str("null");
        }
        json.push_str(",\"lastFlips\":[");
        write_move_iter(&mut json, self.position.last_flips().iter());
        json.push_str("],\"decision\":");
        if let Some(report) = &self.last_decision {
            write_decision_json(&mut json, report);
        } else {
            json.push_str("null");
        }
        json.push('}');
        json
    }

    fn set_position(&mut self, words: &[&str]) -> Vec<String> {
        if words.first().copied() != Some("startpos") {
            return vec![error("expected: position startpos [moves ...]")];
        }
        let move_words = match words.get(1) {
            None => &[][..],
            Some(&"moves") => &words[2..],
            Some(_) => return vec![error("expected 'moves' after 'startpos'")],
        };
        let mut moves = Vec::with_capacity(move_words.len());
        for word in move_words {
            match Move::from_str(word) {
                Ok(mv) => moves.push(mv),
                Err(err) => return vec![error(&err.to_string())],
            }
        }
        match Position::from_moves(&moves) {
            Ok(position) => {
                self.position = position;
                self.history = moves;
                self.last_decision = None;
                Vec::new()
            }
            Err(err) => vec![error(&err.to_string())],
        }
    }

    fn play_move(&mut self, words: &[&str]) -> Vec<String> {
        if words.len() != 1 {
            return vec![error("expected: play <coord>")];
        }
        let mv = match Move::from_str(words[0]) {
            Ok(mv) => mv,
            Err(err) => return vec![error(&err.to_string())],
        };
        match self.position.play(mv) {
            Ok(next) => {
                self.position = next;
                self.history.push(mv);
                self.last_decision = None;
                Vec::new()
            }
            Err(err) => vec![error(&err.to_string())],
        }
    }

    fn search(&mut self, words: &[&str]) -> Vec<String> {
        let mut options = MctsOptions::default();
        let mut index = 0;
        while index < words.len() {
            let Some(value) = words.get(index + 1) else {
                return vec![error("search options need a value")];
            };
            match words[index] {
                "strategy" => match Strategy::from_str(value) {
                    Ok(strategy) => options.strategy = strategy,
                    Err(msg) => return vec![error(msg)],
                },
                "simulations" => match value.parse::<u32>() {
                    Ok(simulations @ 1..=1_000_000) => options.max_simulations = simulations,
                    _ => return vec![error("simulations must be from 1 through 1000000")],
                },
                "depth" => match value.parse::<u8>() {
                    Ok(depth @ 1..=20) => options.alpha_beta_depth = depth,
                    _ => return vec![error("depth must be from 1 through 20")],
                },
                "nodes" => match value.parse::<u64>() {
                    Ok(nodes @ 1..=10_000_000) => options.alpha_beta_nodes = nodes,
                    _ => return vec![error("nodes must be from 1 through 10000000")],
                },
                "softtime" => match value.parse::<u32>() {
                    Ok(time @ 0..=10_000) => options.soft_time_ms = time,
                    _ => return vec![error("softtime must be from 0 through 10000 milliseconds")],
                },
                "seed" => match value.parse::<u64>() {
                    Ok(seed) => options.seed = seed,
                    _ => return vec![error("seed must be an unsigned 64-bit number")],
                },
                "exploration" => match value.parse::<f64>() {
                    Ok(exploration)
                        if exploration.is_finite() && (0.0..=4.0).contains(&exploration) =>
                    {
                        options.exploration = exploration;
                    }
                    _ => return vec![error("exploration must be from 0 through 4")],
                },
                other => {
                    return vec![error(&format!(
                        "unsupported search option '{other}': expected strategy, simulations, depth, nodes, softtime, seed, or exploration"
                    ))];
                }
            }
            index += 2;
        }

        let report = self.searcher.search(self.position, options);
        let best = report
            .best_move
            .map_or_else(|| "none".to_owned(), |mv| mv.to_string());
        let output = format!(
            "bestmove {best} strategy {} simulations {} depth {} nodes {} score {:.4}",
            report.strategy,
            report.simulations,
            report.depth,
            report.tree_nodes,
            report.expected_score,
        );
        self.last_decision = Some(report);
        vec![output]
    }

    fn run_perft(&self, words: &[&str]) -> Vec<String> {
        if words.len() != 1 {
            return vec![error("expected: perft <depth>")];
        }
        let Ok(depth) = words[0].parse::<u8>() else {
            return vec![error("perft depth must be an integer")];
        };
        if depth > 6 {
            return vec![error("perft depth must be from 0 through 6")];
        }
        vec![format!(
            "perft depth {depth} nodes {}",
            perft(self.position, depth)
        )]
    }
}

fn write_decision_json(json: &mut String, report: &MctsReport) {
    let best_move_json = report
        .best_move
        .map_or_else(|| "null".to_owned(), |mv| format!("\"{mv}\""));
    let net_val_json = report
        .network_value
        .map_or_else(|| "null".to_owned(), |val| format!("{val:.4}"));
    write!(
        json,
        "{{\"strategy\":\"{}\",\"bestMove\":{},\"simulations\":{},\"treeNodes\":{},\"rootVisits\":{},\"rolloutMoves\":{},\"leafEvaluations\":{},\"depth\":{},\"expectedScore\":{:.4},\"networkValue\":{},\"elapsedMs\":{:.1},\"rootMoves\":[",
        report.strategy,
        best_move_json,
        report.simulations,
        report.tree_nodes,
        report.root_visits,
        report.rollout_moves,
        report.leaf_evaluations,
        report.depth,
        report.expected_score,
        net_val_json,
        report.elapsed_ms,
    )
    .expect("writing to String cannot fail");
    for (i, stats) in report.root_moves.iter().enumerate() {
        if i > 0 {
            json.push(',');
        }
        write!(
            json,
            "{{\"move\":\"{}\",\"visits\":{},\"prior\":{:.4},\"expectedScore\":{:.4}}}",
            stats.mv, stats.visits, stats.prior, stats.expected_score,
        )
        .expect("writing to String cannot fail");
    }
    json.push_str("]}");
}

fn write_move_iter(json: &mut String, iter: impl Iterator<Item = Move>) {
    for (i, mv) in iter.enumerate() {
        if i > 0 {
            json.push(',');
        }
        write!(json, "\"{mv}\"").expect("writing to String cannot fail");
    }
}

fn optional_u8_json(value: Option<u8>) -> String {
    value.map_or_else(|| "null".to_owned(), |v| v.to_string())
}

const fn result_name(result: GameResult) -> &'static str {
    match result {
        GameResult::Ongoing => "ongoing",
        GameResult::Win { .. } => "win",
    }
}

const fn winner_json(result: GameResult) -> &'static str {
    match result {
        GameResult::Win {
            winner: Color::Red, ..
        } => "\"R\"",
        GameResult::Win {
            winner: Color::Blue,
            ..
        } => "\"B\"",
        GameResult::Ongoing => "null",
    }
}

const fn win_reason_json(result: GameResult) -> &'static str {
    match result {
        GameResult::Win { reason, .. } => match reason {
            crate::WinReason::Connection => "\"connection\"",
        },
        GameResult::Ongoing => "null",
    }
}

fn error(reason: &str) -> String {
    format!("error message {reason}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protocol_commands_and_snapshot_match_contract() {
        let mut engine = Engine::new();
        assert_eq!(engine.command("isready"), vec!["readyok"]);

        assert!(engine.command("position startpos moves a1 d1").is_empty());
        let snap: serde_json::Value = serde_json::from_str(&engine.snapshot_json()).unwrap();
        assert_eq!(snap["size"], 7);
        assert_eq!(snap["sideToMove"], "R");
        assert_eq!(snap["ply"], 2);
        assert_eq!(snap["counts"]["red"], 1);
        assert_eq!(snap["counts"]["blue"], 1);
        assert_eq!(snap["activeRow"], 0);
        assert_eq!(snap["activeCol"], 3);
        assert_eq!(snap["wildcard"], false);
        assert_eq!(snap["lastMove"], "d1");
        assert_eq!(snap["history"], serde_json::json!(["a1", "d1"]));

        // Invalid play does not mutate state
        let err = engine.command("play g7");
        assert!(err[0].starts_with("error message "));
        assert_eq!(engine.position().ply(), 2);

        // Legal play advances state
        assert!(engine.command("play b1").is_empty());
        assert_eq!(engine.position().ply(), 3);

        // Search populates decision
        let search_out =
            engine.command("search strategy az-puct simulations 40 seed 11 softtime 0");
        assert!(search_out[0].starts_with("bestmove "));
        let snap2: serde_json::Value = serde_json::from_str(&engine.snapshot_json()).unwrap();
        assert_eq!(snap2["decision"]["strategy"], "az-puct");
        assert_eq!(snap2["decision"]["simulations"], 40);
        assert!(snap2["decision"]["networkValue"].is_number());
    }
}
