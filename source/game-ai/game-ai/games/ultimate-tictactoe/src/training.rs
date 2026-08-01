use crate::network::{
    B_POLICY, B1, HIDDEN, INPUTS, PARAM_COUNT, POLICY_OUTPUTS, PolicyNetwork, W_POLICY, W1, encode,
    transform_index,
};
use crate::{GameResult, MctsOptions, MctsSearcher, MctsStrategy, Move, Position};
use std::path::Path;

#[derive(Clone, Debug)]
pub struct PolicyTrainingConfig {
    pub samples: usize,
    pub teacher_simulations: u32,
    pub epochs: usize,
    pub sample_until_ply: u8,
    pub seed: u64,
}

impl Default for PolicyTrainingConfig {
    fn default() -> Self {
        Self {
            samples: 24_000,
            teacher_simulations: 1_500,
            epochs: 20,
            sample_until_ply: 20,
            seed: 0x5345_4c46_504c_4159,
        }
    }
}

#[derive(Clone)]
struct Example {
    position: Position,
    policy: [f32; POLICY_OUTPUTS],
}

/// Trains a new policy from the embedded policy's self-play search visits.
pub fn train_policy(config: &PolicyTrainingConfig, output: &Path) -> Result<(), String> {
    validate_config(config)?;
    println!(
        "generating {} positions from PUCT self-play ({} simulations/move)",
        config.samples, config.teacher_simulations
    );
    let mut random = SplitMix64(config.seed);
    let (training, validation) = generate_examples(config, &mut random)?;
    println!(
        "split by complete games · {} training positions · {} validation positions",
        training.len(),
        validation.len()
    );
    train_model(
        PolicyNetwork::embedded()?,
        &training,
        &validation,
        config.epochs,
        config.seed,
        output,
    )
}

fn validate_config(config: &PolicyTrainingConfig) -> Result<(), String> {
    if config.samples < 100 {
        return Err("samples must be at least 100".to_owned());
    }
    if config.teacher_simulations < 81 {
        return Err("teacher simulations must be at least 81".to_owned());
    }
    if config.epochs == 0 {
        return Err("epochs must be positive".to_owned());
    }
    if config.sample_until_ply > 81 {
        return Err("sample-until ply must be from 0 through 81".to_owned());
    }
    Ok(())
}

fn generate_examples(
    config: &PolicyTrainingConfig,
    random: &mut SplitMix64,
) -> Result<(Vec<Example>, Vec<Example>), String> {
    let mut games = Vec::new();
    let mut positions = 0;
    while positions < config.samples {
        let mut position = Position::start();
        let mut game = Vec::with_capacity(64);
        let mut teacher = MctsSearcher::new();
        while position.result() == GameResult::Ongoing {
            let report = teacher.search(
                position,
                MctsOptions {
                    max_simulations: config.teacher_simulations,
                    soft_time_ms: 0,
                    exploration: std::f64::consts::SQRT_2,
                    seed: random.next() ^ position.hash(),
                    strategy: MctsStrategy::PuctLearned,
                },
            );
            let mut policy = [0.0; POLICY_OUTPUTS];
            for stats in &report.root_moves {
                policy[stats.mv.global_index() as usize] =
                    stats.visits as f32 / report.root_visits.max(1) as f32;
            }
            game.push(Example { position, policy });
            let mv = if position.ply() < config.sample_until_ply {
                sample_root_move(&report.root_moves, report.root_visits, random)
            } else {
                report.best_move
            }
            .ok_or_else(|| "teacher returned no move in an ongoing game".to_owned())?;
            position = position
                .play(mv)
                .map_err(|error| format!("teacher returned illegal move {mv}: {error}"))?;
        }
        positions += game.len();
        games.push(game);
        if games.len() % 25 == 0 || positions >= config.samples {
            println!("  {:>3} games · {positions:>5} positions", games.len());
        }
    }

    shuffle(&mut games, random);
    let validation_games = (games.len() / 10).max(1);
    let validation = games
        .split_off(games.len() - validation_games)
        .into_iter()
        .flatten()
        .collect();
    let training = games.into_iter().flatten().collect();
    Ok((training, validation))
}

