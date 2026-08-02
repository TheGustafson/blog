use ai_hex::{
    BoardSize, GameResult, KnowledgePolicy, MctsOptions, MctsSearcher, MctsStrategy, Move,
    Position, RolloutPolicy, Seat, SwapRule,
};
use std::env;
use std::process::ExitCode;

const USAGE: &str = "cargo run --release --bin selfplay -- \
    [--games N] [--size 9..24] [--simulations N] [--softtime MS] \
    [--a plain-uct|uct-rave] [--b plain-uct|uct-rave] \
    [--rave-a N] [--rave-b N] [--exploration-a N] [--exploration-b N] \
    [--rollout-a random|save-bridge] [--rollout-b random|save-bridge] \
    [--knowledge-a off|N] [--knowledge-b off|N] \
    [--connections-a on|off] [--connections-b on|off] \
    [--opening-plies N] [--seed N]";

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("error: {message}\nusage: {USAGE}");
            ExitCode::FAILURE
        }
    }
}

#[derive(Clone, Copy)]
struct Arguments {
    games: usize,
    size: BoardSize,
    simulations: u32,
    soft_time_ms: u32,
    a_strategy: MctsStrategy,
    b_strategy: MctsStrategy,
    rave_a: f64,
    rave_b: f64,
    exploration_a: f64,
    exploration_b: f64,
    rollout_a: RolloutPolicy,
    rollout_b: RolloutPolicy,
    knowledge_a: KnowledgePolicy,
    knowledge_b: KnowledgePolicy,
    connections_a: bool,
    connections_b: bool,
    opening_plies: u16,
    seed: u64,
}

impl Default for Arguments {
    fn default() -> Self {
        Self {
            games: 100,
            size: BoardSize::new(9).expect("9 is a supported size"),
            simulations: 4_000,
            soft_time_ms: 0,
            a_strategy: MctsStrategy::PlainUct,
            b_strategy: MctsStrategy::UctRave,
            rave_a: 1_000.0,
            rave_b: 1_000.0,
            exploration_a: 0.2,
            exploration_b: 0.2,
            rollout_a: RolloutPolicy::SaveBridge,
            rollout_b: RolloutPolicy::SaveBridge,
            knowledge_a: KnowledgePolicy::InferiorCells { min_visits: 32 },
            knowledge_b: KnowledgePolicy::InferiorCells { min_visits: 32 },
            connections_a: true,
            connections_b: true,
            opening_plies: 2,
            seed: 1,
        }
    }
}

#[derive(Default)]
struct Score {
    a_wins: usize,
    b_wins: usize,
    a_simulations: u64,
    b_simulations: u64,
    a_elapsed_ms: u64,
    b_elapsed_ms: u64,
    a_searches: u64,
    b_searches: u64,
    a_bridge_replies: u64,
    b_bridge_replies: u64,
    a_knowledge_nodes: u64,
    b_knowledge_nodes: u64,
    a_pruned_moves: u64,
    b_pruned_moves: u64,
    actions: u64,
}

fn run() -> Result<(), String> {
    let arguments = parse_arguments()?;
    if arguments.games == 0 || arguments.games % 2 != 0 {
        return Err("games must be a positive even number so openings can be paired".to_owned());
    }
    if arguments.opening_plies >= arguments.size.cell_count() {
        return Err("opening-plies must leave at least one empty cell".to_owned());
    }

    println!(
        "A {} (RAVE {:.0}, C {:.3}, {}, K {}, VC {}) vs B {} (RAVE {:.0}, C {:.3}, {}, K {}, VC {}) · {}×{} · {} games · {} sims / {} ms · {} opening plies",
        arguments.a_strategy.as_str(),
        arguments.rave_a,
        arguments.exploration_a,
        arguments.rollout_a.as_str(),
        knowledge_name(arguments.knowledge_a),
        on_off(arguments.connections_a),
        arguments.b_strategy.as_str(),
        arguments.rave_b,
        arguments.exploration_b,
        arguments.rollout_b.as_str(),
        knowledge_name(arguments.knowledge_b),
        on_off(arguments.connections_b),
        arguments.size.get(),
        arguments.size.get(),
        arguments.games,
        arguments.simulations,
        arguments.soft_time_ms,
        arguments.opening_plies,
    );

    let mut score = Score::default();
    for pair in 0..arguments.games / 2 {
        let opening = random_opening(arguments, pair as u64)?;
        play_game(arguments, pair * 2, &opening, true, &mut score)?;
        play_game(arguments, pair * 2 + 1, &opening, false, &mut score)?;
        if (pair + 1) % 5 == 0 || pair + 1 == arguments.games / 2 {
            println!(
                "{:>4}/{} · A {} – B {}",
                (pair + 1) * 2,
                arguments.games,
                score.a_wins,
                score.b_wins,
            );
        }
    }

    let total_searches =
        score.actions - u64::from(arguments.opening_plies) * arguments.games as u64;
    println!("\nA {} · B {}", score.a_wins, score.b_wins);
    println!(
        "Average game {:.1} actions · {:.0} searches",
        score.actions as f64 / arguments.games as f64,
        total_searches as f64 / arguments.games as f64,
    );
    print_throughput("A", &score, true);
    print_throughput("B", &score, false);
    Ok(())
}

