use ai_ultimate_tictactoe::{
    GameResult, MctsOptions, MctsSearcher, MctsStrategy, Player, Position, mcts_preset,
};
use std::env;
use std::io::{self, Write};
use std::process::ExitCode;
use std::time::{Duration, Instant};

#[derive(Clone, Copy)]
struct Competitor {
    name: &'static str,
    strategy: MctsStrategy,
}

const COMPETITORS: [Competitor; 4] = [
    competitor("Random UCT", MctsStrategy::UctRandom),
    competitor("Tactical UCT", MctsStrategy::UctTactical),
    competitor("Handcrafted PUCT", MctsStrategy::PuctHandcrafted),
    competitor("Learned PUCT", MctsStrategy::PuctLearned),
];

const fn competitor(name: &'static str, strategy: MctsStrategy) -> Competitor {
    Competitor { name, strategy }
}

struct Config {
    profile: String,
    options: MctsOptions,
    games_per_pair: usize,
    opening_plies: usize,
    seed: u64,
}

#[derive(Clone, Copy, Default)]
struct PairResult {
    left_wins: u32,
    right_wins: u32,
    draws: u32,
}

#[derive(Clone, Copy, Default)]
struct SearchStats {
    moves: u64,
    simulations: u64,
    elapsed: Duration,
}

#[derive(Clone, Copy, Default)]
struct Standing {
    wins: u32,
    losses: u32,
    draws: u32,
}

impl Standing {
    fn points(self) -> f64 {
        f64::from(self.wins) + f64::from(self.draws) * 0.5
    }

    fn games(self) -> u32 {
        self.wins + self.losses + self.draws
    }
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
    let config = parse_config(env::args().skip(1).collect())?;
    let pairs = COMPETITORS.len() * (COMPETITORS.len() - 1) / 2;
    let total_games = pairs * config.games_per_pair;
    println!(
        "Ultimate Tic-Tac-Toe MCTS round robin\n{} configurations · {pairs} pairings · {} games/pair · {total_games} games\n{} profile · {} simulations/move · no clock · {} opening plies · paired colors · seed {}\n",
        COMPETITORS.len(),
        config.games_per_pair,
        config.profile,
        config.options.max_simulations,
        config.opening_plies,
        config.seed,
    );

    let mut standings = [Standing::default(); COMPETITORS.len()];
    let mut timing = [SearchStats::default(); COMPETITORS.len()];
    let mut matrix = [[None; COMPETITORS.len()]; COMPETITORS.len()];
    let started = Instant::now();
    let mut completed_games = 0;
    for left in 0..COMPETITORS.len() {
        for right in left + 1..COMPETITORS.len() {
            let result = play_pair(left, right, &config, &mut timing)?;
            matrix[left][right] = Some(result);
            standings[left].wins += result.left_wins;
            standings[left].losses += result.right_wins;
            standings[left].draws += result.draws;
            standings[right].wins += result.right_wins;
            standings[right].losses += result.left_wins;
            standings[right].draws += result.draws;
            completed_games += config.games_per_pair;
            println!(
                "{:>4}/{total_games} · {:<19} {:>2}–{:>2}–{:<2} {:<19}",
                completed_games,
                COMPETITORS[left].name,
                result.left_wins,
                result.draws,
                result.right_wins,
                COMPETITORS[right].name,
            );
            io::stdout()
                .flush()
                .map_err(|error| format!("could not flush results: {error}"))?;
        }
    }

    print_standings(&standings, &timing);
    print_matrix(&matrix);
    println!("\nelapsed {:.1}s", started.elapsed().as_secs_f64());
    Ok(())
}

fn play_pair(
    left: usize,
    right: usize,
    config: &Config,
    timing: &mut [SearchStats; COMPETITORS.len()],
) -> Result<PairResult, String> {
    let mut result = PairResult::default();
    let mut left_searcher = MctsSearcher::new();
    let mut right_searcher = MctsSearcher::new();
    for game in 0..config.games_per_pair {
        let mut position = opening(paired_opening_seed(config.seed, game), config.opening_plies);
        let left_is_x = game % 2 == 0;
        while position.result() == GameResult::Ongoing {
            let left_to_move = (position.side_to_move() == Player::X) == left_is_x;
            let index = if left_to_move { left } else { right };
            let competitor = COMPETITORS[index];
            let mut options = config.options;
            options.strategy = competitor.strategy;
            options.seed ^= position.hash();
            let searcher = if left_to_move {
                &mut left_searcher
            } else {
                &mut right_searcher
            };
            let search_started = Instant::now();
            let report = searcher.search(position, options);
            timing[index].elapsed += search_started.elapsed();
            timing[index].moves += 1;
            timing[index].simulations += u64::from(report.simulations);
            let mv = report
                .best_move
                .ok_or_else(|| "search returned no move in an ongoing game".to_owned())?;
            position = position
                .play(mv)
                .map_err(|error| format!("search returned illegal move {mv}: {error}"))?;
        }

        match position.result() {
            GameResult::Win(Player::X) if left_is_x => result.left_wins += 1,
            GameResult::Win(Player::O) if !left_is_x => result.left_wins += 1,
            GameResult::Win(_) => result.right_wins += 1,
            GameResult::Draw => result.draws += 1,
            GameResult::Ongoing => unreachable!(),
        }
    }
    Ok(result)
}

