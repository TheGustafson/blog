use ai_ultimate_tictactoe::{
    GameResult, MCTS_PRESETS, MctsOptions, MctsSearcher, MctsStrategy, Move, Player, Position,
    SEARCH_PRESETS, SearchOptions, Searcher, mcts_preset, search_preset,
};
use std::env;
use std::process::ExitCode;
use std::time::{Duration, Instant};

struct Config {
    mcts_name: String,
    mcts: MctsOptions,
    opponent: Opponent,
    alpha_name: String,
    alpha: SearchOptions,
    games: usize,
    opening_plies: usize,
    seed: u64,
    show_games: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Opponent {
    AlphaBeta,
    HandcraftedPuct,
    LearnedPuct,
    UctRandom,
    UctTactical,
}

#[derive(Default)]
struct Stats {
    moves: u64,
    work: u64,
    depth: u64,
    elapsed: Duration,
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
        "MCTS: {} / {} ({:>7} simulations, {:>4} ms, C {:.3})\nopponent: {}\n{} games · {} random opening plies · paired colors · seed {}\n",
        config.mcts_name,
        config.mcts.strategy.name(),
        config.mcts.max_simulations,
        config.mcts.soft_time_ms,
        config.mcts.exploration,
        opponent_description(&config),
        config.games,
        config.opening_plies,
        config.seed,
    );

    let mut candidate_wins = 0;
    let mut opponent_wins = 0;
    let mut draws = 0;
    let mut total_plies = 0_u64;
    let mut candidate_stats = Stats::default();
    let mut opponent_stats = Stats::default();

    for game in 0..config.games {
        let opening_seed = paired_opening_seed(config.seed, game);
        let (mut position, mut history) = opening(opening_seed, config.opening_plies);
        let candidate_is_x = game % 2 == 0;
        let mut candidate = MctsSearcher::new();
        let mut opponent_mcts = MctsSearcher::new();
        let mut alpha = Searcher::new();

        while position.result() == GameResult::Ongoing {
            let candidate_to_move = (position.side_to_move() == Player::X) == candidate_is_x;
            let started = Instant::now();
            let mv = if candidate_to_move {
                let mut options = config.mcts;
                options.seed ^= position.hash();
                let report = candidate.search(position, options);
                candidate_stats.elapsed += started.elapsed();
                candidate_stats.moves += 1;
                candidate_stats.work += u64::from(report.simulations);
                report.best_move
            } else {
                match config.opponent {
                    Opponent::AlphaBeta => {
                        let report = alpha.search(position, config.alpha);
                        opponent_stats.elapsed += started.elapsed();
                        opponent_stats.moves += 1;
                        opponent_stats.work += report.nodes;
                        opponent_stats.depth += u64::from(report.depth);
                        report.best_move
                    }
                    Opponent::HandcraftedPuct
                    | Opponent::LearnedPuct
                    | Opponent::UctRandom
                    | Opponent::UctTactical => {
                        let mut options = config.mcts;
                        options.strategy = opponent_strategy(config.opponent);
                        options.seed ^= position.hash();
                        let report = opponent_mcts.search(position, options);
                        opponent_stats.elapsed += started.elapsed();
                        opponent_stats.moves += 1;
                        opponent_stats.work += u64::from(report.simulations);
                        report.best_move
                    }
                }
            }
            .ok_or_else(|| "search returned no move in an ongoing game".to_owned())?;
            position = position
                .play(mv)
                .map_err(|error| format!("search returned illegal move {mv}: {error}"))?;
            history.push(mv);
        }

        total_plies += u64::from(position.ply());
        let outcome = match position.result() {
            GameResult::Win(Player::X) if candidate_is_x => {
                candidate_wins += 1;
                "candidate wins"
            }
            GameResult::Win(Player::O) if !candidate_is_x => {
                candidate_wins += 1;
                "candidate wins"
            }
            GameResult::Win(_) => {
                opponent_wins += 1;
                "opponent wins"
            }
            GameResult::Draw => {
                draws += 1;
                "draw"
            }
            GameResult::Ongoing => unreachable!(),
        };

        println!(
            "{:>3}. candidate as {} · {:>13} · {:>2} plies{}",
            game + 1,
            if candidate_is_x { "X" } else { "O" },
            outcome,
            position.ply(),
            if config.show_games {
                format!(
                    " · {}",
                    history
                        .iter()
                        .map(Move::to_string)
                        .collect::<Vec<_>>()
                        .join(" ")
                )
            } else {
                String::new()
            },
        );
    }

