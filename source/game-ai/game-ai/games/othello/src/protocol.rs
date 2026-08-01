use crate::{
    Evaluation, EvaluationProfile, GameResult, Move, Position, Score, ScoreKind, SearchConfig,
    SearchReport, Side, Square, evaluate, perft, search,
};
use std::fmt::Write;
use std::str::FromStr;
#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;

/// One line-oriented engine session shared by the browser and native binary.
pub struct Engine {
    position: Position,
    history: Vec<Move>,
    evaluator: EvaluationProfile,
    last_report: Option<SearchReport>,
    last_move: Option<Move>,
    last_flips: u64,
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
            evaluator: EvaluationProfile::default(),
            last_report: None,
            last_move: None,
            last_flips: 0,
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
            "eval" => vec![evaluation_info(evaluate(self.position, self.evaluator))],
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
        let mut json = String::with_capacity(if diagnostics { 16_384 } else { 1_024 });
        if diagnostics {
            json.push_str("{\"game\":\"othello\",\"board\":[");
        } else {
            json.push_str("{\"board\":[");
        }
        for index in 0..64u8 {
            if index > 0 {
                json.push(',');
            }
            match self.position.side_at(Square::new(index)) {
                Some(Side::Black) => json.push_str("\"B\""),
                Some(Side::White) => json.push_str("\"W\""),
                None => json.push_str("null"),
            }
        }
        let (result, winner, black, white) = result_fields(self.position);
        write!(
            json,
            "],\"sideToMove\":\"{}\",\"result\":\"{}\",\"winner\":{},\"counts\":{{\"black\":{},\"white\":{}}},",
            self.position.side_to_move(),
            result,
            winner,
            black,
            white
        )
        .expect("writing to String cannot fail");

        json.push_str("\"legalMoves\":[");
        write_moves(&mut json, self.position.legal_moves().as_slice());
        json.push_str("],\"history\":[");
        write_moves(&mut json, &self.history);
        json.push_str("],\"lastMove\":");
        match self.last_move {
            Some(mv) => write!(json, "\"{mv}\"").expect("writing to String cannot fail"),
            None => json.push_str("null"),
        }
        json.push_str(",\"lastFlips\":[");
        write_squares(&mut json, self.last_flips);
        json.push_str("],\"overlays\":{\"legal\":[");
        write_squares(&mut json, self.position.legal_placement_bits());