fn train_model(
    mut network: PolicyNetwork,
    training: &[Example],
    validation: &[Example],
    epochs: usize,
    seed: u64,
    output: &Path,
) -> Result<(), String> {
    let mut random = SplitMix64(seed);
    let mut optimizer = Adam::new();
    let mut order = (0..training.len()).collect::<Vec<_>>();
    let mut best_loss = print_metrics("initial", &network, validation);
    let mut best_network = network.clone();

    for epoch in 0..epochs {
        shuffle(&mut order, &mut random);
        let learning_rate = 0.003 * (1.0 - 0.75 * epoch as f32 / epochs as f32);
        let mut loss = 0.0;
        for batch in order.chunks(32) {
            let mut gradient = vec![0.0; PARAM_COUNT];
            for &example in batch {
                let symmetry = random.index(8) as u8;
                loss += accumulate_gradient(&network, &training[example], symmetry, &mut gradient);
            }
            optimizer.update(
                &mut network.parameters,
                &gradient,
                1.0 / batch.len() as f32,
                learning_rate,
            );
        }
        if epoch == 0 || (epoch + 1) % 4 == 0 || epoch + 1 == epochs {
            println!(
                "epoch {:>2}/{:<2} · train policy {:.3}",
                epoch + 1,
                epochs,
                loss / training.len() as f32,
            );
            let validation_loss = print_metrics("validation", &network, validation);
            if validation_loss < best_loss {
                best_loss = validation_loss;
                best_network = network.clone();
            }
        }
    }

    print_metrics("selected", &best_network, validation);
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("could not create {}: {error}", parent.display()))?;
    }
    std::fs::write(output, best_network.to_bytes())
        .map_err(|error| format!("could not write {}: {error}", output.display()))?;
    println!(
        "wrote {} parameters to {}",
        best_network.parameters.len(),
        output.display()
    );
    Ok(())
}

fn sample_root_move(
    moves: &[crate::MctsMoveStats],
    root_visits: u32,
    random: &mut SplitMix64,
) -> Option<Move> {
    let mut target = random.index(root_visits.max(1) as usize) as u32;
    for stats in moves {
        if target < stats.visits {
            return Some(stats.mv);
        }
        target -= stats.visits;
    }
    moves.first().map(|stats| stats.mv)
}

fn accumulate_gradient(
    network: &PolicyNetwork,
    example: &Example,
    symmetry: u8,
    gradient: &mut [f32],
) -> f32 {
    let input = encode(example.position, symmetry);
    let hidden = network.hidden(&input);
    let mut legal = [false; POLICY_OUTPUTS];
    let mut target = [0.0; POLICY_OUTPUTS];
    for mv in example.position.legal_moves().iter() {
        let original = mv.global_index() as usize;
        let transformed = transform_index(mv.global_index(), 9, symmetry) as usize;
        legal[transformed] = true;
        target[transformed] = example.policy[original];
    }
    let output = network.predict_encoded(&input, &legal);
    let mut hidden_gradient = [0.0; HIDDEN];
    let mut loss = 0.0;

    for action in 0..POLICY_OUTPUTS {
        if !legal[action] {
            continue;
        }
        let probability = output[action] as f32;
        if target[action] > 0.0 {
            loss -= target[action] * probability.max(1e-9).ln();
        }
        let derivative = probability - target[action];
        gradient[B_POLICY + action] += derivative;
        let offset = W_POLICY + action * HIDDEN;
        for hidden_index in 0..HIDDEN {
            gradient[offset + hidden_index] += derivative * hidden[hidden_index];
            hidden_gradient[hidden_index] += derivative * network.parameters[offset + hidden_index];
        }
    }

    for hidden_index in 0..HIDDEN {
        if hidden[hidden_index] == 0.0 {
            continue;
        }
        let derivative = hidden_gradient[hidden_index];
        gradient[B1 + hidden_index] += derivative;
        let offset = W1 + hidden_index * INPUTS;
        for input_index in 0..INPUTS {
            if input[input_index] != 0.0 {
                gradient[offset + input_index] += derivative * input[input_index];
            }
        }
    }
    loss
}

