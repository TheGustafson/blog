use crate::{
    Color, Evaluation, EvaluationProfile, FeatureDelta, GameResult, IterationSummary, Move,
    NNUE_FEATURES, NNUE_HIDDEN, NNUE_QA, NNUE_QB, NnueAccumulator, PieceContribution, PieceKind,
    Position, Score, ScoreKind, SearchConfig, SearchReport, builtin_nnue, evaluate,
    nnue_feature_index, perft, piece_contributions,
};
use std::fmt::Write;
use std::str::FromStr;
#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;

const DEFAULT_DEPTH: u8 = 6;
const MAX_DEPTH: u8 = 64;

struct LastSearch {
    report: SearchReport,
    requested_depth: u8,
    completed_depth: u8,
    total_nodes: u64,
    elapsed_millis: u128,
    iterations: Vec<IterationSummary>,
}

/// One UCI session shared by the native executable and the browser Worker.
pub struct Engine {
    position: Position,
    moves: Vec<Move>,
    keys: Vec<u64>,
    evaluator: EvaluationProfile,
    incremental_nnue: bool,
    quiescence: bool,
    move_ordering: bool,
    transposition_table: bool,
    last_nnue_delta: Option<FeatureDelta>,
    last_search: Option<LastSearch>,
}

impl Default for Engine {
    fn default() -> Self {
        Self::new()
    }
}

impl Engine {
    pub fn new() -> Self {
        let position = Position::start();
        let key = position.key();
        Self {
            position,
            moves: Vec::new(),
            keys: vec![key],
            evaluator: EvaluationProfile::default(),
            incremental_nnue: true,
            quiescence: true,
            move_ordering: true,
            transposition_table: true,
            last_nnue_delta: None,
            last_search: None,
        }
    }

    pub fn command(&mut self, line: &str) -> Vec<String> {
        self.command_until(line, || false)
    }

    /// Execute one protocol command with a cooperative search-stop signal.
    ///
    /// Browser sessions use [`Self::command`] and cancel by replacing their
    /// Worker. The native stdin adapter supplies an atomic-backed closure so
    /// it can read UCI `stop` while `go` is running.
    pub fn command_until<F>(&mut self, line: &str, should_stop: F) -> Vec<String>
    where
        F: Fn() -> bool,
    {
        let words: Vec<_> = line.split_whitespace().collect();
        let Some(command) = words.first().copied() else {
            return vec![error("empty command")];
        };
        match command {
            "uci" | "gai" => self.handshake(),
            "isready" => vec!["readyok".to_owned()],
            "ucinewgame" | "newgame" => {
                self.reset();
                Vec::new()
            }
            "setoption" => self.set_option(&words[1..]),
            "position" => self.set_position(&words[1..]),
            "go" => self.go(&words[1..], should_stop),
            "stop" | "quit" => Vec::new(),
            "d" => vec![format!("Fen: {}", self.position.fen())],
            "eval" => vec![evaluation_info(evaluate(&self.position, self.evaluator))],
            "perft" => self.run_perft(&words[1..]),
            "bench" => self.bench(&words[1..]),
            "legal" => vec![format!(
                "legalmoves {}",
                self.legal_moves()
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(" ")
            )],
            "state" => vec![format!("state {}", self.diagnostic_snapshot_json())],
            _ => vec![error(&format!("unknown command: {command}"))],
        }
    }

    pub fn snapshot_json(&self) -> String {
        self.serialize_snapshot(false)
    }

    fn diagnostic_snapshot_json(&self) -> String {
        self.serialize_snapshot(true)
    }

