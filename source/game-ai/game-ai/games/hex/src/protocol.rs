use crate::{
    BoardSize, Color, GameResult, KnowledgePolicy, MctsOptions, MctsReport, MctsSearcher,
    MctsStrategy, Move, Position, RolloutPolicy, Seat, SwapRule,
};
use std::fmt::Write;
use std::str::FromStr;

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
    pub fn new() -> Self {
        Self {
            position: Position::new(BoardSize::default(), SwapRule::Enabled),
            history: Vec::new(),
            searcher: MctsSearcher::new(),
            last_decision: None,
        }
    }

    pub fn command(&mut self, line: &str) -> Vec<String> {
        let words = line.split_whitespace().collect::<Vec<_>>();
        let Some(command) = words.first().copied() else {
            return vec![error("empty command")];
        };
        match command {
            "gai" => vec![
                "id name ai-hex".to_owned(),
                "id game hex".to_owned(),
                "gaiok".to_owned(),
            ],
            "isready" => vec!["readyok".to_owned()],
            "newgame" => self.new_game(&words[1..]),
            "position" => self.set_position(&words[1..]),
            "mcts" => self.search(&words[1..]),
            "legal" => vec![format!(
                "legalmoves {}",
                self.position
                    .legal_moves()
                    .iter()
                    .map(Move::to_string)
                    .collect::<Vec<_>>()
                    .join(" ")
            )],
            "state" => vec![format!("state {}", self.snapshot_json())],
            "quit" => Vec::new(),
            _ => vec![error("unknown command")],
        }
    }

    pub fn snapshot_json(&self) -> String {
        let mut json = String::with_capacity(8_192);
        write!(
            json,
            "{{\"size\":{},\"board\":[",
            self.position.size().get()
        )
        .expect("writing to String cannot fail");
        for dense in 0..self.position.size().cell_count() {
            if dense > 0 {
                json.push(',');
            }
            let cell = crate::Cell::from_dense(dense, self.position.size());
            match self.position.color_at(cell) {
                Some(Color::Red) => json.push_str("\"R\""),
                Some(Color::Blue) => json.push_str("\"B\""),
                None => json.push_str("null"),
            }
        }
        write!(
            json,
            "],\"seatToMove\":\"{}\",\"colorToMove\":\"{}\",\"seatColors\":[\"{}\",\"{}\"],\"colorsSwapped\":{},\"swapAvailable\":{},\"result\":\"{}\",\"winnerSeat\":{},\"winnerColor\":{},",
            self.position.seat_to_move().as_str(),
            color_code(self.position.color_to_move()),
            color_code(self.position.color_for_seat(Seat::One)),
            color_code(self.position.color_for_seat(Seat::Two)),
            self.position.colors_swapped(),
            self.position.swap_available(),
            result_name(self.position.result()),
            winner_seat_json(self.position.result()),
            winner_color_json(self.position),
        )
        .expect("writing to String cannot fail");

        json.push_str("\"winningPath\":[");
        for (index, cell) in self.position.winning_path().iter().enumerate() {
            if index > 0 {
                json.push(',');
            }
            write!(json, "\"{cell}\"").expect("writing to String cannot fail");
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
            Some(report) => write_report(&mut json, report),
            None => json.push_str("null"),
        }
        json.push('}');
        json
    }

    fn new_game(&mut self, words: &[&str]) -> Vec<String> {
        match parse_setup(words) {
            Ok((size, rule, consumed)) if consumed == words.len() => {
                self.position = Position::new(size, rule);
                self.history.clear();
                self.last_decision = None;
                Vec::new()
            }
            Ok(_) => vec![error("newgame supports size and swap")],
            Err(message) => vec![error(message)],
        }
    }

    fn set_position(&mut self, words: &[&str]) -> Vec<String> {
        let (size, rule, consumed) = match parse_setup(words) {
            Ok(setup) => setup,
            Err(message) => return vec![error(message)],
        };
        let remaining = &words[consumed..];
        let move_words = match remaining.first() {
            None => &[][..],
            Some(&"moves") => &remaining[1..],
            Some(_) => return vec![error("expected 'moves' after position setup")],
        };
        let moves = match move_words
            .iter()
            .map(|word| Move::from_str(word))
            .collect::<Result<Vec<_>, _>>()
        {
            Ok(moves) => moves,
            Err(message) => return vec![error(&message.to_string())],
        };
        match Position::from_moves(size, rule, &moves) {
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
        let mut options = MctsOptions::default();
        let mut index = 0;
        while index < words.len() {
            let Some(value) = words.get(index + 1) else {
                return vec![error("mcts options need a value")];
            };
            match words[index] {
                "simulations" => match value.parse::<u32>() {
                    Ok(simulations @ 1..=1_000_000) => options.max_simulations = simulations,
                    _ => return vec![error("simulations must be from 1 through 1000000")],
                },
                "softtime" => match value.parse::<u32>() {
                    Ok(time @ 0..=2_000) => options.soft_time_ms = time,
                    _ => return vec![error("softtime must be from 0 through 2000 milliseconds")],
                },
                "exploration" => match value.parse::<f64>() {
                    Ok(exploration)
                        if exploration.is_finite() && (0.0..=4.0).contains(&exploration) =>
                    {
                        options.exploration = exploration;
                    }
                    _ => return vec![error("exploration must be from 0 through 4")],
                },
                "strategy" => match *value {
                    "plain-uct" => options.strategy = MctsStrategy::PlainUct,
                    "uct-rave" => options.strategy = MctsStrategy::UctRave,
                    _ => return vec![error("strategy must be plain-uct or uct-rave")],
                },
                "rave" => match value.parse::<f64>() {
                    Ok(equivalence)
                        if equivalence.is_finite() && (1.0..=100_000.0).contains(&equivalence) =>
                    {
                        options.rave_equivalence = equivalence;
                    }
                    _ => return vec![error("rave must be from 1 through 100000")],
                },
                "rollout" => match *value {
                    "random" => options.rollout_policy = RolloutPolicy::Random,
                    "save-bridge" => options.rollout_policy = RolloutPolicy::SaveBridge,
                    _ => return vec![error("rollout must be random or save-bridge")],
                },
                "knowledge" => match *value {
                    "off" => options.knowledge_policy = KnowledgePolicy::Disabled,
                    value => match value.parse::<u32>() {
                        Ok(min_visits @ 0..=1_000_000) => {
                            options.knowledge_policy =
                                KnowledgePolicy::InferiorCells { min_visits };
                        }
                        _ => return vec![error("knowledge must be off or from 0 through 1000000")],
                    },
                },
                "connections" => match *value {
                    "on" => options.use_virtual_connections = true,
                    "off" => options.use_virtual_connections = false,
                    _ => return vec![error("connections must be on or off")],
                },
                "seed" => match value.parse::<u64>() {
                    Ok(seed) => options.seed = seed,
                    _ => return vec![error("seed must be an unsigned 64-bit number")],
                },
                _ => {
                    return vec![error(
                        "mcts supports simulations, softtime, exploration, strategy, rave, rollout, knowledge, connections, and seed",
                    )];
                }
            }
            index += 2;
        }
        let report = self.searcher.search(self.position, options);
        let best = report
            .best_move
            .map_or_else(|| "none".to_owned(), |mv| mv.to_string());
        let output = format!(
            "bestmove {best} simulations {} nodes {} score {:.4}",
            report.simulations, report.tree_nodes, report.expected_score,
        );
        self.last_decision = Some(report);
        vec![output]
    }
}