fn play_game(
    arguments: Arguments,
    game: usize,
    opening: &[Move],
    a_is_one: bool,
    score: &mut Score,
) -> Result<(), String> {
    let mut position = Position::from_moves(arguments.size, SwapRule::Enabled, opening)
        .map_err(|error| format!("invalid generated opening: {error}"))?;
    let mut searcher = MctsSearcher::new();
    while position.result() == GameResult::Ongoing {
        let a_to_move = (position.seat_to_move() == Seat::One) == a_is_one;
        let options = MctsOptions {
            max_simulations: arguments.simulations,
            soft_time_ms: arguments.soft_time_ms,
            exploration: if a_to_move {
                arguments.exploration_a
            } else {
                arguments.exploration_b
            },
            strategy: if a_to_move {
                arguments.a_strategy
            } else {
                arguments.b_strategy
            },
            rave_equivalence: if a_to_move {
                arguments.rave_a
            } else {
                arguments.rave_b
            },
            rollout_policy: if a_to_move {
                arguments.rollout_a
            } else {
                arguments.rollout_b
            },
            knowledge_policy: if a_to_move {
                arguments.knowledge_a
            } else {
                arguments.knowledge_b
            },
            use_virtual_connections: if a_to_move {
                arguments.connections_a
            } else {
                arguments.connections_b
            },
            seed: arguments.seed
                ^ (game as u64).wrapping_mul(0x9e37_79b9)
                ^ u64::from(position.actions()).wrapping_mul(0x85eb_ca6b),
        };
        let report = searcher.search(position, options);
        if a_to_move {
            score.a_simulations += u64::from(report.simulations);
            score.a_elapsed_ms += u64::from(report.elapsed_ms);
            score.a_searches += 1;
            score.a_bridge_replies += report.bridge_replies;
            score.a_knowledge_nodes += u64::from(report.knowledge_nodes);
            score.a_pruned_moves += u64::from(report.pruned_moves);
        } else {
            score.b_simulations += u64::from(report.simulations);
            score.b_elapsed_ms += u64::from(report.elapsed_ms);
            score.b_searches += 1;
            score.b_bridge_replies += report.bridge_replies;
            score.b_knowledge_nodes += u64::from(report.knowledge_nodes);
            score.b_pruned_moves += u64::from(report.pruned_moves);
        }
        let mv = report
            .best_move
            .ok_or_else(|| "search returned no move".to_owned())?;
        position = position
            .play(mv)
            .map_err(|error| format!("search returned illegal move {mv}: {error}"))?;
    }
    score.actions += u64::from(position.actions());
    let GameResult::Win(winner) = position.result() else {
        unreachable!()
    };
    if (winner == Seat::One) == a_is_one {
        score.a_wins += 1;
    } else {
        score.b_wins += 1;
    }
    Ok(())
}

fn random_opening(arguments: Arguments, pair: u64) -> Result<Vec<Move>, String> {
    let mut position = Position::new(arguments.size, SwapRule::Enabled);
    let mut random = SplitMix64(arguments.seed ^ pair.wrapping_mul(0xd1b5_4a32_d192_ed03));
    let mut opening = Vec::with_capacity(usize::from(arguments.opening_plies));
    for _ in 0..arguments.opening_plies {
        let placements = position
            .legal_moves()
            .into_iter()
            .filter(|mv| *mv != Move::Swap)
            .collect::<Vec<_>>();
        let mv = placements[random.index(placements.len())];
        position = position
            .play(mv)
            .map_err(|error| format!("failed to build opening: {error}"))?;
        opening.push(mv);
    }
    Ok(opening)
}

fn print_throughput(label: &str, score: &Score, a: bool) {
    let select = |a_value, b_value| if a { a_value } else { b_value };
    let simulations = select(score.a_simulations, score.b_simulations);
    let elapsed_ms = select(score.a_elapsed_ms, score.b_elapsed_ms);
    let searches = select(score.a_searches, score.b_searches);
    let bridge_replies = select(score.a_bridge_replies, score.b_bridge_replies);
    let knowledge_nodes = select(score.a_knowledge_nodes, score.b_knowledge_nodes);
    let pruned_moves = select(score.a_pruned_moves, score.b_pruned_moves);
    let per_second = if elapsed_ms == 0 {
        0.0
    } else {
        simulations as f64 * 1_000.0 / elapsed_ms as f64
    };
    println!(
        "{label}: {:.0} simulations/search · {:.0} simulations/s · {:.0} bridge replies/search · {:.1} knowledge nodes/search · {:.1} pruned/search",
        simulations as f64 / searches.max(1) as f64,
        per_second,
        bridge_replies as f64 / searches.max(1) as f64,
        knowledge_nodes as f64 / searches.max(1) as f64,
        pruned_moves as f64 / searches.max(1) as f64,
    );
}