    fn serialize_snapshot(&self, diagnostics: bool) -> String {
        let mut position = self.position.clone();
        let legal = position.legal_moves();
        let result = position.result();
        let repeated = self
            .keys
            .iter()
            .filter(|&&key| key == self.position.key())
            .count()
            >= 3;
        let mut json = String::with_capacity(if diagnostics { 32_768 } else { 2_048 });
        if diagnostics {
            json.push_str("{\"game\":\"chess\",\"board\":[");
        } else {
            json.push_str("{\"board\":[");
        }
        for index in 0..64u8 {
            if index > 0 {
                json.push(',');
            }
            match self.position.piece_at(crate::Square::new(index)) {
                Some(piece) => {
                    write!(json, "\"{}\"", piece.fen_char())
                        .expect("writing to String cannot fail");
                }
                None => json.push_str("null"),
            }
        }
        write!(
            json,
            "],\"fen\":\"{}\",\"sideToMove\":\"{}\",\"inCheck\":{},\"result\":\"{}\",\"winner\":{},",
            self.position.fen(),
            self.position.side_to_move(),
            self.position.in_check(self.position.side_to_move()),
            result_name(result, repeated),
            winner_json(result)
        )
        .expect("writing to String cannot fail");
        json.push_str("\"legalMoves\":[");
        write_moves(&mut json, legal.as_slice());
        json.push_str("],\"history\":[");
        write_moves(&mut json, &self.moves);
        json.push_str("],\"lastMove\":");
        match self.moves.last() {
            Some(mv) => write!(json, "\"{mv}\"").expect("writing to String cannot fail"),
            None => json.push_str("null"),
        }

        if diagnostics {
            let evaluation = evaluate(&self.position, self.evaluator);
            let contributions = piece_contributions(&self.position, self.evaluator);
            write!(
                json,
                ",\"halfmoveClock\":{},\"fullmoveNumber\":{},\"evaluator\":\"{}\",\"options\":{{\"incrementalNnue\":{},\"quiescence\":{},\"moveOrdering\":{},\"transpositionTable\":{}}},\"evaluation\":",
                self.position.halfmove_clock(),
                self.position.fullmove_number(),
                self.evaluator,
                self.incremental_nnue,
                self.quiescence,
                self.move_ordering,
                self.transposition_table
            )
            .expect("writing to String cannot fail");
            write_evaluation_json(&mut json, evaluation, &contributions);
            json.push_str(",\"nnue\":");
            write_nnue_json(&mut json, &self.position, self.last_nnue_delta.as_ref());
            json.push_str(",\"analysis\":");
            match &self.last_search {
                Some(search) => write_search_json(&mut json, search),
                None => json.push_str("null"),
            }
        } else {
            json.push_str(",\"analysis\":");
            match &self.last_search {
                Some(search) => {
                    json.push_str("{\"bestMove\":");
                    match search.report.best_move {
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
        self.moves.clear();
        self.keys.clear();
        self.keys.push(self.position.key());
        self.last_nnue_delta = None;
        self.last_search = None;
    }

    fn handshake(&self) -> Vec<String> {
        vec![
            "id name AI Chess".to_owned(),
            "id author Nick Gustafson".to_owned(),
            "option name Evaluator type combo default piece-square var material var piece-square var tiny-nnue"
                .to_owned(),
            "option name Quiescence type check default true".to_owned(),
            "option name NNUE Accumulator type check default true".to_owned(),
            "option name Move Ordering type check default true".to_owned(),
            "option name Transposition Table type check default true".to_owned(),
            "uciok".to_owned(),
        ]
    }

    fn set_option(&mut self, words: &[&str]) -> Vec<String> {
        let Some(name_marker) = words
            .first()
            .filter(|word| word.eq_ignore_ascii_case("name"))
        else {
            return vec![error("setoption requires 'name <name> value <value>'")];
        };
        let _ = name_marker;
        let Some(value_index) = words
            .iter()
            .position(|word| word.eq_ignore_ascii_case("value"))
        else {
            return vec![error("setoption requires a value")];
        };
        if value_index <= 1 || value_index + 1 >= words.len() {
            return vec![error("setoption name or value is missing")];
        }
        let name = words[1..value_index].join(" ").to_ascii_lowercase();
        let value = words[value_index + 1..].join(" ");
        let changed = match name.as_str() {
            "evaluator" => match EvaluationProfile::from_str(&value) {
                Ok(profile) => {
                    self.evaluator = profile;
                    true
                }
                Err(message) => return vec![error(message)],
            },
            "quiescence" => match parse_bool(&value) {
                Some(value) => {
                    self.quiescence = value;
                    true
                }
                None => return vec![error("Quiescence must be true or false")],
            },
            "nnue accumulator" | "nnueaccumulator" | "incremental nnue" => {
                match parse_bool(&value) {
                    Some(value) => {
                        self.incremental_nnue = value;
                        true
                    }
                    None => return vec![error("NNUE Accumulator must be true or false")],
                }
            }
            "move ordering" | "moveordering" => match parse_bool(&value) {
                Some(value) => {
                    self.move_ordering = value;
                    true
                }
                None => return vec![error("Move Ordering must be true or false")],
            },
            "transposition table" | "transpositiontable" => match parse_bool(&value) {
                Some(value) => {
                    self.transposition_table = value;
                    true
                }
                None => return vec![error("Transposition Table must be true or false")],
            },
            _ => return vec![error(&format!("unknown option: {name}"))],
        };
        if changed {
            self.last_search = None;
        }
        Vec::new()
    }

    fn set_position(&mut self, words: &[&str]) -> Vec<String> {
        let Some(first) = words.first().copied() else {
            return vec![error("position requires 'startpos' or 'fen <six fields>'")];
        };
        let (mut position, move_words) = match first {
            "startpos" => match trailing_moves(&words[1..]) {
                Ok(moves) => (Position::start(), moves),
                Err(message) => return vec![error(message)],
            },
            "fen" => {
                if words.len() < 7 {
                    return vec![error("position fen requires all six FEN fields")];
                }
                let fen = words[1..7].join(" ");
                let position = match Position::from_fen(&fen) {
                    Ok(position) => position,
                    Err(message) => return vec![error(message)],
                };
                let move_words = match trailing_moves(&words[7..]) {
                    Ok(moves) => moves,
                    Err(message) => return vec![error(message)],
                };
                (position, move_words)
            }
            _ => return vec![error("position must begin with startpos or fen")],
        };

        let mut moves = Vec::with_capacity(move_words.len());
        let mut keys = vec![position.key()];
        let mut last_nnue_delta = None;
        for notation in move_words {
            let mv = match position.find_move(notation) {
                Ok(mv) => mv,
                Err(message) => return vec![error(&format!("{notation}: {message}"))],
            };
            let delta = FeatureDelta::from_move(&position, mv)
                .expect("a legal move must have a valid NNUE feature delta");
            position
                .make_move(mv)
                .expect("a move selected from the legal list must be makeable");
            last_nnue_delta = Some(delta);
            moves.push(mv);
            keys.push(position.key());
        }
        self.position = position;
        self.moves = moves;
        self.keys = keys;
        self.last_nnue_delta = last_nnue_delta;
        self.last_search = None;
        Vec::new()
    }

    fn go<F>(&mut self, words: &[&str], should_stop: F) -> Vec<String>
    where
        F: Fn() -> bool,
    {
        let limits = match parse_go(words, self.position.side_to_move()) {
            Ok(limits) => limits,
            Err(message) => return vec![error(message), "bestmove 0000".to_owned()],
        };
        let config = SearchConfig {
            depth: limits.depth,
            nodes: limits.nodes,
            time_millis: limits.time_millis,
            evaluator: self.evaluator,
            incremental_nnue: self.incremental_nnue,
            quiescence: self.quiescence,
            move_ordering: self.move_ordering,
            transposition_table: self.transposition_table,
        };

        #[cfg(target_arch = "wasm32")]
        let started = js_sys::Date::now();
        #[cfg(not(target_arch = "wasm32"))]
        let started = Instant::now();

        let prior = &self.keys[..self.keys.len().saturating_sub(1)];
        let iterative = crate::search::iterative_search_with_history_until(
            self.position.clone(),
            config,
            prior,
            should_stop,
        );

        #[cfg(target_arch = "wasm32")]
        let elapsed_millis = (js_sys::Date::now() - started).max(0.0) as u128;
        #[cfg(not(target_arch = "wasm32"))]
        let elapsed_millis = started.elapsed().as_millis();

        let report = iterative.result;
        let total_nodes = iterative.total_nodes;
        let completed_depth = iterative.completed_depth;
        let iterations = iterative.iterations;
        let nps = u128::from(total_nodes) * 1_000 / elapsed_millis.max(1);
        let pv = report
            .principal_variation
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(" ");
        let info = format!(
            "info depth {} seldepth {} {} nodes {} nps {} time {} qnodes {} tthits {} pv {}",
            completed_depth,
            report.stats.max_ply,
            uci_score(report.score),
            total_nodes,
            nps,
            elapsed_millis,
            report.stats.qnodes,
            report.stats.tt_hits,
            pv
        );
        let best_move = report
            .best_move
            .map_or_else(|| "0000".to_owned(), |mv| mv.to_string());
        self.last_search = Some(LastSearch {
            report,
            requested_depth: limits.depth,
            completed_depth,
            total_nodes,
            elapsed_millis,
            iterations,
        });
        vec![info, format!("bestmove {best_move}")]
    }

    fn legal_moves(&self) -> Vec<Move> {
        let mut position = self.position.clone();
        position.legal_moves().into_iter().collect()
    }

    fn run_perft(&self, words: &[&str]) -> Vec<String> {
        if words.len() != 1 {
            return vec![error("perft requires one depth")];
        }
        let Ok(depth) = words[0].parse::<u8>() else {
            return vec![error("perft depth must be an integer")];
        };
        if depth > 6 {
            return vec![error("perft depth must be between 0 and 6")];
        }
        let mut position = self.position.clone();
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
            "info string perft depth {depth} nodes {nodes} time {elapsed_millis}"
        )]
    }

    fn bench(&mut self, words: &[&str]) -> Vec<String> {
        if !words.is_empty() {
            return vec![error("bench takes no arguments")];
        }
        let saved = self.position.clone();
        let saved_moves = self.moves.clone();
        let saved_keys = self.keys.clone();
        let setup =
            self.set_position(&["startpos", "moves", "e2e4", "e7e5", "g1f3", "b8c6", "f1b5"]);
        debug_assert!(setup.is_empty());
        let response = self.go(&["nodes", "50000"], || false);
        let signature = self.last_search.as_ref().map_or_else(
            || "none".to_owned(),
            |search| {
                format!(
                    "{}:{}:{}",
                    search
                        .report
                        .best_move
                        .map_or_else(|| "0000".to_owned(), |mv| mv.to_string()),
                    search.report.score.raw(),
                    search.total_nodes
                )
            },
        );
        self.position = saved;
        self.moves = saved_moves;
        self.keys = saved_keys;
        self.last_search = None;
        vec![format!(
            "info string bench signature {signature} detail {}",
            response.join(" | ")
        )]
    }
}

struct GoLimits {
    depth: u8,
    nodes: Option<u64>,
    time_millis: Option<u64>,
}

fn parse_go(words: &[&str], side: Color) -> Result<GoLimits, &'static str> {
    if words.is_empty() {
        return Ok(GoLimits {
            depth: DEFAULT_DEPTH,
            nodes: None,
            time_millis: None,
        });
    }
    let mut depth = None;
    let mut nodes = None;
    let mut movetime = None;
    let mut white_time = None;
    let mut black_time = None;
    let mut white_increment = 0;
    let mut black_increment = 0;
    let mut moves_to_go = 30;
    let mut infinite = false;
    let mut seen = [false; 8];
    let mut index = 0;
    while index < words.len() {
        let name = words[index];
        if name == "infinite" {
            if infinite {
                return Err("duplicate go option");
            }
            infinite = true;
            index += 1;
            continue;
        }
        let option_index = match name {
            "depth" => 0,
            "nodes" => 1,
            "movetime" => 2,
            "wtime" => 3,
            "btime" => 4,
            "winc" => 5,
            "binc" => 6,
            "movestogo" => 7,
            _ => return Err("unsupported go option"),
        };
        if seen[option_index] {
            return Err("duplicate go option");
        }
        seen[option_index] = true;
        let value = words.get(index + 1).ok_or("go option is missing a value")?;
        match name {
            "depth" => {
                let parsed = value
                    .parse::<u8>()
                    .map_err(|_| "depth must be an integer")?;
                if !(1..=MAX_DEPTH).contains(&parsed) {
                    return Err("depth must be between 1 and 64");
                }
                depth = Some(parsed);
            }
            "nodes" => {
                let parsed = value
                    .parse::<u64>()
                    .map_err(|_| "nodes must be an integer")?;
                if parsed == 0 {
                    return Err("nodes must be positive");
                }
                nodes = Some(parsed);
            }
            "movetime" => {
                let parsed = value
                    .parse::<u64>()
                    .map_err(|_| "movetime must be an integer")?;
                if parsed == 0 {
                    return Err("movetime must be positive");
                }
                movetime = Some(parsed);
            }
            "wtime" => white_time = Some(parse_clock(value)?),
            "btime" => black_time = Some(parse_clock(value)?),
            "winc" => white_increment = parse_clock(value)?,
            "binc" => black_increment = parse_clock(value)?,
            "movestogo" => {
                let parsed = value
                    .parse::<u64>()
                    .map_err(|_| "movestogo must be an integer")?;
                if parsed == 0 {
                    return Err("movestogo must be positive");
                }
                moves_to_go = parsed;
            }
            _ => unreachable!("go option names were validated above"),
        }
        index += 2;
    }
    if infinite {
        return Ok(GoLimits {
            depth: MAX_DEPTH,
            nodes: None,
            time_millis: None,
        });
    }
    let clock = match side {
        Color::White => white_time.map(|time| (time, white_increment)),
        Color::Black => black_time.map(|time| (time, black_increment)),
    };
    let time_millis = movetime.or_else(|| {
        clock.map(|(remaining, increment)| {
            let safety = (remaining / 20).clamp(1, 100);
            let usable = remaining.saturating_sub(safety).max(1);
            (usable / moves_to_go)
                .saturating_add(increment.saturating_mul(3) / 4)
                .min(usable / 2 + 1)
                .max(1)
        })
    });
    Ok(GoLimits {
        depth: depth.unwrap_or(if nodes.is_some() || time_millis.is_some() {
            MAX_DEPTH
        } else {
            DEFAULT_DEPTH
        }),
        nodes,
        time_millis,
    })
}

fn parse_clock(value: &str) -> Result<u64, &'static str> {
    value
        .parse::<u64>()
        .map_err(|_| "clock values must be non-negative integers")
}

