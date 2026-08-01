use crate::{
    GameResult, MiniResult, Move, Player, Position, SearchOptions, SearchReport, Searcher,
};
#[cfg(feature = "mcts")]
use crate::{MctsOptions, MctsReport, MctsSearcher, MctsStrategy};
use std::fmt::Write;
use std::str::FromStr;

/// A stateful text-protocol session shared by native tests and the WASM worker.
pub struct Engine {
    position: Position,
    history: Vec<Move>,
    searcher: Searcher,
    #[cfg(feature = "mcts")]
    mcts_searcher: MctsSearcher,
    last_decision: Option<Decision>,
}

enum Decision {
    AlphaBeta(SearchReport),
    #[cfg(feature = "mcts")]
    Mcts(MctsReport),
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
            searcher: Searcher::new(),
            #[cfg(feature = "mcts")]
            mcts_searcher: MctsSearcher::new(),
            last_decision: None,
        }
    }

    pub fn command(&mut self, line: &str) -> Vec<String> {
        let words: Vec<_> = line.split_whitespace().collect();
        let Some(command) = words.first().copied() else {
            return vec![error("empty command")];
        };
        match command {
            "gai" => vec![
                "id name ai-ultimate-tictactoe".to_owned(),
                "id game ultimate-tictactoe".to_owned(),
                "gaiok".to_owned(),
            ],
            "isready" => vec!["readyok".to_owned()],
            "newgame" => {
                self.position = Position::start();
                self.history.clear();
                self.last_decision = None;
                Vec::new()
            }
            "position" => self.set_position(&words[1..]),
            "play" => self.search(&words[1..]),
            #[cfg(feature = "mcts")]
            "mcts" => self.search_mcts(&words[1..]),
            "legal" => vec![format!(
                "legalmoves {}",
                self.position
                    .legal_moves()
                    .iter()
                    .map(|mv| mv.to_string())
                    .collect::<Vec<_>>()
                    .join(" ")
            )],
            "state" => vec![format!("state {}", self.snapshot_json())],
            "quit" => Vec::new(),
            _ => vec![error("unknown command")],
        }
    }

    pub fn snapshot_json(&self) -> String {
        let mut json = String::with_capacity(2_048);
        json.push_str("{\"board\":[");
        for global in 0..81 {
            if global > 0 {
                json.push(',');
            }
            match self.position.player_at(Move::from_global_index(global)) {
                Some(Player::X) => json.push_str("\"X\""),
                Some(Player::O) => json.push_str("\"O\""),
                None => json.push_str("null"),
            }
        }
        json.push_str("],\"miniBoards\":[");
        for board in 0..9 {
            if board > 0 {
                json.push(',');
            }
            match self.position.mini_result(board) {
                MiniResult::Open => json.push_str("null"),
                MiniResult::Draw => json.push_str("\"draw\""),
                MiniResult::Win(Player::X) => json.push_str("\"X\""),
                MiniResult::Win(Player::O) => json.push_str("\"O\""),
            }
        }
        write!(
            json,
            "],\"sideToMove\":\"{}\",\"activeBoard\":{},\"result\":\"{}\",\"winner\":{},",
            self.position.side_to_move(),
            self.position
                .active_board()
                .map_or_else(|| "null".to_owned(), |board| board.to_string()),
            result_name(self.position.result()),
            winner_json(self.position.result()),
        )
        .expect("writing to String cannot fail");

        json.push_str("\"macroWinningLine\":[");
        if let Some(line) = self.position.macro_winning_line() {
            let mut first = true;
            for board in 0..9 {
                if line & (1 << board) == 0 {
                    continue;
                }
                if !first {
                    json.push(',');
                }
                first = false;
                write!(json, "{board}").expect("writing to String cannot fail");
            }
        }
        json.push_str("],\"legalMoves\":[");
        for (index, mv) in self.position.legal_moves().iter().enumerate() {
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
        json.push_str("],\"lastMove\":");
        if let Some(last) = self.history.last() {
            write!(json, "\"{last}\"").expect("writing to String cannot fail");
        } else {
            json.push_str("null");
        }
        json.push_str(",\"decision\":");
        match &self.last_decision {
            Some(Decision::AlphaBeta(report)) => {
                write!(
                    json,
                    "{{\"bestMove\":{},\"algorithm\":\"alpha-beta\",\"depth\":{},\"score\":{},\"nodes\":{}}}",
                    report
                        .best_move
                        .map_or_else(|| "null".to_owned(), |mv| format!("\"{mv}\"")),
                    report.depth,
                    report.score,
                    report.nodes,
                )
                .expect("writing to String cannot fail");
            }
            #[cfg(feature = "mcts")]
            Some(Decision::Mcts(report)) => write_mcts_report(&mut json, report),
            None => json.push_str("null"),
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
                Err(message) => return vec![error(&message.to_string())],
            }
        }
        match Position::from_moves(&moves) {
            Ok(position) => {
                self.position = position;
                self.history = moves;
                self.last_decision = None;
                Vec::new()
            }
            Err(message) => vec![error(&message.to_string())],
        }
    }

    fn search(&mut self, words: &[&str]) -> Vec<String> {
        let mut options = SearchOptions::default();
        let mut index = 0;
        while index < words.len() {
            let Some(value) = words.get(index + 1) else {
                return vec![error("play options need a value")];
            };
            match words[index] {
                "depth" => match value.parse::<u8>() {
                    Ok(depth @ 1..=20) => options.max_depth = depth,
                    _ => return vec![error("depth must be from 1 through 20")],
                },
                "nodes" => match value.parse::<u64>() {
                    Ok(nodes @ 1..=10_000_000) => options.node_limit = nodes,
                    _ => return vec![error("nodes must be from 1 through 10000000")],
                },
                "softtime" => match value.parse::<u32>() {
                    Ok(time @ 1..=1_000) => options.soft_time_ms = time,
                    _ => return vec![error("softtime must be from 1 through 1000 milliseconds")],
                },
                _ => return vec![error("play supports depth, nodes, and softtime")],
            }
            index += 2;
        }
        let report = self.searcher.search(self.position, options);
        let best = report
            .best_move
            .map_or_else(|| "none".to_owned(), |mv| mv.to_string());
        let output = format!(
            "bestmove {best} depth {} score {} nodes {}",
            report.depth, report.score, report.nodes
        );
        self.last_decision = Some(Decision::AlphaBeta(report));
        vec![output]
    }

    #[cfg(feature = "mcts")]
    fn search_mcts(&mut self, words: &[&str]) -> Vec<String> {
        let mut options = MctsOptions::default();
        let mut index = 0;
        while index < words.len() {
            let Some(value) = words.get(index + 1) else {
                return vec![error("mcts options need a value")];
            };
            match words[index] {
                "simulations" => match value.parse::<u32>() {
                    Ok(simulations @ 1..=1_000_000) => {
                        options.max_simulations = simulations;
                    }
                    _ => return vec![error("simulations must be from 1 through 1000000")],
                },
                "softtime" => match value.parse::<u32>() {
                    Ok(time @ 1..=1_000) => options.soft_time_ms = time,
                    _ => return vec![error("softtime must be from 1 through 1000 milliseconds")],
                },
                "exploration" => match value.parse::<f64>() {
                    Ok(exploration)
                        if exploration.is_finite() && (0.0..=4.0).contains(&exploration) =>
                    {
                        options.exploration = exploration;
                    }
                    _ => return vec![error("exploration must be from 0 through 4")],
                },
                "seed" => match value.parse::<u64>() {
                    Ok(seed) => options.seed = seed,
                    _ => return vec![error("seed must be an unsigned 64-bit number")],
                },
                "strategy" => match *value {
                    "random-uct" => options.strategy = MctsStrategy::UctRandom,
                    "tactical-uct" => options.strategy = MctsStrategy::UctTactical,
                    "handcrafted-puct" => options.strategy = MctsStrategy::PuctHandcrafted,
                    "learned-puct" => options.strategy = MctsStrategy::PuctLearned,
                    _ => {
                        return vec![error(
                            "strategy must be random-uct, tactical-uct, handcrafted-puct, or learned-puct",
                        )];
                    }
                },
                _ => {
                    return vec![error(
                        "mcts supports simulations, softtime, exploration, seed, and strategy",
                    )];
                }
            }
            index += 2;
        }

        let report = self.mcts_searcher.search(self.position, options);
        let best = report
            .best_move
            .map_or_else(|| "none".to_owned(), |mv| mv.to_string());
        let output = format!(
            "bestmove {best} simulations {} nodes {} score {:.4} strategy {}",
            report.simulations,
            report.tree_nodes,
            report.expected_score,
            report.strategy.name(),
        );
        self.last_decision = Some(Decision::Mcts(report));
        vec![output]
    }
}