fn parse_arguments() -> Result<Arguments, String> {
    let words = env::args().skip(1).collect::<Vec<_>>();
    let mut arguments = Arguments::default();
    let mut index = 0;
    while index < words.len() {
        let flag = &words[index];
        let value = words
            .get(index + 1)
            .ok_or_else(|| format!("{flag} needs a value"))?;
        match flag.as_str() {
            "--games" => arguments.games = parse(value, "games")?,
            "--size" => {
                arguments.size =
                    BoardSize::new(parse(value, "size")?).map_err(|error| error.to_string())?
            }
            "--simulations" => arguments.simulations = parse(value, "simulations")?,
            "--softtime" => arguments.soft_time_ms = parse(value, "softtime")?,
            "--a" => arguments.a_strategy = parse_strategy(value)?,
            "--b" => arguments.b_strategy = parse_strategy(value)?,
            "--rave-a" => arguments.rave_a = parse_rave(value)?,
            "--rave-b" => arguments.rave_b = parse_rave(value)?,
            "--exploration-a" => arguments.exploration_a = parse_exploration(value)?,
            "--exploration-b" => arguments.exploration_b = parse_exploration(value)?,
            "--rollout-a" => arguments.rollout_a = parse_rollout(value)?,
            "--rollout-b" => arguments.rollout_b = parse_rollout(value)?,
            "--knowledge-a" => arguments.knowledge_a = parse_knowledge(value)?,
            "--knowledge-b" => arguments.knowledge_b = parse_knowledge(value)?,
            "--connections-a" => arguments.connections_a = parse_on_off(value)?,
            "--connections-b" => arguments.connections_b = parse_on_off(value)?,
            "--opening-plies" => arguments.opening_plies = parse(value, "opening-plies")?,
            "--seed" => arguments.seed = parse(value, "seed")?,
            _ => return Err(format!("unknown option {flag}")),
        }
        index += 2;
    }
    if arguments.simulations == 0 {
        return Err("simulations must be positive".to_owned());
    }
    Ok(arguments)
}

fn parse_on_off(value: &str) -> Result<bool, String> {
    match value {
        "on" => Ok(true),
        "off" => Ok(false),
        _ => Err("connections must be on or off".to_owned()),
    }
}

const fn on_off(value: bool) -> &'static str {
    if value { "on" } else { "off" }
}

fn parse_strategy(value: &str) -> Result<MctsStrategy, String> {
    match value {
        "plain-uct" => Ok(MctsStrategy::PlainUct),
        "uct-rave" => Ok(MctsStrategy::UctRave),
        _ => Err(format!("unknown strategy {value}")),
    }
}

fn parse_rave(value: &str) -> Result<f64, String> {
    let equivalence = parse::<f64>(value, "RAVE equivalence")?;
    if equivalence.is_finite() && equivalence >= 1.0 {
        Ok(equivalence)
    } else {
        Err("RAVE equivalence must be at least 1".to_owned())
    }
}

fn parse_exploration(value: &str) -> Result<f64, String> {
    let exploration = parse::<f64>(value, "exploration")?;
    if exploration.is_finite() && (0.0..=4.0).contains(&exploration) {
        Ok(exploration)
    } else {
        Err("exploration must be from 0 through 4".to_owned())
    }
}

fn parse_rollout(value: &str) -> Result<RolloutPolicy, String> {
    match value {
        "random" => Ok(RolloutPolicy::Random),
        "save-bridge" => Ok(RolloutPolicy::SaveBridge),
        _ => Err(format!("unknown rollout policy {value}")),
    }
}

fn parse_knowledge(value: &str) -> Result<KnowledgePolicy, String> {
    if value == "off" {
        return Ok(KnowledgePolicy::Disabled);
    }
    let min_visits = parse::<u32>(value, "knowledge threshold")?;
    if min_visits <= 1_000_000 {
        Ok(KnowledgePolicy::InferiorCells { min_visits })
    } else {
        Err("knowledge threshold must be at most 1000000".to_owned())
    }
}

fn knowledge_name(policy: KnowledgePolicy) -> String {
    policy
        .min_visits()
        .map_or_else(|| "off".to_owned(), |visits| visits.to_string())
}

fn parse<T>(value: &str, name: &str) -> Result<T, String>
where
    T: std::str::FromStr,
{
    value.parse().map_err(|_| format!("invalid {name}"))
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

    fn index(&mut self, length: usize) -> usize {
        (self.next() as usize) % length
    }
}