    println!(
        "\nscore  candidate {} · opponent {} · draws {}\naverage game {:.1} plies",
        candidate_wins,
        opponent_wins,
        draws,
        total_plies as f64 / config.games as f64,
    );
    print_stats("candidate", "simulations", &candidate_stats);
    print_stats(
        "opponent",
        if config.opponent == Opponent::AlphaBeta {
            "nodes"
        } else {
            "simulations"
        },
        &opponent_stats,
    );
    Ok(())
}

fn opponent_description(config: &Config) -> String {
    match config.opponent {
        Opponent::AlphaBeta => format!(
            "alpha-beta {} (depth {}, {:>7} nodes, {:>4} ms)",
            config.alpha_name,
            config.alpha.max_depth,
            config.alpha.node_limit,
            config.alpha.soft_time_ms,
        ),
        Opponent::HandcraftedPuct => format!(
            "{} / PUCT + handcrafted priors ({:>7} simulations, {:>4} ms, C {:.3})",
            config.mcts_name,
            config.mcts.max_simulations,
            config.mcts.soft_time_ms,
            config.mcts.exploration,
        ),
        Opponent::LearnedPuct => format!(
            "{} / PUCT + learned priors ({:>7} simulations, {:>4} ms, C {:.3})",
            config.mcts_name,
            config.mcts.max_simulations,
            config.mcts.soft_time_ms,
            config.mcts.exploration,
        ),
        Opponent::UctRandom | Opponent::UctTactical => format!(
            "{} / UCT + {} ({:>7} simulations, {:>4} ms, C {:.3})",
            config.mcts_name,
            if config.opponent == Opponent::UctRandom {
                "random rollouts"
            } else {
                "tactical rollouts"
            },
            config.mcts.max_simulations,
            config.mcts.soft_time_ms,
            config.mcts.exploration,
        ),
    }
}

fn opponent_strategy(opponent: Opponent) -> MctsStrategy {
    match opponent {
        Opponent::HandcraftedPuct => MctsStrategy::PuctHandcrafted,
        Opponent::LearnedPuct => MctsStrategy::PuctLearned,
        Opponent::UctRandom => MctsStrategy::UctRandom,
        Opponent::UctTactical => MctsStrategy::UctTactical,
        Opponent::AlphaBeta => unreachable!("alpha-beta does not use an MCTS strategy"),
    }
}

fn print_stats(label: &str, unit: &str, stats: &Stats) {
    let moves = stats.moves.max(1);
    let depth = if stats.depth == 0 {
        String::new()
    } else {
        format!(" · depth {:>4.1}", stats.depth as f64 / moves as f64)
    };
    println!(
        "{label:<5} {:>9.0} {unit}/move{depth} · {:>6.2} ms/move",
        stats.work as f64 / moves as f64,
        stats.elapsed.as_secs_f64() * 1_000.0 / moves as f64,
    );
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
        let mv = moves
            .iter()
            .nth((random.next() as usize) % moves.len())
            .expect("opening index is inside the move list");
        position = position.play(mv).expect("opening moves are legal");
        history.push(mv);
    }
    (position, history)
}

fn paired_opening_seed(seed: u64, game: usize) -> u64 {
    seed.wrapping_add((game / 2) as u64)
}

fn parse_config() -> Result<Config, String> {
    parse_config_from(env::args().skip(1).collect())
}