fn trailing_moves<'a>(words: &'a [&str]) -> Result<&'a [&'a str], &'static str> {
    match words.first() {
        None => Ok(&[]),
        Some(&"moves") => Ok(&words[1..]),
        Some(_) => Err("expected 'moves' after the base position"),
    }
}

fn parse_bool(value: &str) -> Option<bool> {
    match value.to_ascii_lowercase().as_str() {
        "true" | "on" | "1" => Some(true),
        "false" | "off" | "0" => Some(false),
        _ => None,
    }
}

fn evaluation_info(evaluation: Evaluation) -> String {
    format!(
        "info string eval profile {} total {} phase {} mg_material {} eg_material {} mg_psqt {} eg_psqt {} material {} psqt {} nnue {}",
        evaluation.profile,
        evaluation.total,
        evaluation.phase,
        evaluation.middlegame_material,
        evaluation.endgame_material,
        evaluation.middlegame_piece_square,
        evaluation.endgame_piece_square,
        evaluation.material,
        evaluation.piece_square,
        evaluation.nnue
    )
}

fn uci_score(score: Score) -> String {
    match score.kind() {
        ScoreKind::MateIn { plies } => format!("score mate {}", plies.div_ceil(2)),
        ScoreKind::MatedIn { plies } => format!("score mate -{}", plies.div_ceil(2)),
        ScoreKind::Draw => "score cp 0".to_owned(),
        ScoreKind::Centipawns(value) => format!("score cp {value}"),
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

fn write_evaluation_json(json: &mut String, evaluation: Evaluation, pieces: &[PieceContribution]) {
    write!(
        json,
        "{{\"profile\":\"{}\",\"sideToMove\":\"{}\",\"phase\":{},\"total\":{},\"terms\":{{\"middlegameMaterial\":{},\"endgameMaterial\":{},\"middlegamePieceSquare\":{},\"endgamePieceSquare\":{},\"material\":{},\"pieceSquare\":{},\"nnue\":{}}},\"pieces\":[",
        evaluation.profile,
        evaluation.side_to_move,
        evaluation.phase,
        evaluation.total,
        evaluation.middlegame_material,
        evaluation.endgame_material,
        evaluation.middlegame_piece_square,
        evaluation.endgame_piece_square,
        evaluation.material,
        evaluation.piece_square,
        evaluation.nnue
    )
    .expect("writing to String cannot fail");
    for (index, piece) in pieces.iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
        write!(
            json,
            "{{\"square\":\"{}\",\"color\":\"{}\",\"piece\":\"{}\",\"middlegameMaterial\":{},\"endgameMaterial\":{},\"middlegamePieceSquare\":{},\"endgamePieceSquare\":{},\"material\":{},\"pieceSquare\":{},\"total\":{}}}",
            piece.square,
            piece.color,
            piece_name(piece.kind),
            piece.middlegame_material,
            piece.endgame_material,
            piece.middlegame_piece_square,
            piece.endgame_piece_square,
            piece.material,
            piece.piece_square,
            piece.total
        )
        .expect("writing to String cannot fail");
    }
    json.push_str("]}");
}

fn write_nnue_json(json: &mut String, position: &Position, last_delta: Option<&FeatureDelta>) {
    let network = builtin_nnue();
    let accumulator = NnueAccumulator::refresh(position, network);
    write!(
        json,
        "{{\"score\":{},\"checksum\":\"{:016x}\",\"featureCount\":{},\"hiddenSize\":{},\"activeFeatures\":{},\"quantization\":{{\"feature\":{},\"output\":{}}},\"perspectives\":{{",
        accumulator.evaluate(position.side_to_move(), network),
        network.checksum(),
        NNUE_FEATURES,
        NNUE_HIDDEN,
        position.occupied().count_ones(),
        NNUE_QA,
        NNUE_QB
    )
    .expect("writing to String cannot fail");
    for (index, perspective) in Color::ALL.iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
        write!(json, "\"{perspective}\":").expect("writing to String cannot fail");
        write_accumulator_perspective(json, accumulator.perspective(*perspective));
    }
    json.push_str("},\"lastDelta\":[");
    if let Some(delta) = last_delta {
        for (index, (piece, square, sign)) in delta.changes().enumerate() {
            if index > 0 {
                json.push(',');
            }
            write!(
                json,
                "{{\"piece\":\"{}\",\"color\":\"{}\",\"kind\":\"{}\",\"square\":\"{}\",\"sign\":{},\"whiteFeature\":{},\"blackFeature\":{}}}",
                piece.fen_char(),
                piece.color,
                piece_name(piece.kind),
                square,
                sign,
                nnue_feature_index(piece, square, Color::White),
                nnue_feature_index(piece, square, Color::Black)
            )
            .expect("writing to String cannot fail");
        }
    }
    json.push_str("]}");
}