fn parse_setup(words: &[&str]) -> Result<(BoardSize, SwapRule, usize), &'static str> {
    let mut size = BoardSize::default();
    let mut swap = SwapRule::Enabled;
    let mut index = 0;
    while index < words.len() {
        match words[index] {
            "moves" => break,
            "size" => {
                let Some(value) = words.get(index + 1) else {
                    return Err("size needs a value");
                };
                size = value
                    .parse::<u8>()
                    .ok()
                    .and_then(|value| BoardSize::new(value).ok())
                    .ok_or("size must be from 9 through 24")?;
            }
            "swap" => {
                let Some(value) = words.get(index + 1) else {
                    return Err("swap needs on or off");
                };
                swap = match *value {
                    "on" => SwapRule::Enabled,
                    "off" => SwapRule::Disabled,
                    _ => return Err("swap must be on or off"),
                };
            }
            _ => return Err("setup supports size and swap"),
        }
        index += 2;
    }
    Ok((size, swap, index))
}

fn write_report(json: &mut String, report: &MctsReport) {
    write!(
        json,
        "{{\"bestMove\":{},\"algorithm\":\"uct\",\"strategy\":\"{}\",\"raveEquivalence\":{:.1},\"rolloutPolicy\":\"{}\",\"knowledgeThreshold\":{},\"virtualConnectionsEnabled\":{},\"simulations\":{},\"treeNodes\":{},\"rootVisits\":{},\"rolloutMoves\":{},\"bridgeReplies\":{},\"knowledgeNodes\":{},\"prunedMoves\":{},\"mustPlayNodes\":{},\"rootPrunedMoves\":{},\"rootMustPlayMoves\":{},\"virtualConnections\":{},\"semiConnections\":{},\"connectionSearchTruncatedNodes\":{},\"provenNodes\":{},\"solverPropagations\":{},\"provenWinner\":{},\"proofDistance\":{},\"expectedScore\":{:.4},\"elapsedMs\":{},\"rootMoves\":[",
        report
            .best_move
            .map_or_else(|| "null".to_owned(), |mv| format!("\"{mv}\"")),
        report.strategy.as_str(),
        report.rave_equivalence,
        report.rollout_policy.as_str(),
        report
            .knowledge_policy
            .min_visits()
            .map_or_else(|| "null".to_owned(), |visits| visits.to_string()),
        report.virtual_connections_enabled,
        report.simulations,
        report.tree_nodes,
        report.root_visits,
        report.rollout_moves,
        report.bridge_replies,
        report.knowledge_nodes,
        report.pruned_moves,
        report.must_play_nodes,
        report.root_pruned_moves,
        report.root_must_play_moves,
        report.virtual_connections,
        report.semi_connections,
        report.connection_search_truncated_nodes,
        report.proven_nodes,
        report.solver_propagations,
        optional_seat_json(report.proven_winner),
        report
            .proof_distance
            .map_or_else(|| "null".to_owned(), |distance| distance.to_string()),
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
            "{{\"move\":\"{}\",\"visits\":{},\"expectedScore\":{:.4},\"raveVisits\":{},\"raveExpectedScore\":{:.4},\"provenWinner\":{},\"proofDistance\":{}}}",
            stats.mv,
            stats.visits,
            stats.expected_score,
            stats.rave_visits,
            stats.rave_expected_score,
            optional_seat_json(stats.proven_winner),
            stats
                .proof_distance
                .map_or_else(|| "null".to_owned(), |distance| distance.to_string()),
        )
        .expect("writing to String cannot fail");
    }
    json.push_str("]}");
}

fn result_name(result: GameResult) -> &'static str {
    match result {
        GameResult::Ongoing => "ongoing",
        GameResult::Win(_) => "win",
    }
}

fn winner_seat_json(result: GameResult) -> &'static str {
    match result {
        GameResult::Win(Seat::One) => "\"one\"",
        GameResult::Win(Seat::Two) => "\"two\"",
        GameResult::Ongoing => "null",
    }
}

fn optional_seat_json(seat: Option<Seat>) -> &'static str {
    match seat {
        Some(Seat::One) => "\"one\"",
        Some(Seat::Two) => "\"two\"",
        None => "null",
    }
}

fn winner_color_json(position: Position) -> &'static str {
    match position.result() {
        GameResult::Win(seat) => match position.color_for_seat(seat) {
            Color::Red => "\"R\"",
            Color::Blue => "\"B\"",
        },
        GameResult::Ongoing => "null",
    }
}

const fn color_code(color: Color) -> &'static str {
    match color {
        Color::Red => "R",
        Color::Blue => "B",
    }
}

fn error(message: &str) -> String {
    format!("info string error {message}")
}