        if diagnostics {
            json.push_str("],\"blackFrontier\":[");
            write_squares(&mut json, self.position.frontier_bits(Side::Black));
            json.push_str("],\"whiteFrontier\":[");
            write_squares(&mut json, self.position.frontier_bits(Side::White));
            write!(
                json,
                "]}},\"evaluator\":\"{}\",\"evaluation\":",
                self.evaluator
            )
            .expect("writing to String cannot fail");
            write_evaluation_json(&mut json, evaluate(self.position, self.evaluator));
            json.push_str(",\"analysis\":");
            match &self.last_report {
                Some(report) => write_report_json(&mut json, report),
                None => json.push_str("null"),
            }
        } else {
            write!(
                json,
                "]}},\"evaluator\":\"{}\",\"analysis\":",
                self.evaluator
            )
            .expect("writing to String cannot fail");
            match &self.last_report {
                Some(report) => {
                    json.push_str("{\"bestMove\":");
                    match report.best_move {
                        Some(mv) => {
                            write!(json, "\"{mv}\"").expect("writing to String cannot fail");
                        }
                        None => json.push_str("null"),
                    }
                    json.push('}');
                }
                None => json.push_str("null"),
            }
        }
        json.push('}');
        json
    }

    fn reset(&mut self) {
        self.position = Position::start();
        self.history.clear();
        self.last_report = None;
        self.last_move = None;
        self.last_flips = 0;
    }

    fn handshake(&self) -> Vec<String> {
        vec![
            "id name ai-othello".to_owned(),
            "id author Nick Gustafson".to_owned(),
            "id game othello".to_owned(),
            "option name Evaluator type combo default phase var material var mobility var corners var frontier var phase"
                .to_owned(),
            "gaiok".to_owned(),
        ]
    }

    fn set_option(&mut self, words: &[&str]) -> Vec<String> {
        if words.len() != 4
            || !words[0].eq_ignore_ascii_case("name")
            || !words[1].eq_ignore_ascii_case("evaluator")
            || !words[2].eq_ignore_ascii_case("value")
        {
            return vec![error(
                "bad_setoption",
                "expected: setoption name Evaluator value <evaluator>",
            )];
        }
        match EvaluationProfile::from_str(words[3]) {
            Ok(evaluator) => {
                self.evaluator = evaluator;
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
                "Othello supports: position startpos [moves ...]",
            )];
        }
        let move_words = match words.get(1) {
            None => &[][..],
            Some(&"moves") => &words[2..],
            Some(_) => {
                return vec![error("bad_position", "expected 'moves' after 'startpos'")];
            }
        };
        let mut position = Position::start();
        let mut history = Vec::with_capacity(move_words.len());
        let mut last_move = None;
        let mut last_flips = 0;
        for word in move_words {
            let mv = match Move::from_str(word) {
                Ok(mv) => mv,
                Err(message) => return vec![error("bad_move", message)],
            };
            match position.make_move(mv) {
                Ok(undo) => {
                    history.push(mv);
                    last_move = Some(mv);
                    last_flips = undo.flipped();
                }
                Err(move_error) => {
                    return vec![error("illegal_move", &move_error.to_string())];
                }
            }
        }
        self.position = position;
        self.history = history;
        self.last_move = last_move;
        self.last_flips = last_flips;
        self.last_report = None;
        Vec::new()
    }

    fn go(&mut self, words: &[&str]) -> Vec<String> {
        let Ok(config) = parse_config(words, self.evaluator) else {
            return vec![error(
                "bad_go",
                "expected: go depth <0..16> [endgame <0..16>]",
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

        let report = search(self.position, config);

        #[cfg(target_arch = "wasm32")]
        let elapsed_millis = (js_sys::Date::now() - started).max(0.0) as u128;
        #[cfg(not(target_arch = "wasm32"))]
        let elapsed_millis = started.elapsed().as_millis();

        let nps = u128::from(report.stats.nodes) * 1_000 / elapsed_millis.max(1);
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
            "info depth {} {} nodes {} nps {} time {} cutoffs {} passes {} exactnodes {} exact {} pv {}",
            config.depth,
            score_protocol(report.score, report.exact),
            report.stats.nodes,
            nps,
            elapsed_millis,
            report.stats.cutoffs,
            report.stats.passes,
            report.stats.exact_nodes,
            report.exact,
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
            "perft depth {depth} nodes {nodes} time {elapsed_millis}"
        )]
    }
}

fn parse_config(words: &[&str], evaluator: EvaluationProfile) -> Result<SearchConfig, ()> {
    if words.len() != 2 && words.len() != 4 {
        return Err(());
    }
    let mut depth = None;
    let mut endgame = None;
    for pair in words.chunks_exact(2) {
        match pair {
            ["depth", value] => {
                let value = value.parse::<u8>().map_err(|_| ())?;
                if value > 16 {
                    return Err(());
                }
                if depth.replace(value).is_some() {
                    return Err(());
                }
            }
            ["endgame", value] => {
                let value = value.parse::<u8>().map_err(|_| ())?;
                if value > 16 {
                    return Err(());
                }
                if endgame.replace(value).is_some() {
                    return Err(());
                }
            }
            _ => return Err(()),
        }
    }
    Ok(SearchConfig {
        depth: depth.ok_or(())?,
        evaluator,
        exact_endgame_empties: endgame.unwrap_or(0),
    })
}

fn evaluation_info(evaluation: Evaluation) -> String {
    format!(
        "evaluation profile {} total {} phase {} material {} mobility {} potential {} corners {} danger {} frontier {}",
        evaluation.profile,
        evaluation.total,
        evaluation.phase,
        evaluation.material,
        evaluation.mobility,
        evaluation.potential_mobility,
        evaluation.corners,
        evaluation.corner_danger,
        evaluation.frontier
    )
}

fn score_protocol(score: Score, exact: bool) -> String {
    match score.kind() {
        ScoreKind::Win { margin } => format!("score win {margin}"),
        ScoreKind::Loss { margin } => format!("score loss {margin}"),
        ScoreKind::Draw => "score draw 0".to_owned(),
        ScoreKind::Estimate(0) if exact => "score draw 0".to_owned(),
        ScoreKind::Estimate(value) => format!("score eval {value}"),
    }
}

fn write_moves(json: &mut String, moves: &[Move]) {
    for (index, mv) in moves.iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
        write!(json, "\"{mv}\"").expect("writing to String cannot fail");
    }
}