fn write_accumulator_perspective(json: &mut String, values: &[i32]) {
    let clipped_low = values.iter().filter(|&&value| value <= 0).count();
    let active = values
        .iter()
        .filter(|&&value| value > 0 && value < NNUE_QA)
        .count();
    let clipped_high = values.iter().filter(|&&value| value >= NNUE_QA).count();
    let minimum = values.iter().copied().min().unwrap_or_default();
    let maximum = values.iter().copied().max().unwrap_or_default();
    write!(
        json,
        "{{\"clippedLow\":{},\"active\":{},\"clippedHigh\":{},\"minimum\":{},\"maximum\":{},\"lanes\":[",
        clipped_low, active, clipped_high, minimum, maximum
    )
    .expect("writing to String cannot fail");
    for (index, value) in values.iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
        write!(json, "{value}").expect("writing to String cannot fail");
    }
    json.push_str("]}");
}

fn write_search_json(json: &mut String, search: &LastSearch) {
    let report = &search.report;
    write!(
        json,
        "{{\"requestedDepth\":{},\"completedDepth\":{},\"completed\":{},\"totalNodes\":{},\"elapsedMillis\":{},\"bestMove\":",
        search.requested_depth,
        search.completed_depth,
        search.completed_depth == search.requested_depth,
        search.total_nodes,
        search.elapsed_millis
    )
    .expect("writing to String cannot fail");
    match report.best_move {
        Some(mv) => write!(json, "\"{mv}\"").expect("writing to String cannot fail"),
        None => json.push_str("null"),
    }
    json.push_str(",\"score\":");
    write_score_json(json, report.score);
    json.push_str(",\"pv\":[");
    write_moves(json, &report.principal_variation);
    write!(
        json,
        "],\"stats\":{{\"iterationNodes\":{},\"qnodes\":{},\"leaves\":{},\"evaluations\":{},\"cutoffs\":{},\"ttProbes\":{},\"ttHits\":{},\"ttStores\":{},\"maxPly\":{},\"nnueRefreshes\":{},\"nnueUpdates\":{},\"nnueFeatureChanges\":{},\"nnueAccumulatorOps\":{}}},\"candidates\":[",
        report.stats.nodes,
        report.stats.qnodes,
        report.stats.leaves,
        report.stats.evaluations,
        report.stats.cutoffs,
        report.stats.tt_probes,
        report.stats.tt_hits,
        report.stats.tt_stores,
        report.stats.max_ply,
        report.stats.nnue_refreshes,
        report.stats.nnue_updates,
        report.stats.nnue_feature_changes,
        report.stats.nnue_accumulator_ops
    )
    .expect("writing to String cannot fail");
    for (index, candidate) in report.candidates.iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
        write!(
            json,
            "{{\"move\":\"{}\",\"nodes\":{},\"qnodes\":{},\"cutoffs\":{},\"completed\":{},\"score\":",
            candidate.mv,
            candidate.nodes,
            candidate.qnodes,
            candidate.cutoffs,
            candidate.completed
        )
        .expect("writing to String cannot fail");
        write_score_json(json, candidate.score);
        json.push('}');
    }
    json.push_str("],\"iterations\":[");
    for (index, iteration) in search.iterations.iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
        write!(
            json,
            "{{\"depth\":{},\"completed\":{},\"bestMove\":",
            iteration.depth, iteration.completed
        )
        .expect("writing to String cannot fail");
        match iteration.best_move {
            Some(mv) => write!(json, "\"{mv}\"").expect("writing to String cannot fail"),
            None => json.push_str("null"),
        }
        json.push_str(",\"score\":");
        write_score_json(json, iteration.score);
        write!(
            json,
            ",\"nodes\":{},\"qnodes\":{},\"cutoffs\":{},\"ttHits\":{}}}",
            iteration.nodes, iteration.qnodes, iteration.cutoffs, iteration.tt_hits
        )
        .expect("writing to String cannot fail");
    }
    json.push_str("]}");
}

