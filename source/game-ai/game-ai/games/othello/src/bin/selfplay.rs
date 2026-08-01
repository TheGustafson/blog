use ai_othello::{
    EvaluationProfile, GameResult, Move, Position, SEARCH_PRESETS, SearchPreset, Side, search,
    search_preset,
};
use std::env;
use std::process::ExitCode;
use std::time::{Duration, Instant};

#[derive(Clone)]
struct Player {
    name: String,
    preset: SearchPreset,
    evaluator: EvaluationProfile,
}

#[derive(Default)]
struct Stats {
    moves: u64,
    nodes: u64,
    depth: u64,
    exact_nodes: u64,
    elapsed: Duration,
}

struct Config {
    a: Player,
    b: Player,
    games: usize,
    opening_plies: usize,
    seed: u64,
    show_games: bool,
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
    println!(
        "A: {} (depth {}, exact at {}, {})\nB: {} (depth {}, exact at {}, {})\n{} games · {} random opening plies · paired colors · seed {}\n",
        config.a.name,
        config.a.preset.depth,
        config.a.preset.exact_endgame_empties,
        config.a.evaluator,
        config.b.name,
        config.b.preset.depth,
        config.b.preset.exact_endgame_empties,
        config.b.evaluator,
        config.games,
        config.opening_plies,
        config.seed,
    );

    let mut a_wins = 0;
    let mut b_wins = 0;
    let mut draws = 0;
    let mut total_plies = 0_u64;
    let mut a_stats = Stats::default();
    let mut b_stats = Stats::default();