fn write_squares(json: &mut String, mut bits: u64) {
    let mut index = 0;
    while bits != 0 {
        if index > 0 {
            json.push(',');
        }
        let square = Square::new(bits.trailing_zeros() as u8);
        write!(json, "\"{square}\"").expect("writing to String cannot fail");
        bits &= bits - 1;
        index += 1;
    }
}

fn write_evaluation_json(json: &mut String, evaluation: Evaluation) {
    write!(
        json,
        "{{\"profile\":\"{}\",\"phase\":{},\"total\":{},\"terms\":{{\"material\":{},\"mobility\":{},\"potentialMobility\":{},\"corners\":{},\"cornerDanger\":{},\"frontier\":{}}},\"weights\":{{\"material\":{},\"mobility\":{},\"potentialMobility\":{},\"corners\":{},\"cornerDanger\":{},\"frontier\":{}}}}}",
        evaluation.profile,
        evaluation.phase,
        evaluation.total,
        evaluation.material,
        evaluation.mobility,
        evaluation.potential_mobility,
        evaluation.corners,
        evaluation.corner_danger,
        evaluation.frontier,
        evaluation.weights.material,
        evaluation.weights.mobility,
        evaluation.weights.potential_mobility,
        evaluation.weights.corners,
        evaluation.weights.corner_danger,
        evaluation.weights.frontier
    )
    .expect("writing to String cannot fail");
}

fn write_report_json(json: &mut String, report: &SearchReport) {
    write!(
        json,
        "{{\"depth\":{},\"evaluator\":\"{}\",\"exact\":{},\"bestMove\":",
        report.config.depth, report.config.evaluator, report.exact
    )
    .expect("writing to String cannot fail");
    match report.best_move {
        Some(mv) => write!(json, "\"{mv}\"").expect("writing to String cannot fail"),
        None => json.push_str("null"),
    }
    json.push_str(",\"score\":");
    write_score_json(json, report.score, report.exact);
    json.push_str(",\"pv\":[");
    write_moves(json, &report.principal_variation);
    write!(
        json,
        "],\"stats\":{{\"nodes\":{},\"leaves\":{},\"cutoffs\":{},\"passes\":{},\"exactNodes\":{},\"maxPly\":{}}},\"candidates\":[",
        report.stats.nodes,
        report.stats.leaves,
        report.stats.cutoffs,
        report.stats.passes,
        report.stats.exact_nodes,
        report.stats.max_ply
    )
    .expect("writing to String cannot fail");
    for (index, candidate) in report.candidates.iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
        write!(
            json,
            "{{\"move\":\"{}\",\"nodes\":{},\"cutoffs\":{},\"flipped\":[",
            candidate.mv, candidate.nodes, candidate.cutoffs
        )
        .expect("writing to String cannot fail");
        write_squares(json, candidate.flipped);
        json.push_str("],\"score\":");
        write_score_json(json, candidate.score, report.exact);
        json.push('}');
    }
    json.push_str("]}");
}

fn write_score_json(json: &mut String, score: Score, exact: bool) {
    match score.kind() {
        ScoreKind::Win { margin } => {
            write!(json, "{{\"kind\":\"win\",\"margin\":{margin}}}")
                .expect("writing to String cannot fail");
        }
        ScoreKind::Loss { margin } => {
            write!(json, "{{\"kind\":\"loss\",\"margin\":{margin}}}")
                .expect("writing to String cannot fail");
        }
        ScoreKind::Draw | ScoreKind::Estimate(0) if exact => {
            json.push_str("{\"kind\":\"draw\"}");
        }
        ScoreKind::Estimate(value) => {
            write!(json, "{{\"kind\":\"estimate\",\"value\":{value}}}")
                .expect("writing to String cannot fail");
        }
        ScoreKind::Draw => json.push_str("{\"kind\":\"draw\"}"),
    }
}

fn result_fields(position: Position) -> (&'static str, &'static str, u8, u8) {
    let black = position.disc_count(Side::Black);
    let white = position.disc_count(Side::White);
    match position.result() {
        GameResult::Ongoing => ("ongoing", "null", black, white),
        GameResult::Draw { .. } => ("draw", "null", black, white),
        GameResult::Win {
            winner: Side::Black,
            ..
        } => ("win", "\"B\"", black, white),
        GameResult::Win {
            winner: Side::White,
            ..
        } => ("win", "\"W\"", black, white),
    }
}

fn error(code: &str, message: &str) -> String {
    format!("error code {code} message {}", message.replace('\n', " "))
}
