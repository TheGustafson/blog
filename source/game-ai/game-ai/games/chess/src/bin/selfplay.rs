use ai_chess::{
    Color, EvaluationProfile, GameResult, Move, Position, SEARCH_PRESETS, SearchConfig,
    iterative_search_with_history, search_preset,
};
use std::env;
use std::process::ExitCode;
use std::time::{Duration, Instant};

#[derive(Clone)]
struct Player {
    name: String,
    search: SearchConfig,
}

#[derive(Default)]
struct Stats {
    moves: u64,
    nodes: u64,
    depth: u64,
    elapsed: Duration,
}

struct Config {
    a: Player,
    b: Player,
    games: usize,
    opening_plies: usize,
    max_plies: usize,
    seed: u64,
    show_games: bool,
}

enum Outcome {
    Win(Color),
    Draw,
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("error: {message}\n\n{}", usage());
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let config = parse_config()?;
    print_player("A", &config.a);
    print_player("B", &config.b);
    println!(
        "{} games · {} random opening plies · paired colors · seed {}\n",
        config.games, config.opening_plies, config.seed,
    );

    let mut a_wins = 0;
    let mut b_wins = 0;
    let mut draws = 0;
    let mut total_plies = 0_u64;
    let mut a_stats = Stats::default();
    let mut b_stats = Stats::default();

    for game in 0..config.games {
        let opening_seed = paired_opening_seed(config.seed, game);
        let (mut position, mut history, mut keys) = opening(opening_seed, config.opening_plies);
        let a_is_white = a_has_first_color(game);

        let outcome = loop {
            if repeated_three_times(position.key(), &keys) || history.len() >= config.max_plies {
                break Outcome::Draw;
            }
            match position.clone().result() {
                GameResult::Ongoing => {}
                GameResult::Checkmate { winner } => break Outcome::Win(winner),
                GameResult::Stalemate
                | GameResult::FiftyMoveDraw
                | GameResult::InsufficientMaterialDraw => break Outcome::Draw,
            }

            let a_to_move = (position.side_to_move() == Color::White) == a_is_white;
            let (player, stats) = if a_to_move {
                (&config.a, &mut a_stats)
            } else {
                (&config.b, &mut b_stats)
            };
            let prior = &keys[..keys.len().saturating_sub(1)];
            let started = Instant::now();
            let report = iterative_search_with_history(position.clone(), player.search, prior);
            stats.elapsed += started.elapsed();
            stats.moves += 1;
            stats.nodes += report.total_nodes;
            stats.depth += u64::from(report.completed_depth);
            let mv = report
                .result
                .best_move
                .ok_or_else(|| "search returned no move in an ongoing game".to_owned())?;
            position
                .make_move(mv)
                .map_err(|error| format!("search returned illegal move {mv}: {error}"))?;
            history.push(mv);
            keys.push(position.key());
        };

        total_plies += history.len() as u64;
        let label = match outcome {
            Outcome::Win(winner) if (winner == Color::White) == a_is_white => {
                a_wins += 1;
                "A wins"
            }
            Outcome::Win(_) => {
                b_wins += 1;
                "B wins"
            }
            Outcome::Draw => {
                draws += 1;
                "draw"
            }
        };
        if config.show_games {
            let moves = history
                .iter()
                .map(Move::to_string)
                .collect::<Vec<_>>()
                .join(" ");
            println!(
                "{:>3}. A as {} · {:>6} · {:>3} plies · {moves}",
                game + 1,
                if a_is_white { "W" } else { "B" },
                label,
                history.len(),
            );
        } else {
            println!(
                "{:>3}. A as {} · {:>6} · {:>3} plies",
                game + 1,
                if a_is_white { "W" } else { "B" },
                label,
                history.len(),
            );
        }
    }

    println!(
        "\nscore  A {a_wins} · B {b_wins} · draws {draws}\naverage game {:.1} plies",
        total_plies as f64 / config.games as f64,
    );
    print_stats("A", &a_stats);
    print_stats("B", &b_stats);
    Ok(())
}

fn print_player(label: &str, player: &Player) {
    println!(
        "{label}: {} (depth {}, {} nodes, {} ms, {})",
        player.name,
        player.search.depth,
        player.search.nodes.unwrap_or_default(),
        player.search.time_millis.unwrap_or_default(),
        player.search.evaluator,
    );
}

fn opening(seed: u64, plies: usize) -> (Position, Vec<Move>, Vec<u64>) {
    let mut random = SplitMix64(seed);
    let mut position = Position::start();
    let mut history = Vec::with_capacity(160);
    let mut keys = vec![position.key()];
    for _ in 0..plies {
        if position.clone().result() != GameResult::Ongoing {
            break;
        }
        let moves: Vec<_> = position.legal_moves().into_iter().collect();
        if moves.is_empty() {
            break;
        }
        let mv = moves[(random.next() as usize) % moves.len()];
        position
            .make_move(mv)
            .expect("opening move was generated as legal");
        history.push(mv);
        keys.push(position.key());
    }
    (position, history, keys)
}

