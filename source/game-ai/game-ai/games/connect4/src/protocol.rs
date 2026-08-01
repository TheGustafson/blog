use crate::oracle::{OracleCase, probe_oracle};
use crate::{
    Algorithm, Cell, Column, GameResult, Move, Position, Score, ScoreKind, SearchLimits,
    SearchReport, Side, iterative_search, perft, search,
};
use std::fmt::Write;
use std::str::FromStr;
#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;

/// One line-oriented engine session shared by tests and the native binary.
pub struct Engine {
    position: Position,
    history: Vec<Move>,
    algorithm: Algorithm,
    last_report: Option<SearchReport>,
    last_oracle: Option<Option<&'static OracleCase>>,
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
            last_report: None,
            last_oracle: None,
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
            "legal" => vec![format!(
                "legalmoves {}",
                self.position
                    .legal_moves()
                    .into_iter()
                    .map(|mv| mv.to_string())
                    .collect::<Vec<_>>()
                    .join(" ")
            )],
            "perft" => self.run_perft(&words[1..]),
            "oracle" => self.oracle(&words[1..]),
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
        let mut json = String::with_capacity(if diagnostics { 8_192 } else { 512 });
        if diagnostics {
            json.push_str("{\"game\":\"connect4\",\"columns\":[");
        } else {
            json.push_str("{\"columns\":[");
        }
        for column_index in 0..Column::COUNT {
            if column_index > 0 {
                json.push(',');
            }
            json.push('[');
            for row in 0..Cell::ROWS {
                if row > 0 {
                    json.push(',');
                }
                let cell = Cell::new(Column::new(column_index as u8), row as u8);
                match self.position.side_at(cell) {
                    Some(Side::Red) => json.push_str("\"R\""),
                    Some(Side::Yellow) => json.push_str("\"Y\""),
                    None => json.push_str("null"),
                }
            }
            json.push(']');
        }
        write!(
            json,
            "],\"sideToMove\":\"{}\",\"result\":\"{}\",\"winner\":{},",
            self.position.side_to_move(),
            result_name(self.position.result()),
            winner_json(self.position.result())
        )
        .expect("writing to String cannot fail");

        json.push_str("\"winningLine\":[");
        for (index, cell) in self.position.winning_cells().iter().enumerate() {
            if index > 0 {
                json.push(',');
            }
            write!(json, "\"{cell}\"").expect("writing to String cannot fail");
        }
        json.push_str("],\"legalMoves\":[");
        for (index, mv) in self.position.legal_moves().into_iter().enumerate() {
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
            write!(json, "],\"algorithm\":\"{}\",\"oracle\":", self.algorithm)
                .expect("writing to String cannot fail");
            match self.last_oracle {
                None => json.push_str("null"),
                Some(None) => json.push_str("{\"status\":\"miss\"}"),
                Some(Some(case)) => {
                    write!(
                        json,
                        "{{\"status\":\"hit\",\"source\":\"gamesolver-tutorial\",\"notation\":\"{}\",\"ponsScore\":{},\"outcome\":\"{}\",\"description\":\"{}\"}}",
                        case.notation,
                        case.pons_score,
                        case.outcome.name(),
                        case.description
                    )
                    .expect("writing to String cannot fail");
                }
            }
            json.push_str(",\"analysis\":");
            if let Some(report) = &self.last_report {
                write_report_json(&mut json, report);
            } else {
                json.push_str("null");
            }
        } else {
            json.push_str("],\"analysis\":");
            if let Some(report) = &self.last_report {
                json.push_str("{\"bestMove\":");
                match report.best_move {
                    Some(mv) => {
                        write!(json, "\"{mv}\"").expect("writing to String cannot fail");
                    }
                    None => json.push_str("null"),
                }
                json.push('}');
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
        self.last_oracle = None;
    }

    fn handshake(&self) -> Vec<String> {
        vec![
            "id name ai-connect4".to_owned(),
            "id author Nick Gustafson".to_owned(),
            "id game connect4".to_owned(),
            "option name Algorithm type combo default tt var plain var alpha-beta var ordered var tt"
                .to_owned(),
            "option name Depth type spin default 7 min 1 max 42".to_owned(),
            "extension go iterative depth <depth> [nodes <budget>]".to_owned(),
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
                Vec::new()
            }
            Err(message) => vec![error("bad_option_value", message)],
        }
    }

