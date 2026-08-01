use ai_ultimate_tictactoe::{
    GameResult, Move, Player, Position, SEARCH_PRESETS, SearchOptions, Searcher, search_preset,
};
use std::env;
use std::process::ExitCode;
use std::time::{Duration, Instant};

#[derive(Clone)]
struct Limit {
    name: String,
    search: SearchOptions,
}

#[derive(Default)]
struct PlayerStats {
    moves: u64,
    nodes: u64,
    depth: u64,
    elapsed: Duration,
}

struct Config {
    a: Limit,
    b: Limit,
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
        "A: {} (depth {}, {:>7} nodes, {:>4} ms)\nB: {} (depth {}, {:>7} nodes, {:>4} ms)\n{} games · {} random opening plies · paired colors · seed {}\n",
        config.a.name,
        config.a.search.max_depth,
        config.a.search.node_limit,
        config.a.search.soft_time_ms,
        config.b.name,
        config.b.search.max_depth,
        config.b.search.node_limit,
        config.b.search.soft_time_ms,
        config.games,
        config.opening_plies,
        config.seed,
    );

    let mut a_wins = 0;
    let mut b_wins = 0;
    let mut draws = 0;
    let mut total_plies = 0_u64;
    let mut a_stats = PlayerStats::default();
    let mut b_stats = PlayerStats::default();

    for game in 0..config.games {
        let opening_seed = paired_opening_seed(config.seed, game);
        let (mut position, mut history) = opening(opening_seed, config.opening_plies);
        let a_is_x = a_has_first_color(game);
        let mut a_searcher = Searcher::new();
        let mut b_searcher = Searcher::new();

        while position.result() == GameResult::Ongoing {
            let a_to_move = (position.side_to_move() == Player::X) == a_is_x;
            let (searcher, limit, stats) = if a_to_move {
                (&mut a_searcher, &config.a, &mut a_stats)
            } else {
                (&mut b_searcher, &config.b, &mut b_stats)
            };
            let started = Instant::now();
            let report = searcher.search(position, limit.search);
            stats.elapsed += started.elapsed();
            stats.moves += 1;
            stats.nodes += report.nodes;
            stats.depth += u64::from(report.depth);
            let mv = report
                .best_move
                .ok_or_else(|| "search returned no move in an ongoing game".to_owned())?;
            position = position
                .play(mv)
                .map_err(|error| format!("search returned illegal move {mv}: {error}"))?;
            history.push(mv);
        }

        total_plies += u64::from(position.ply());
        let outcome = match position.result() {
            GameResult::Win(Player::X) if a_is_x => {
                a_wins += 1;
                "A wins"
            }
            GameResult::Win(Player::O) if !a_is_x => {
                a_wins += 1;
                "A wins"
            }
            GameResult::Win(_) => {
                b_wins += 1;
                "B wins"
            }
            GameResult::Draw => {
                draws += 1;
                "draw"
            }
            GameResult::Ongoing => unreachable!(),
        };

        if config.show_games {
            println!(
                "{:>3}. A as {} · {:>6} · {:>2} plies · {}",
                game + 1,
                if a_is_x { "X" } else { "O" },
                outcome,
                position.ply(),
                history
                    .iter()
                    .map(Move::to_string)
                    .collect::<Vec<_>>()
                    .join(" "),
            );
        } else {
            println!(
                "{:>3}. A as {} · {:>6} · {:>2} plies",
                game + 1,
                if a_is_x { "X" } else { "O" },
                outcome,
                position.ply(),
            );
        }
    }

    println!(
        "\nscore  A {} · B {} · draws {}\naverage game {:.1} plies",
        a_wins,
        b_wins,
        draws,
        total_plies as f64 / config.games as f64,
    );
    print_stats("A", &a_stats);
    print_stats("B", &b_stats);
    Ok(())
}