#[cfg(feature = "mcts")]
fn write_mcts_report(json: &mut String, report: &MctsReport) {
    write!(
        json,
        "{{\"bestMove\":{},\"algorithm\":\"mcts\",\"strategy\":\"{}\",\"simulations\":{},\"treeNodes\":{},\"rootVisits\":{},\"rolloutMoves\":{},\"leafEvaluations\":{},\"expectedScore\":{:.4},\"elapsedMs\":{},\"rootMoves\":[",
        report
            .best_move
            .map_or_else(|| "null".to_owned(), |mv| format!("\"{mv}\"")),
        report.strategy.name(),
        report.simulations,
        report.tree_nodes,
        report.root_visits,
        report.rollout_moves,
        report.leaf_evaluations,
        report.expected_score,
        report.elapsed_ms,
    )
    .expect("writing to String cannot fail");
    for (index, stats) in report.root_moves.iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
        write!(
            json,
            "{{\"move\":\"{}\",\"visits\":{},\"prior\":{:.6},\"expectedScore\":{:.4}}}",
            stats.mv, stats.visits, stats.prior, stats.expected_score,
        )
        .expect("writing to String cannot fail");
    }
    json.push_str("]}");
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
        GameResult::Win(Player::X) => "\"X\"",
        GameResult::Win(Player::O) => "\"O\"",
        _ => "null",
    }
}

fn error(message: &str) -> String {
    format!("info string error {message}")
}