    fn set_position(&mut self, words: &[&str]) -> Vec<String> {
        if words.first().copied() != Some("startpos") {
            return vec![error(
                "bad_position",
                "Connect Four supports: position startpos [moves ...]",
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
                self.last_oracle = None;
                Vec::new()
            }
            Err(move_error) => vec![error("illegal_move", &move_error.to_string())],
        }
    }

    fn go(&mut self, words: &[&str]) -> Vec<String> {
        let (iterative, limit_words) = match words.first() {
            Some(&"iterative") => (true, &words[1..]),
            _ => (false, words),
        };
        let Ok(limits) = parse_limits(limit_words) else {
            return vec![error(
                "bad_go",
                "expected: go [iterative] depth <1..42> [nodes <positive integer>]",
            )];
        };
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

        let (report, reported_nodes, completed_depth, iterations) = if iterative {
            let iterative = iterative_search(self.position, self.algorithm, limits);
            (
                iterative.result,
                iterative.total_nodes,
                iterative.completed_depth,
                iterative.iterations.len(),
            )
        } else {
            let report = search(self.position, self.algorithm, limits);
            let nodes = report.stats.nodes;
            (report, nodes, limits.depth, 1)
        };

        #[cfg(target_arch = "wasm32")]
        let elapsed_millis = (js_sys::Date::now() - started).max(0.0) as u128;
        #[cfg(not(target_arch = "wasm32"))]
        let elapsed_millis = started.elapsed().as_millis();
        let millis = elapsed_millis.max(1);
        let nps = u128::from(reported_nodes) * 1_000 / millis;
        let pv = report
            .principal_variation
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(" ");
        let best_move = report
            .best_move
            .map_or_else(|| "(none)".to_owned(), |mv| mv.to_string());
        let info = format!(
            "info depth {} {} nodes {} iterationnodes {} nps {} time {} cutoffs {} tthits {} ttstores {} completed {} iterations {} pv {}",
            completed_depth,
            score_protocol(report.score),
            reported_nodes,
            report.stats.nodes,
            nps,
            elapsed_millis,
            report.stats.cutoffs,
            report.stats.tt_hits,
            report.stats.tt_stores,
            report.completed,
            iterations,
            pv
        );
        self.last_report = Some(report);
        vec![info, format!("bestmove {best_move}")]
    }

    fn run_perft(&self, words: &[&str]) -> Vec<String> {
        if words.len() != 1 {
            return vec![error("bad_perft", "expected: perft <depth 0..12>")];
        }
        let Ok(depth) = words[0].parse::<u8>() else {
            return vec![error("bad_perft", "depth must be an integer")];
        };
        if depth > 12 {
            return vec![error("bad_perft", "depth must be between 0 and 12")];
        }
        let mut position = self.position;
        #[cfg(target_arch = "wasm32")]
        let started = js_sys::Date::now();
        #[cfg(not(target_arch = "wasm32"))]
        let started = Instant::now();

        let nodes = perft(&mut position, depth);

        #[cfg(target_arch = "wasm32")]
        let elapsed_millis = (js_sys::Date::now() - started).max(0.0) as u128;
        #[cfg(not(target_arch = "wasm32"))]
        let elapsed_millis = started.elapsed().as_millis();

        vec![format!(
            "perft depth {depth} nodes {nodes} time {}",
            elapsed_millis
        )]
    }

    fn oracle(&mut self, words: &[&str]) -> Vec<String> {
        if !words.is_empty() {
            return vec![error("bad_oracle", "expected: oracle")];
        }
        let hit = probe_oracle(self.position);
        self.last_oracle = Some(hit);
        match hit {
            Some(case) => vec![format!(
                "oracle source gamesolver-tutorial notation {} score {} outcome {}",
                case.notation,
                case.pons_score,
                case.outcome.name()
            )],
            None => vec!["oracle miss".to_owned()],
        }
    }
}

fn parse_limits(words: &[&str]) -> Result<SearchLimits, ()> {
    if words.len() != 2 && words.len() != 4 {
        return Err(());
    }
    let mut depth = None;
    let mut nodes = None;
    for pair in words.chunks_exact(2) {
        match pair {
            ["depth", value] => {
                let value = value.parse::<u8>().map_err(|_| ())?;
                if !(1..=42).contains(&value) {
                    return Err(());
                }
                if depth.replace(value).is_some() {
                    return Err(());
                }
            }
            ["nodes", value] => {
                let value = value.parse::<u64>().map_err(|_| ())?;
                if value == 0 {
                    return Err(());
                }
                if nodes.replace(value).is_some() {
                    return Err(());
                }
            }
            _ => return Err(()),
        }
    }
    let depth = depth.ok_or(())?;
    Ok(SearchLimits { depth, nodes })
}

fn score_protocol(score: Score) -> String {
    match score.kind() {
        ScoreKind::ForcedWin { plies } => format!("score win {plies}"),
        ScoreKind::ForcedLoss { plies } => format!("score loss {plies}"),
        ScoreKind::Estimate(value) => format!("score eval {value}"),
    }
}

fn write_report_json(json: &mut String, report: &SearchReport) {
    write!(
        json,
        "{{\"algorithm\":\"{}\",\"depth\":{},\"completed\":{},\"bestMove\":",
        report.algorithm, report.requested_depth, report.completed
    )
    .expect("writing to String cannot fail");
    match report.best_move {
        Some(mv) => write!(json, "\"{mv}\"").expect("writing to String cannot fail"),
        None => json.push_str("null"),
    }
    json.push_str(",\"score\":");
    write_score_json(json, report.score);
    json.push_str(",\"pv\":[");
    for (index, mv) in report.principal_variation.iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
        write!(json, "\"{mv}\"").expect("writing to String cannot fail");
    }
    write!(
        json,
        "],\"stats\":{{\"nodes\":{},\"leaves\":{},\"cutoffs\":{},\"ttProbes\":{},\"ttHits\":{},\"ttStores\":{},\"maxPly\":{}}},\"rootBranches\":[",
        report.stats.nodes,
        report.stats.leaves,
        report.stats.cutoffs,
        report.stats.tt_probes,
        report.stats.tt_hits,
        report.stats.tt_stores,
        report.stats.max_ply
    )
    .expect("writing to String cannot fail");
    for (index, branch) in report.root_branches.iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
        write!(
            json,
            "{{\"move\":\"{}\",\"nodes\":{},\"cutoffs\":{},\"completed\":{},\"score\":",
            branch.mv, branch.nodes, branch.cutoffs, branch.completed
        )
        .expect("writing to String cannot fail");
        write_score_json(json, branch.score);
        json.push('}');
    }
    json.push_str("]}");
}

fn write_score_json(json: &mut String, score: Score) {
    match score.kind() {
        ScoreKind::ForcedWin { plies } => {
            write!(json, "{{\"kind\":\"win\",\"plies\":{plies}}}")
                .expect("writing to String cannot fail");
        }
        ScoreKind::ForcedLoss { plies } => {
            write!(json, "{{\"kind\":\"loss\",\"plies\":{plies}}}")
                .expect("writing to String cannot fail");
        }
        ScoreKind::Estimate(value) => {
            write!(json, "{{\"kind\":\"estimate\",\"value\":{value}}}")
                .expect("writing to String cannot fail");
        }
    }
}

const fn result_name(result: GameResult) -> &'static str {
    match result {
        GameResult::Ongoing => "ongoing",
        GameResult::Draw => "draw",
        GameResult::Win(_) => "win",
    }
}

fn winner_json(result: GameResult) -> &'static str {
    match result {
        GameResult::Win(Side::Red) => "\"R\"",
        GameResult::Win(Side::Yellow) => "\"Y\"",
        GameResult::Ongoing | GameResult::Draw => "null",
    }
}

fn error(code: &str, message: &str) -> String {
    format!("error code {code} message {}", message.replace('\n', " "))
}
