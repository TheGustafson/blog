use ai_backgammon::SearchOptions;
use ai_backgammon::selfplay::{AgentConfig, ArenaConfig, run_paired};
use std::env;
use std::process::ExitCode;
use std::time::Instant;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let mut pairs = 50;
    let mut seed = 0x26b7_3dc1_85a4_9e0f;
    let mut a = AgentConfig::Expectimax(search_options(2, 100_000));
    let mut b = AgentConfig::Static;
    let mut arguments = env::args().skip(1);
    while let Some(flag) = arguments.next() {
        let value = arguments
            .next()
            .ok_or_else(|| format!("{flag} needs a value"))?;
        match flag.as_str() {
            "--pairs" => pairs = parse(&value, "pairs")?,
            "--seed" => seed = parse_seed(&value)?,
            "--a" => a = parse_agent(&value)?,
            "--b" => b = parse_agent(&value)?,
            _ => return Err(format!("unknown option {flag}")),
        }
    }
    if pairs == 0 {
        return Err("pairs must be greater than zero".into());
    }

    let started = Instant::now();
    let report = run_paired(
        a,
        b,
        ArenaConfig {
            pairs,
            seed,
            max_turns: 1_000,
        },
    )
    .map_err(|error| error.to_string())?;
    println!("pairs={pairs} games={} seed={seed:#018x}", report.games);
    println!(
        "A wins={} points={} | B wins={} points={}",
        report.a_wins, report.a_points, report.b_wins, report.b_points,
    );
    println!(
        "single={} gammon={} backgammon={} elapsed={:.2?}",
        report.singles,
        report.gammons,
        report.backgammons,
        started.elapsed(),
    );
    print_search(
        "A",
        report.a_searches,
        report.a_nodes,
        report.a_depth,
        report.a_stops,
    );
    print_search(
        "B",
        report.b_searches,
        report.b_nodes,
        report.b_depth,
        report.b_stops,
    );
    Ok(())
}

fn print_search(label: &str, searches: u64, nodes: u64, depth: u64, stops: u64) {
    if searches == 0 {
        return;
    }
    println!(
        "{label} searches={searches} avg_nodes={:.0} avg_depth={:.2} stopped={stops}",
        nodes as f64 / searches as f64,
        depth as f64 / searches as f64,
    );
}

fn parse_agent(value: &str) -> Result<AgentConfig, String> {
    match value {
        "first" => Ok(AgentConfig::FirstLegal),
        "pip" => Ok(AgentConfig::Pip),
        "static" => Ok(AgentConfig::Static),
        _ => {
            let mut fields = value.split(':');
            if fields.next() != Some("search") {
                return Err(format!("unknown agent {value}"));
            }
            let depth = parse(fields.next().unwrap_or(""), "search depth")?;
            let nodes = parse(fields.next().unwrap_or(""), "search nodes")?;
            if fields.next().is_some() || nodes == 0 {
                return Err(
                    "search agents use search:DEPTH:NODES with a positive node limit".into(),
                );
            }
            Ok(AgentConfig::Expectimax(search_options(depth, nodes)))
        }
    }
}

fn search_options(max_depth: u8, node_limit: u64) -> SearchOptions {
    SearchOptions {
        max_depth,
        node_limit,
        soft_time_ms: 0,
    }
}

fn parse<T: std::str::FromStr>(value: &str, name: &str) -> Result<T, String> {
    value
        .parse()
        .map_err(|_| format!("invalid {name}: {value}"))
}

fn parse_seed(value: &str) -> Result<u64, String> {
    value.strip_prefix("0x").map_or_else(
        || parse(value, "seed"),
        |hex| u64::from_str_radix(hex, 16).map_err(|_| format!("invalid seed: {value}")),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_every_agent_kind() {
        assert_eq!(parse_agent("first").unwrap(), AgentConfig::FirstLegal);
        assert_eq!(parse_agent("pip").unwrap(), AgentConfig::Pip);
        assert_eq!(parse_agent("static").unwrap(), AgentConfig::Static);
        assert_eq!(
            parse_agent("search:2:50000").unwrap(),
            AgentConfig::Expectimax(search_options(2, 50_000)),
        );
    }

    #[test]
    fn rejects_incomplete_or_zero_node_limits() {
        assert!(parse_agent("search:2").is_err());
        assert!(parse_agent("search:0:10").is_ok());
        assert!(parse_agent("search:2:0").is_err());
        assert!(parse_agent("search:2:10:extra").is_err());
    }

    #[test]
    fn parses_decimal_and_hex_seeds() {
        assert_eq!(parse_seed("42").unwrap(), 42);
        assert_eq!(parse_seed("0x2a").unwrap(), 42);
    }
}