fn print_metrics(label: &str, network: &PolicyNetwork, examples: &[Example]) -> f32 {
    let mut policy_loss = 0.0;
    let mut top_move = 0;
    for example in examples {
        let input = encode(example.position, 0);
        let mut legal = [false; POLICY_OUTPUTS];
        for mv in example.position.legal_moves().iter() {
            legal[mv.global_index() as usize] = true;
        }
        let output = network.predict_encoded(&input, &legal);
        for (&target, &probability) in example.policy.iter().zip(&output) {
            if target > 0.0 {
                policy_loss -= target * (probability as f32).max(1e-9).ln();
            }
        }
        top_move += usize::from(
            maximum_index(&output, &legal) == maximum_index(&example.policy.map(f64::from), &legal),
        );
    }
    let count = examples.len().max(1) as f32;
    let policy_loss = policy_loss / count;
    println!(
        "  {label:<10} · policy {policy_loss:.3} · top move {:>5.1}%",
        top_move as f32 * 100.0 / count,
    );
    policy_loss
}

fn maximum_index(values: &[f64; POLICY_OUTPUTS], legal: &[bool; POLICY_OUTPUTS]) -> usize {
    (0..POLICY_OUTPUTS)
        .filter(|&index| legal[index])
        .max_by(|&left, &right| values[left].total_cmp(&values[right]))
        .unwrap_or(0)
}

struct Adam {
    first: Vec<f32>,
    second: Vec<f32>,
    step: i32,
}

impl Adam {
    fn new() -> Self {
        Self {
            first: vec![0.0; PARAM_COUNT],
            second: vec![0.0; PARAM_COUNT],
            step: 0,
        }
    }

    fn update(&mut self, parameters: &mut [f32], gradient: &[f32], scale: f32, rate: f32) {
        self.step += 1;
        let first_correction = 1.0 - 0.9_f32.powi(self.step);
        let second_correction = 1.0 - 0.999_f32.powi(self.step);
        for index in 0..parameters.len() {
            let derivative = gradient[index] * scale + 1e-6 * parameters[index];
            self.first[index] = 0.9 * self.first[index] + 0.1 * derivative;
            self.second[index] = 0.999 * self.second[index] + 0.001 * derivative * derivative;
            let first = self.first[index] / first_correction;
            let second = self.second[index] / second_correction;
            parameters[index] -= rate * first / (second.sqrt() + 1e-8);
        }
    }
}

fn shuffle<T>(values: &mut [T], random: &mut SplitMix64) {
    for index in (1..values.len()).rev() {
        values.swap(index, random.index(index + 1));
    }
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

    fn index(&mut self, len: usize) -> usize {
        debug_assert!(len > 0);
        (self.next() as usize) % len
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn self_play_generation_is_deterministic_and_normalized() {
        let config = PolicyTrainingConfig {
            samples: 100,
            teacher_simulations: 81,
            epochs: 1,
            sample_until_ply: 4,
            seed: 41,
        };
        let first = generate_examples(&config, &mut SplitMix64(config.seed)).unwrap();
        let second = generate_examples(&config, &mut SplitMix64(config.seed)).unwrap();

        assert!(!first.0.is_empty());
        assert!(!first.1.is_empty());
        assert!(first.0.len() + first.1.len() >= config.samples);
        assert_eq!(first.0.len(), second.0.len());
        assert_eq!(first.1.len(), second.1.len());
        for (left, right) in first.0.iter().zip(second.0.iter()) {
            assert_eq!(left.position, right.position);
            assert_eq!(left.policy, right.policy);
            assert!((left.policy.iter().sum::<f32>() - 1.0).abs() < 1e-6);
        }
    }

    #[test]
    fn training_writes_a_loadable_policy_candidate() {
        let position = Position::start();
        let mut policy = [0.0; POLICY_OUTPUTS];
        for mv in position.legal_moves().iter() {
            policy[mv.global_index() as usize] = 1.0 / 81.0;
        }
        let example = Example { position, policy };
        let output = std::env::temp_dir().join(format!(
            "ai-ultimate-tictactoe-policy-{}.bin",
            std::process::id()
        ));

        train_model(
            PolicyNetwork::embedded().unwrap(),
            std::slice::from_ref(&example),
            std::slice::from_ref(&example),
            1,
            41,
            &output,
        )
        .unwrap();
        let bytes = std::fs::read(&output).unwrap();
        let candidate = PolicyNetwork::from_bytes(&bytes).unwrap();
        let priors = candidate.predict(position);
        assert!((priors.iter().sum::<f64>() - 1.0).abs() < 1e-6);
        std::fs::remove_file(output).unwrap();
    }
}