fn opening(seed: u64, plies: usize) -> (Position, Vec<Move>) {
    let mut random = SplitMix64(seed);
    let mut position = Position::start();
    let mut history = Vec::with_capacity(81);
    for _ in 0..plies {
        let moves = position.legal_moves();
        if moves.is_empty() {
            break;
        }
        let index = (random.next() as usize) % moves.len();
        let mv = moves.iter().nth(index).expect("index is inside move list");
        position = position
            .play(mv)
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

fn print_stats(label: &str, stats: &PlayerStats) {
    let moves = stats.moves.max(1);
    println!(
        "{label}      {:>8.0} nodes/move · depth {:>4.1} · {:>6.2} ms/move · {:.2} Mnps",
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
    let mut games = 8;
    let mut opening_plies = 4;
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
            "--a-time" => a_time = Some(parse(value, "A soft time")?),
            "--b-time" => b_time = Some(parse(value, "B soft time")?),
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
    if opening_plies > 20 {
        return Err("opening plies must be from 0 through 20".to_owned());
    }

    let mut a = profile(&a_name)?;
    let mut b = profile(&b_name)?;
    if let Some(depth) = a_depth {
        a.search.max_depth = depth;
    }
    if let Some(depth) = b_depth {
        b.search.max_depth = depth;
    }
    if let Some(nodes) = a_nodes {
        a.search.node_limit = nodes;
    }
    if let Some(nodes) = b_nodes {
        b.search.node_limit = nodes;
    }
    if let Some(milliseconds) = a_time {
        a.search.soft_time_ms = milliseconds;
    }
    if let Some(milliseconds) = b_time {
        b.search.soft_time_ms = milliseconds;
    }
    validate_limit(&a)?;
    validate_limit(&b)?;
    Ok(Config {
        a,
        b,
        games,
        opening_plies,
        seed,
        show_games,
    })
}

fn profile(name: &str) -> Result<Limit, String> {
    let search = search_preset(name)
        .ok_or_else(|| {
            format!(
                "unknown profile {name}; use {}",
                SEARCH_PRESETS
                    .iter()
                    .map(|preset| preset.name)
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        })?
        .options;
    Ok(Limit {
        name: name.to_owned(),
        search,
    })
}

fn validate_limit(limit: &Limit) -> Result<(), String> {
    if !(1..=20).contains(&limit.search.max_depth) {
        return Err(format!("{} depth must be from 1 through 20", limit.name));
    }
    if !(1..=10_000_000).contains(&limit.search.node_limit) {
        return Err(format!(
            "{} nodes must be from 1 through 10000000",
            limit.name
        ));
    }
    if limit.search.soft_time_ms > 1_000 {
        return Err(format!(
            "{} soft time must be no more than 1000 milliseconds",
            limit.name
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
    "Usage: selfplay [options]\n\n  --a PROFILE          beginner, easy, medium, hard, expert, or maximum\n  --b PROFILE          beginner, easy, medium, hard, expert, or maximum\n  --a-depth N          override A's maximum depth\n  --b-depth N          override B's maximum depth\n  --a-nodes N          override A's node ceiling\n  --b-nodes N          override B's node ceiling\n  --a-time N           override A's soft time in milliseconds\n  --b-time N           override B's soft time in milliseconds\n  --games N            number of games (default 8)\n  --opening-plies N    randomized plies shared by each color pair (default 4)\n  --seed N             deterministic opening seed (default 1)\n  --show-games         print complete move lists"
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
    fn profiles_and_overrides_are_parsed_without_changing_the_other_player() {
        let config = parse_config_from(args(&[
            "--a",
            "hard",
            "--b",
            "easy",
            "--a-depth",
            "6",
            "--a-nodes",
            "12345",
            "--a-time",
            "99",
            "--games",
            "6",
            "--opening-plies",
            "7",
            "--seed",
            "99",
            "--show-games",
        ]))
        .expect("valid arguments");

        assert_eq!(config.a.name, "hard");
        assert_eq!(
            config.a.search,
            SearchOptions {
                max_depth: 6,
                node_limit: 12_345,
                soft_time_ms: 99,
            }
        );
        assert_eq!(config.b.name, "easy");
        assert_eq!(config.b.search, search_preset("easy").unwrap().options);
        assert_eq!(config.games, 6);
        assert_eq!(config.opening_plies, 7);
        assert_eq!(config.seed, 99);
        assert!(config.show_games);
    }

    #[test]
    fn invalid_match_shapes_and_search_limits_are_rejected() {
        assert!(parse_error(&["--games", "3"]).contains("even number"));
        assert!(parse_error(&["--opening-plies", "21"]).contains("0 through 20"));
        assert!(parse_error(&["--a-depth", "21"]).contains("1 through 20"));
        assert!(parse_error(&["--a-nodes", "0"]).contains("1 through 10000000"));
        assert!(parse_error(&["--a-time", "1001"]).contains("no more than 1000"));
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