fn write_score_json(json: &mut String, score: Score) {
    match score.kind() {
        ScoreKind::MateIn { plies } => {
            write!(json, "{{\"kind\":\"mate\",\"plies\":{plies}}}")
                .expect("writing to String cannot fail");
        }
        ScoreKind::MatedIn { plies } => {
            write!(json, "{{\"kind\":\"mated\",\"plies\":{plies}}}")
                .expect("writing to String cannot fail");
        }
        ScoreKind::Draw => json.push_str("{\"kind\":\"draw\"}"),
        ScoreKind::Centipawns(value) => {
            write!(json, "{{\"kind\":\"centipawns\",\"value\":{value}}}")
                .expect("writing to String cannot fail");
        }
    }
}

const fn piece_name(kind: PieceKind) -> &'static str {
    match kind {
        PieceKind::Pawn => "pawn",
        PieceKind::Knight => "knight",
        PieceKind::Bishop => "bishop",
        PieceKind::Rook => "rook",
        PieceKind::Queen => "queen",
        PieceKind::King => "king",
    }
}

const fn result_name(result: GameResult, repeated: bool) -> &'static str {
    if repeated && matches!(result, GameResult::Ongoing) {
        return "threefold";
    }
    match result {
        GameResult::Ongoing => "ongoing",
        GameResult::Checkmate { .. } => "checkmate",
        GameResult::Stalemate => "stalemate",
        GameResult::FiftyMoveDraw => "fifty-move",
        GameResult::InsufficientMaterialDraw => "insufficient-material",
    }
}