fn print_standings(
    standings: &[Standing; COMPETITORS.len()],
    timing: &[SearchStats; COMPETITORS.len()],
) {
    let mut order = (0..COMPETITORS.len()).collect::<Vec<_>>();
    order.sort_by(|&left, &right| {
        standings[right]
            .points()
            .total_cmp(&standings[left].points())
            .then_with(|| standings[right].wins.cmp(&standings[left].wins))
            .then_with(|| COMPETITORS[left].name.cmp(COMPETITORS[right].name))
    });

    println!("\nStandings");
    println!(" #  configuration       W    D    L   score   sims/move  ms/move");
    for (rank, &index) in order.iter().enumerate() {
        let standing = standings[index];
        let stats = timing[index];
        let score = standing.points() * 100.0 / f64::from(standing.games());
        let simulations = stats.simulations / stats.moves.max(1);
        let milliseconds = stats.elapsed.as_secs_f64() * 1_000.0 / stats.moves.max(1) as f64;
        println!(
            "{:>2}. {:<20} {:>3}  {:>3}  {:>3}  {:>5.1}%   {:>7}  {:>7.2}",
            rank + 1,
            COMPETITORS[index].name,
            standing.wins,
            standing.draws,
            standing.losses,
            score,
            simulations,
            milliseconds,
        );
    }
}

fn print_matrix(matrix: &[[Option<PairResult>; COMPETITORS.len()]; COMPETITORS.len()]) {
    println!("\nPair results (row wins–draws–losses)");
    for (left, left_competitor) in COMPETITORS.iter().enumerate() {
        for (right, right_competitor) in COMPETITORS.iter().enumerate().skip(left + 1) {
            let result = matrix[left][right].expect("every pair was played");
            println!(
                "  {:<20} vs {:<20} {:>2}–{:>2}–{:>2}",
                left_competitor.name,
                right_competitor.name,
                result.left_wins,
                result.draws,
                result.right_wins,
            );
        }
    }
}

fn opening(seed: u64, plies: usize) -> Position {
    let mut random = SplitMix64(seed);
    let mut position = Position::start();
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
    }
    position
}

fn paired_opening_seed(seed: u64, game: usize) -> u64 {
    seed.wrapping_add((game / 2) as u64)
}

fn parse_config(args: Vec<String>) -> Result<Config, String> {
    let mut profile = "medium".to_owned();
    let mut games_per_pair = 48;
    let mut opening_plies = 6;
    let mut seed = 2_900;
    let mut index = 0;
    while index < args.len() {
        if args[index] == "--help" || args[index] == "-h" {
            println!("{}", usage());
            std::process::exit(0);
        }
        let value = args
            .get(index + 1)
            .ok_or_else(|| format!("{} needs a value", args[index]))?;
        match args[index].as_str() {
            "--profile" => profile = value.clone(),
            "--games-per-pair" => games_per_pair = parse(value, "games per pair")?,
            "--opening-plies" => opening_plies = parse(value, "opening plies")?,
            "--seed" => seed = parse(value, "seed")?,
            option => return Err(format!("unknown option {option}")),
        }
        index += 2;
    }
    if !(2..=1_000).contains(&games_per_pair) || games_per_pair % 2 != 0 {
        return Err("games per pair must be an even number from 2 through 1000".to_owned());
    }
    if opening_plies > 20 {
        return Err("opening plies must be from 0 through 20".to_owned());
    }
    let mut options = mcts_preset(&profile)
        .ok_or_else(|| format!("unknown MCTS profile {profile}"))?
        .options;
    options.soft_time_ms = 0;
    Ok(Config {
        profile,
        options,
        games_per_pair,
        opening_plies,
        seed,
    })
}

fn parse<T: std::str::FromStr>(value: &str, label: &str) -> Result<T, String> {
    value
        .parse()
        .map_err(|_| format!("{label} must be a number"))
}

fn usage() -> &'static str {
    "Usage: mcts-round-robin [options]\n\n  --profile NAME        MCTS profile (default medium)\n  --games-per-pair N    even games per pairing (default 48; 288 total)\n  --opening-plies N     shared randomized opening plies (default 6)\n  --seed N              deterministic opening seed (default 2900)"
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
    fn defaults_schedule_two_hundred_and_eighty_eight_paired_games() {
        let config = parse_config(Vec::new()).unwrap();
        let pairs = COMPETITORS.len() * (COMPETITORS.len() - 1) / 2;

        assert_eq!(config.profile, "medium");
        assert_eq!(pairs * config.games_per_pair, 288);
        assert_eq!(config.options.max_simulations, 2_000);
        assert_eq!(config.options.soft_time_ms, 0);
    }

    #[test]
    fn openings_are_shared_by_color_pairs_and_every_configuration_is_unique() {
        assert_eq!(paired_opening_seed(41, 0), paired_opening_seed(41, 1));
        assert_eq!(opening(41, 6), opening(41, 6));
        for (left, left_competitor) in COMPETITORS.iter().enumerate() {
            for right_competitor in COMPETITORS.iter().skip(left + 1) {
                assert_ne!(left_competitor.name, right_competitor.name);
            }
        }
    }

    #[test]
    fn invalid_tournament_shapes_are_rejected() {
        assert!(parse_config(args(&["--games-per-pair", "3"])).is_err());
        assert!(parse_config(args(&["--opening-plies", "21"])).is_err());
        assert!(parse_config(args(&["--profile", "missing"])).is_err());
    }
}