fn paired_opening_seed(seed: u64, game: usize) -> u64 {
    seed.wrapping_add((game / 2) as u64)
}

fn a_has_first_color(game: usize) -> bool {
    game % 2 == 0
}

fn repeated_three_times(key: u64, keys: &[u64]) -> bool {
    keys.iter().filter(|candidate| **candidate == key).count() >= 3
}

fn print_stats(label: &str, stats: &Stats) {
    let moves = stats.moves.max(1);
    println!(
        "{label}      {:>8.0} nodes/move · depth {:>4.1} · {:>7.2} ms/move · {:.2} Mnps",
        stats.nodes as f64 / moves as f64,
        stats.depth as f64 / moves as f64,
        stats.elapsed.as_secs_f64() * 1_000.0 / moves as f64,
        stats.nodes as f64 / stats.elapsed.as_secs_f64().max(0.000_001) / 1_000_000.0,
    );
}

fn parse_config() -> Result<Config, String> {
    parse_config_from(env::args().skip(1).collect())
}

fn parse_config_from(args: Vec<String>) -> Result<Config, String> {
    let mut a_name = "maximum".to_owned();
    let mut b_name = "maximum".to_owned();
    let mut a_depth = None;
    let mut b_depth = None;
    let mut a_nodes = None;
    let mut b_nodes = None;
    let mut a_time = None;
    let mut b_time = None;
    let mut a_evaluator = None;
    let mut b_evaluator = None;
    let mut games = 8;
    let mut opening_plies = 6;
    let mut max_plies = 160;
    let mut seed = 1;
    let mut show_games = false;
    let mut index = 0;
    while index < args.len() {
        if args[index] == "--show-games" {
            show_games = true;
            index += 1;
            continue;
        }
        if args[index] == "--help" || args[index] == "-h" {
            println!("{}", usage());
            std::process::exit(0);
        }
        let value = args
            .get(index + 1)
            .ok_or_else(|| format!("{} needs a value", args[index]))?;
        match args[index].as_str() {
            "--a" => a_name = value.clone(),
            "--b" => b_name = value.clone(),
            "--a-depth" => a_depth = Some(parse(value, "A depth")?),
            "--b-depth" => b_depth = Some(parse(value, "B depth")?),
            "--a-nodes" => a_nodes = Some(parse(value, "A nodes")?),
            "--b-nodes" => b_nodes = Some(parse(value, "B nodes")?),
            "--a-time" => a_time = Some(parse(value, "A time")?),
            "--b-time" => b_time = Some(parse(value, "B time")?),
            "--a-evaluator" => a_evaluator = Some(parse_evaluator(value)?),
            "--b-evaluator" => b_evaluator = Some(parse_evaluator(value)?),
            "--games" => games = parse(value, "games")?,
            "--opening-plies" => opening_plies = parse(value, "opening plies")?,
            "--max-plies" => max_plies = parse(value, "maximum plies")?,
            "--seed" => seed = parse(value, "seed")?,
            option => return Err(format!("unknown option {option}")),
        }
        index += 2;
    }
    if !(2..=1_000).contains(&games) || games % 2 != 0 {
        return Err("games must be an even number from 2 through 1000".to_owned());
    }
    if opening_plies > 30 {
        return Err("opening plies must be from 0 through 30".to_owned());
    }
    if !(20..=500).contains(&max_plies) {
        return Err("maximum plies must be from 20 through 500".to_owned());
    }

    let mut a = profile(&a_name)?;
    let mut b = profile(&b_name)?;
    apply_overrides(&mut a, a_depth, a_nodes, a_time, a_evaluator);
    apply_overrides(&mut b, b_depth, b_nodes, b_time, b_evaluator);
    validate(&a)?;
    validate(&b)?;
    Ok(Config {
        a,
        b,
        games,
        opening_plies,
        max_plies,
        seed,
        show_games,
    })
}

fn profile(name: &str) -> Result<Player, String> {
    let search = search_preset(name)
        .ok_or_else(|| format!("unknown profile {name}; use {}", preset_names()))?
        .config;
    Ok(Player {
        name: name.to_owned(),
        search,
    })
}

fn apply_overrides(
    player: &mut Player,
    depth: Option<u8>,
    nodes: Option<u64>,
    time: Option<u64>,
    evaluator: Option<EvaluationProfile>,
) {
    if let Some(depth) = depth {
        player.search.depth = depth;
    }
    if let Some(nodes) = nodes {
        player.search.nodes = Some(nodes);
    }
    if let Some(time) = time {
        player.search.time_millis = Some(time);
    }
    if let Some(evaluator) = evaluator {
        player.search.evaluator = evaluator;
    }
}

fn preset_names() -> String {
    SEARCH_PRESETS
        .iter()
        .map(|preset| preset.name)
        .collect::<Vec<_>>()
        .join(", ")
}

