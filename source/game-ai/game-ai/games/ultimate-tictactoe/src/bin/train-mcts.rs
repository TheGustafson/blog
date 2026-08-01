use ai_ultimate_tictactoe::{PolicyTrainingConfig, train_policy};
use std::env;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

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
    let (config, output) = parse_config(env::args().skip(1).collect())?;
    train_policy(&config, &output)
}

fn parse_config(args: Vec<String>) -> Result<(PolicyTrainingConfig, PathBuf), String> {
    let mut config = PolicyTrainingConfig::default();
    let mut output = PathBuf::from("target/mcts-policy-candidate.bin");
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
            "--samples" => config.samples = parse(value, "samples")?,
            "--teacher-simulations" => {
                config.teacher_simulations = parse(value, "teacher simulations")?;
            }
            "--epochs" => config.epochs = parse(value, "epochs")?,
            "--sample-until-ply" => {
                config.sample_until_ply = parse(value, "sample-until ply")?;
            }
            "--seed" => config.seed = parse(value, "seed")?,
            "--output" => output = Path::new(value).to_owned(),
            option => return Err(format!("unknown option {option}")),
        }
        index += 2;
    }
    Ok((config, output))
}

fn parse<T: std::str::FromStr>(value: &str, label: &str) -> Result<T, String> {
    value
        .parse()
        .map_err(|_| format!("{label} must be a number"))
}

fn usage() -> &'static str {
    "Usage: train-mcts [options]\n\n  --samples N                target self-play positions (default 24000)\n  --teacher-simulations N    PUCT simulations per teacher move (default 1500)\n  --epochs N                 training epochs (default 20)\n  --sample-until-ply N       sample openings through this ply (default 20)\n  --seed N                   deterministic self-play and training seed\n  --output PATH              candidate artifact (default target/mcts-policy-candidate.bin)"
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_owned()).collect()
    }

    #[test]
    fn options_override_the_policy_training_defaults() {
        let (config, output) = parse_config(args(&[
            "--samples",
            "1000",
            "--teacher-simulations",
            "250",
            "--epochs",
            "3",
            "--sample-until-ply",
            "8",
            "--seed",
            "41",
            "--output",
            "/tmp/policy.bin",
        ]))
        .unwrap();

        assert_eq!(config.samples, 1_000);
        assert_eq!(config.teacher_simulations, 250);
        assert_eq!(config.epochs, 3);
        assert_eq!(config.sample_until_ply, 8);
        assert_eq!(config.seed, 41);
        assert_eq!(output, Path::new("/tmp/policy.bin"));
    }

    #[test]
    fn incomplete_and_unknown_options_are_rejected() {
        assert!(parse_config(args(&["--samples"])).is_err());
        assert!(parse_config(args(&["--unknown", "1"])).is_err());
    }

    #[test]
    fn default_output_does_not_overwrite_the_embedded_policy() {
        let (_, output) = parse_config(Vec::new()).unwrap();
        assert_eq!(output, Path::new("target/mcts-policy-candidate.bin"));
    }
}