fn winner_json(result: GameResult) -> &'static str {
    match result {
        GameResult::Checkmate {
            winner: Color::White,
        } => "\"white\"",
        GameResult::Checkmate {
            winner: Color::Black,
        } => "\"black\"",
        GameResult::Ongoing
        | GameResult::Stalemate
        | GameResult::FiftyMoveDraw
        | GameResult::InsufficientMaterialDraw => "null",
    }
}

fn error(message: &str) -> String {
    format!("info string error {}", message.replace('\n', " "))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uci_clock_allocation_is_bounded_and_infinite_has_clear_precedence() {
        let ordinary = parse_go(
            &[
                "wtime",
                "10000",
                "btime",
                "20000",
                "winc",
                "1000",
                "movestogo",
                "20",
            ],
            Color::White,
        )
        .unwrap();
        assert_eq!(ordinary.depth, MAX_DEPTH);
        assert_eq!(ordinary.nodes, None);
        assert_eq!(ordinary.time_millis, Some(1_245));

        let hostile = parse_go(
            &[
                "wtime",
                "10",
                "winc",
                "18446744073709551615",
                "movestogo",
                "1",
            ],
            Color::White,
        )
        .unwrap();
        assert_eq!(hostile.time_millis, Some(5));

        let infinite = parse_go(
            &["infinite", "wtime", "10", "nodes", "1", "depth", "1"],
            Color::White,
        )
        .unwrap();
        assert_eq!(infinite.depth, MAX_DEPTH);
        assert_eq!(infinite.nodes, None);
        assert_eq!(infinite.time_millis, None);
    }
}
