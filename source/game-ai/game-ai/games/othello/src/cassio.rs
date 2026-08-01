//! Native adapter for the useful core of Cassio's `ENGINE-PROTOCOL`.
//!
//! Cassio owns iterative deepening and time management. This adapter accepts
//! one explicit depth per request and deliberately keeps that wire contract
//! outside the game, evaluation, and search modules.

use crate::{
    EvaluationProfile, Position, ScoreKind, SearchConfig, SearchReport, Side, search_until,
};
use std::time::Instant;

const MAX_DEPTH: u8 = 16;

#[derive(Debug, Default)]
/// Line-oriented adapter for Cassio's `ENGINE-PROTOCOL`.
pub struct CassioEngine;

impl CassioEngine {
    pub const fn new() -> Self {
        Self
    }

    pub fn command(&mut self, line: &str) -> Vec<String> {
        self.command_until(line, || false)
    }

    /// Executes one command while polling `should_stop` during search.
    pub fn command_until<F: Fn() -> bool>(&mut self, line: &str, should_stop: F) -> Vec<String> {
        let mut words = line.split_whitespace();
        if words.next() != Some("ENGINE-PROTOCOL") {
            return vec![cassio_error("expected ENGINE-PROTOCOL prefix")];
        }
        let Some(command) = words.next() else {
            return vec![
                cassio_error("missing command after ENGINE-PROTOCOL"),
                "ready.".to_owned(),
            ];
        };
        let parameters: Vec<_> = words.collect();
        match command.to_ascii_lowercase().as_str() {
            "init" | "new-position" | "empty-hash" | "stop" => {
                without_parameters(command, &parameters, vec!["ready.".to_owned()])
            }
            "get-version" => without_parameters(
                command,
                &parameters,
                vec!["version: AI Othello 0.1.0".to_owned(), "ready.".to_owned()],
            ),
            "midgame-search" => self.midgame_search(&parameters, should_stop),
            "endgame-search" => self.endgame_search(&parameters, should_stop),
            "feed-hash" => unsupported("feed-hash"),
            "get-search-infos" => unsupported("live search polling"),
            "quit" | "eof" => without_parameters(command, &parameters, vec!["bye bye.".to_owned()]),
            _ => malformed(command, "unknown command"),
        }
    }

    fn midgame_search<F: Fn() -> bool>(&mut self, words: &[&str], should_stop: F) -> Vec<String> {
        let Ok((position, consumed)) = parse_position(words) else {
            return malformed("midgame-search", "expected a 64-cell board and X/O side");
        };
        let limits = &words[consumed..];
        if limits.len() != 4 {
            return malformed(
                "midgame-search",
                "expected: <position> <alpha> <beta> <depth> <precision>",
            );
        }
        let (Ok(alpha), Ok(beta), Ok(depth), Ok(precision)) = (
            limits[0].parse::<f64>(),
            limits[1].parse::<f64>(),
            limits[2].parse::<u8>(),
            limits[3].parse::<u8>(),
        ) else {
            return malformed("midgame-search", "search limits must be numeric");
        };
        if !alpha.is_finite() || !beta.is_finite() || alpha >= beta {
            return malformed("midgame-search", "alpha must be less than beta");
        }
        if depth > MAX_DEPTH {
            return malformed("midgame-search", "depth must be between 0 and 16");
        }
        if !(1..=100).contains(&precision) {
            return malformed("midgame-search", "precision must be between 1 and 100");
        }
        self.run_search(position, depth, 0, precision, should_stop)
    }

    fn endgame_search<F: Fn() -> bool>(&mut self, words: &[&str], should_stop: F) -> Vec<String> {
        let Ok((position, consumed)) = parse_position(words) else {
            return malformed("endgame-search", "expected a 64-cell board and X/O side");
        };
        let limits = &words[consumed..];
        if limits.len() != 3 {
            return malformed(
                "endgame-search",
                "expected: <position> <alpha> <beta> <precision>",
            );
        }
        let (Ok(alpha), Ok(beta), Ok(precision)) = (
            limits[0].parse::<i32>(),
            limits[1].parse::<i32>(),
            limits[2].parse::<u8>(),
        ) else {
            return malformed("endgame-search", "search limits must be integers");
        };
        if alpha >= beta {
            return malformed("endgame-search", "alpha must be less than beta");
        }
        if !(1..=100).contains(&precision) {
            return malformed("endgame-search", "precision must be between 1 and 100");
        }
        let depth = position.empty_count();
        if depth > MAX_DEPTH {
            return malformed(
                "endgame-search",
                "this teaching engine solves at most 16 empty squares",
            );
        }
        self.run_search(position, depth, depth, precision, should_stop)
    }

    fn run_search<F: Fn() -> bool>(
        &mut self,
        position: Position,
        depth: u8,
        exact_endgame_empties: u8,
        precision: u8,
        should_stop: F,
    ) -> Vec<String> {
        let started = Instant::now();
        let config = SearchConfig {
            depth,
            evaluator: EvaluationProfile::Phase,
            exact_endgame_empties,
        };
        let Some(report) = search_until(position, config, should_stop) else {
            return vec!["ready.".to_owned()];
        };
        let elapsed = started.elapsed().as_secs_f64();
        vec![
            result_line(position, &report, precision, elapsed),
            "ready.".to_owned(),
        ]
    }
}

fn parse_position(words: &[&str]) -> Result<(Position, usize), &'static str> {
    let Some(first) = words.first().copied() else {
        return Err("missing position");
    };
    if !first.is_ascii() {
        return Err("position must use Cassio ASCII cells");
    }
    if first.len() == 65 {
        let (board, side) = first.split_at(64);
        Position::from_cassio(board, side).map(|position| (position, 1))
    } else if first.len() == 64 {
        let Some(side) = words.get(1).copied() else {
            return Err("missing side");
        };
        Position::from_cassio(first, side).map(|position| (position, 2))
    } else {
        Err("bad board length")
    }
}

fn result_line(position: Position, report: &SearchReport, precision: u8, elapsed: f64) -> String {
    let value = match report.score.kind() {
        ScoreKind::Win { margin } => f64::from(margin),
        ScoreKind::Draw => 0.0,
        ScoreKind::Loss { margin } => -f64::from(margin),
        ScoreKind::Estimate(raw) => f64::from(raw) / 100.0,
    };
    let side = match position.side_to_move() {
        Side::Black => 'X',
        Side::White => 'O',
    };
    let best_move = report
        .best_move
        .map_or_else(|| "pa".to_owned(), |mv| mv.to_string());
    let pv = report
        .principal_variation
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("");
    format!(
        "{}, move {}, depth {}, @{}%, {}{:+.2} <= v <= {}{:+.2}, {}, node {}, time {:.3}",
        position.to_cassio(),
        best_move,
        report.config.depth,
        precision,
        side,
        value,
        side,
        value,
        pv,
        report.stats.nodes,
        elapsed
    )
}

fn malformed(command: &str, message: &str) -> Vec<String> {
    vec![
        cassio_error(&format!("{command}: {message}")),
        "ready.".to_owned(),
    ]
}

fn without_parameters(command: &str, parameters: &[&str], response: Vec<String>) -> Vec<String> {
    if parameters.is_empty() {
        response
    } else {
        malformed(command, "this command takes no parameters")
    }
}

fn unsupported(feature: &str) -> Vec<String> {
    vec![
        cassio_error(&format!(
            "{feature} is not supported by this teaching engine"
        )),
        "ready.".to_owned(),
    ]
}

fn cassio_error(message: &str) -> String {
    format!("ERROR: {message}")
}