    for game in 0..config.games {
        let opening_seed = paired_opening_seed(config.seed, game);
        let (mut position, mut history) = opening(opening_seed, config.opening_plies);
        let a_is_black = a_has_first_color(game);

        while position.result() == GameResult::Ongoing {
            let a_to_move = (position.side_to_move() == Side::Black) == a_is_black;
            let (player, stats) = if a_to_move {
                (&config.a, &mut a_stats)
            } else {
                (&config.b, &mut b_stats)
            };
            let started = Instant::now();
            let report = search(position, player.preset.config(player.evaluator));
            stats.elapsed += started.elapsed();
            stats.moves += 1;
            stats.nodes += report.stats.nodes;
            stats.depth += u64::from(report.config.depth);
            stats.exact_nodes += report.stats.exact_nodes;
            let mv = report
                .best_move
                .ok_or_else(|| "search returned no move in an ongoing game".to_owned())?;
            position
                .make_move(mv)
                .map_err(|error| format!("search returned illegal move {mv}: {error}"))?;
            history.push(mv);
        }

        total_plies += history.len() as u64;
        let outcome = match position.result() {
            GameResult::Win { winner, .. } if (winner == Side::Black) == a_is_black => {
                a_wins += 1;
                "A wins"
            }
            GameResult::Win { .. } => {
                b_wins += 1;
                "B wins"
            }
            GameResult::Draw { .. } => {
                draws += 1;
                "draw"
            }
            GameResult::Ongoing => unreachable!(),
        };
        if config.show_games {
            let moves = history
                .iter()
                .map(Move::to_string)
                .collect::<Vec<_>>()
                .join(" ");
            println!(
                "{:>3}. A as {} · {:>6} · {:>2} plies · {moves}",
                game + 1,
                if a_is_black { "B" } else { "W" },
                outcome,
                history.len(),
            );
        } else {
            println!(
                "{:>3}. A as {} · {:>6} · {:>2} plies",
                game + 1,
                if a_is_black { "B" } else { "W" },
                outcome,
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

fn opening(seed: u64, plies: usize) -> (Position, Vec<Move>) {
    let mut random = SplitMix64(seed);
    let mut position = Position::start();
    let mut history = Vec::with_capacity(64);
    for _ in 0..plies {
        let moves: Vec<_> = position.legal_moves().into_iter().collect();
        if moves.is_empty() {
            break;
        }
        let mv = moves[(random.next() as usize) % moves.len()];
        position
            .make_move(mv)
            .expect("opening move was generated as legal");
        history.push(mv);
    }
    (position, history)
}

fn paired_opening_seed(seed: u64, game: usize) -> u64 {
    seed.wrapping_add((game / 2) as u64)
}

fn a_has_first_color(game: usize) -> bool {
    game % 2 == 0
}

fn print_stats(label: &str, stats: &Stats) {
    let moves = stats.moves.max(1);
    println!(
        "{label}      {:>9.0} nodes/move · depth {:>3.1} · {:>7.0} exact nodes/move · {:>6.2} ms/move · {:.2} Mnps",
        stats.nodes as f64 / moves as f64,
        stats.depth as f64 / moves as f64,
        stats.exact_nodes as f64 / moves as f64,
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
    let mut a_endgame = None;
    let mut b_endgame = None;
    let mut a_evaluator = EvaluationProfile::Phase;
    let mut b_evaluator = EvaluationProfile::Phase;
    let mut games = 8;
    let mut opening_plies = 8;
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
            "--a-endgame" => a_endgame = Some(parse(value, "A endgame threshold")?),
            "--b-endgame" => b_endgame = Some(parse(value, "B endgame threshold")?),
            "--a-evaluator" => {
                a_evaluator = value.parse().map_err(|message: &str| message.to_owned())?
            }
            "--b-evaluator" => {
                b_evaluator = value.parse().map_err(|message: &str| message.to_owned())?
            }
            "--games" => games = parse(value, "games")?,
            "--opening-plies" => opening_plies = parse(value, "opening plies")?,
            "--seed" => seed = parse(value, "seed")?,
            option => return Err(format!("unknown option {option}")),
        }
        index += 2;
    }
    if !(2..=1_000).contains(&games) || games % 2 != 0 {
        return Err("games must be an even number from 2 through 1000".to_owned());
    }
    if opening_plies > 40 {
        return Err("opening plies must be from 0 through 40".to_owned());
    }

    let mut a = profile(&a_name, a_evaluator)?;
    let mut b = profile(&b_name, b_evaluator)?;
    if let Some(depth) = a_depth {
        a.preset.depth = depth;
    }
    if let Some(depth) = b_depth {
        b.preset.depth = depth;
    }
    if let Some(endgame) = a_endgame {
        a.preset.exact_endgame_empties = endgame;
    }
    if let Some(endgame) = b_endgame {
        b.preset.exact_endgame_empties = endgame;
    }
    validate(&a)?;
    validate(&b)?;
    Ok(Config {
        a,
        b,
        games,
        opening_plies,
        seed,
        show_games,
    })
}

fn profile(name: &str, evaluator: EvaluationProfile) -> Result<Player, String> {
    let preset = search_preset(name)
        .ok_or_else(|| format!("unknown profile {name}; use {}", preset_names()))?;
    Ok(Player {
        name: name.to_owned(),
        preset,
        evaluator,
    })
}

fn preset_names() -> String {
    SEARCH_PRESETS
        .iter()
        .map(|preset| preset.name)
        .collect::<Vec<_>>()
        .join(", ")
}

fn validate(player: &Player) -> Result<(), String> {
    if !(1..=16).contains(&player.preset.depth) {
        return Err(format!("{} depth must be from 1 through 16", player.name));
    }
    if player.preset.exact_endgame_empties > 16 {
        return Err(format!(
            "{} endgame threshold must be from 0 through 16",
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
    "Usage: selfplay [options]\n\n  --a PROFILE          beginner, easy, medium, hard, expert, or maximum\n  --b PROFILE          beginner, easy, medium, hard, expert, or maximum\n  --a-depth N          override A's search depth\n  --b-depth N          override B's search depth\n  --a-endgame N        override A's exact-endgame empty count\n  --b-endgame N        override B's exact-endgame empty count\n  --a-evaluator NAME   material, mobility, corners, frontier, or phase\n  --b-evaluator NAME   material, mobility, corners, frontier, or phase\n  --games N            even number of games (default 8)\n  --opening-plies N    randomized plies shared by each color pair (default 8)\n  --seed N             deterministic opening seed (default 1)\n  --show-games         print complete move lists"
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
            "hard",
            "--b",
            "easy",
            "--a-depth",
            "6",
            "--a-endgame",
            "7",
            "--a-evaluator",
            "corners",
            "--games",
            "6",
            "--opening-plies",
            "9",
            "--seed",
            "99",
            "--show-games",
        ]))
        .expect("valid arguments");

        assert_eq!(config.a.name, "hard");
        assert_eq!(config.a.preset.depth, 6);
        assert_eq!(config.a.preset.exact_endgame_empties, 7);
        assert_eq!(config.a.evaluator, EvaluationProfile::Corners);
        assert_eq!(config.b.name, "easy");
        assert_eq!(config.b.preset.depth, 2);
        assert_eq!(config.b.preset.exact_endgame_empties, 0);
        assert_eq!(config.b.evaluator, EvaluationProfile::Phase);
        assert_eq!(config.games, 6);
        assert_eq!(config.opening_plies, 9);
        assert_eq!(config.seed, 99);
        assert!(config.show_games);
    }

    #[test]
    fn invalid_match_shapes_and_search_limits_are_rejected() {
        assert!(parse_error(&["--games", "3"]).contains("even number"));
        assert!(parse_error(&["--opening-plies", "41"]).contains("0 through 40"));
        assert!(parse_error(&["--a-depth", "0"]).contains("1 through 16"));
        assert!(parse_error(&["--a-endgame", "17"]).contains("0 through 16"));
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