fn parse_config_from(args: Vec<String>) -> Result<Config, String> {
    let mut mcts_name = "maximum".to_owned();
    let mut alpha_name = "maximum".to_owned();
    let mut strategy = MctsStrategy::PuctLearned;
    let mut opponent = Opponent::AlphaBeta;
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
            "--mcts" => mcts_name = value.clone(),
            "--alpha" => alpha_name = value.clone(),
            "--strategy" => strategy = parse_strategy(value)?,
            "--opponent" => {
                opponent = match value.as_str() {
                    "alpha" => Opponent::AlphaBeta,
                    "handcrafted" => Opponent::HandcraftedPuct,
                    "learned" => Opponent::LearnedPuct,
                    "random" => Opponent::UctRandom,
                    "tactical" => Opponent::UctTactical,
                    _ => {
                        return Err(
                            "opponent must be alpha, handcrafted, learned, random, or tactical"
                                .to_owned(),
                        );
                    }
                }
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
    if opening_plies > 20 {
        return Err("opening plies must be from 0 through 20".to_owned());
    }
    let mut mcts = mcts_preset(&mcts_name)
        .ok_or_else(|| unknown_profile("MCTS", &mcts_name, MCTS_PRESETS.map(|preset| preset.name)))?
        .options;
    mcts.strategy = strategy;
    let alpha = search_preset(&alpha_name)
        .ok_or_else(|| {
            unknown_profile(
                "alpha-beta",
                &alpha_name,
                SEARCH_PRESETS.map(|preset| preset.name),
            )
        })?
        .options;
    Ok(Config {
        mcts_name,
        mcts,
        opponent,
        alpha_name,
        alpha,
        games,
        opening_plies,
        seed,
        show_games,
    })
}

fn unknown_profile(label: &str, name: &str, profiles: [&str; 6]) -> String {
    format!(
        "unknown {label} profile {name}; use {}",
        profiles.join(", ")
    )
}

fn parse<T: std::str::FromStr>(value: &str, label: &str) -> Result<T, String> {
    value
        .parse()
        .map_err(|_| format!("{label} must be a number"))
}

fn parse_strategy(value: &str) -> Result<MctsStrategy, String> {
    match value {
        "random-uct" => Ok(MctsStrategy::UctRandom),
        "tactical-uct" => Ok(MctsStrategy::UctTactical),
        "handcrafted-puct" => Ok(MctsStrategy::PuctHandcrafted),
        "learned-puct" => Ok(MctsStrategy::PuctLearned),
        _ => Err(
            "strategy must be random-uct, tactical-uct, handcrafted-puct, or learned-puct"
                .to_owned(),
        ),
    }
}

fn usage() -> &'static str {
    "Usage: mcts-match [options]\n\n  --mcts PROFILE       MCTS browser profile (default maximum)\n  --strategy STRATEGY  random-uct, tactical-uct, handcrafted-puct, or learned-puct\n  --opponent ENGINE    opponent: alpha, handcrafted, learned, random, or tactical\n  --alpha PROFILE      alpha-beta browser profile (default maximum)\n  --games N            even number of games (default 8)\n  --opening-plies N    randomized plies shared by each color pair (default 4)\n  --seed N             deterministic opening and MCTS seed (default 1)\n  --show-games         print complete move lists"
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

    #[test]
    fn profiles_and_match_shape_are_parsed() {
        let config = parse_config_from(args(&[
            "--mcts",
            "hard",
            "--alpha",
            "medium",
            "--strategy",
            "random-uct",
            "--opponent",
            "random",
            "--games",
            "6",
            "--opening-plies",
            "8",
            "--seed",
            "41",
            "--show-games",
        ]))
        .unwrap();
        let mut expected_mcts = mcts_preset("hard").unwrap().options;
        expected_mcts.strategy = MctsStrategy::UctRandom;
        assert_eq!(config.mcts, expected_mcts);
        assert_eq!(config.opponent, Opponent::UctRandom);
        assert_eq!(config.alpha, search_preset("medium").unwrap().options);
        assert_eq!(config.games, 6);
        assert_eq!(config.opening_plies, 8);
        assert_eq!(config.seed, 41);
        assert!(config.show_games);
    }

    #[test]
    fn invalid_profiles_and_unpaired_games_are_rejected() {
        assert!(parse_config_from(args(&["--games", "3"])).is_err());
        assert!(parse_config_from(args(&["--mcts", "missing"])).is_err());
        assert!(parse_config_from(args(&["--alpha", "missing"])).is_err());
        assert!(parse_config_from(args(&["--strategy", "missing"])).is_err());
        assert!(parse_config_from(args(&["--opponent", "missing"])).is_err());
        assert!(parse_config_from(args(&["--opening-plies", "21"])).is_err());
    }

    #[test]
    fn learned_policy_is_the_default_and_an_available_opponent() {
        let default = parse_config_from(Vec::new()).unwrap();
        let learned = parse_config_from(args(&["--opponent", "learned"])).unwrap();

        assert_eq!(default.mcts.strategy, MctsStrategy::PuctLearned);
        assert_eq!(learned.opponent, Opponent::LearnedPuct);
    }

    #[test]
    fn opening_seeds_are_shared_by_color_pairs() {
        assert_eq!(paired_opening_seed(41, 0), 41);
        assert_eq!(paired_opening_seed(41, 1), 41);
        assert_eq!(paired_opening_seed(41, 2), 42);
        assert_eq!(opening(41, 8).1, opening(41, 8).1);
    }
}