fn parse_evaluator(value: &str) -> Result<EvaluationProfile, String> {
    value.parse().map_err(|message: &str| message.to_owned())
}

fn validate(player: &Player) -> Result<(), String> {
    if !(1..=64).contains(&player.search.depth) {
        return Err(format!("{} depth must be from 1 through 64", player.name));
    }
    if !matches!(player.search.nodes, Some(1..=10_000_000)) {
        return Err(format!(
            "{} nodes must be from 1 through 10000000",
            player.name
        ));
    }
    if !matches!(player.search.time_millis, Some(1..=1_000)) {
        return Err(format!(
            "{} time must be from 1 through 1000 milliseconds",
            player.name
        ));
    }
    Ok(())
}

fn parse<T: std::str::FromStr>(value: &str, label: &str) -> Result<T, String> {
    value
        .parse()
        .map_err(|_| format!("{label} must be a number"))
}

fn usage() -> &'static str {
    "Usage: selfplay [options]\n\n  --a PROFILE          beginner, easy, medium, hard, expert, or maximum\n  --b PROFILE          beginner, easy, medium, hard, expert, or maximum\n  --a-depth N          override A's maximum depth\n  --b-depth N          override B's maximum depth\n  --a-nodes N          override A's node budget\n  --b-nodes N          override B's node budget\n  --a-time N           override A's time limit in milliseconds (max 1000)\n  --b-time N           override B's time limit in milliseconds (max 1000)\n  --a-evaluator NAME   material, piece-square, or tiny-nnue\n  --b-evaluator NAME   material, piece-square, or tiny-nnue\n  --games N            even number of games (default 8)\n  --opening-plies N    randomized plies shared by each color pair (default 6)\n  --max-plies N        adjudicate longer games as draws (default 160)\n  --seed N             deterministic opening seed (default 1)\n  --show-games         print complete move lists"
}

struct SplitMix64(u64);

impl SplitMix64 {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut value = self.0;
        value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        value ^ (value >> 31)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_owned()).collect()
    }

    fn parse_error(values: &[&str]) -> String {
        parse_config_from(args(values))
            .err()
            .expect("arguments should be rejected")
    }

    #[test]
    fn profiles_evaluators_and_overrides_are_parsed_independently() {
        let config = parse_config_from(args(&[
            "--a",
            "easy",
            "--b",
            "beginner",
            "--a-depth",
            "4",
            "--a-nodes",
            "12345",
            "--a-time",
            "77",
            "--a-evaluator",
            "piece-square",
            "--games",
            "6",
            "--opening-plies",
            "9",
            "--max-plies",
            "120",
            "--seed",
            "99",
            "--show-games",
        ]))
        .expect("valid arguments");

        assert_eq!(config.a.name, "easy");
        assert_eq!(config.a.search.depth, 4);
        assert_eq!(config.a.search.nodes, Some(12_345));
        assert_eq!(config.a.search.time_millis, Some(77));
        assert_eq!(config.a.search.evaluator, EvaluationProfile::PieceSquare);
        assert!(!config.a.search.quiescence);
        assert!(config.a.search.move_ordering);
        assert!(config.a.search.transposition_table);
        assert_eq!(config.b.name, "beginner");
        assert_eq!(config.b.search.nodes, Some(1_000));
        assert_eq!(config.b.search.evaluator, EvaluationProfile::TinyNnue);
        assert_eq!(config.games, 6);
        assert_eq!(config.opening_plies, 9);
        assert_eq!(config.max_plies, 120);
        assert_eq!(config.seed, 99);
        assert!(config.show_games);
    }

    #[test]
    fn invalid_match_shapes_and_search_limits_are_rejected() {
        assert!(parse_error(&["--games", "3"]).contains("even number"));
        assert!(parse_error(&["--opening-plies", "31"]).contains("0 through 30"));
        assert!(parse_error(&["--max-plies", "19"]).contains("20 through 500"));
        assert!(parse_error(&["--a-depth", "65"]).contains("1 through 64"));
        assert!(parse_error(&["--a-nodes", "0"]).contains("1 through 10000000"));
        assert!(parse_error(&["--a-time", "1001"]).contains("1 through 1000"));
        assert!(parse_error(&["--a", "missing"]).contains("unknown profile"));
        assert!(parse_error(&["--missing", "1"]).contains("unknown option"));
    }

    #[test]
    fn each_opening_is_deterministic_and_shared_by_a_color_pair() {
        assert_eq!(paired_opening_seed(41, 0), 41);
        assert_eq!(paired_opening_seed(41, 1), 41);
        assert_eq!(paired_opening_seed(41, 2), 42);
        assert_eq!(paired_opening_seed(u64::MAX, 2), 0);
        assert!(a_has_first_color(0));
        assert!(!a_has_first_color(1));
        let first = opening(41, 8).1;
        let second = opening(41, 8).1;
        assert_eq!(first, second);
    }
}
