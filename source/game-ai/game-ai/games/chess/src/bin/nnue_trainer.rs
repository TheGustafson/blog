//! Small CPU trainer for the educational raw-piece-square NNUE.
//!
//! This is intentionally not a general ML framework. It freezes the exact
//! runtime architecture, trains one deterministic model, quantizes through the
//! production serializer, and reports held-out and float/integer parity.

use ai_chess::{
    Color, EvaluationProfile, FloatNnueNetwork, MoveKind, NNUE_FEATURES, NNUE_HIDDEN, PieceKind,
    Position, QuantizedNnueNetwork, Square, classical_piece_value, evaluate, nnue_feature_index,
};
use std::collections::HashSet;
use std::env;
use std::fs::File;
use std::io::{BufRead, BufReader};

const OUTPUTS: usize = NNUE_HIDDEN * 2;
const HARDNEG_TEACHER_SCALE: f32 = 1.94;
const CANON_TEACHER_SCALE: f32 = 1.42;
const MAX_RAW_CP: f32 = 2_400.0;
const BUCKETS: u64 = 32;
const TRAIN_BUCKETS: u64 = 2;
const VALIDATION_BUCKET: u64 = 2;
const TEST_BUCKET: u64 = 3;
const EPOCHS: usize = 20;
const BATCH: usize = 256;
const HUBER: f32 = 200.0;
const FEATURE_LR: f32 = 0.0008;
const OUTPUT_LR: f32 = 0.05;
const SEED_PULL: f32 = 0.0002;

#[derive(Clone)]
struct Sample {
    position: Position,
    features: Vec<[u16; 2]>,
    side_to_move: Color,
    target: f32,
    psqt: f32,
}

#[derive(Clone)]
struct Model {
    feature_bias: Vec<f32>,
    feature_weights: Vec<f32>,
    output_weights: Vec<f32>,
    output_bias: f32,
}

impl Model {
    fn classical_seed() -> Self {
        const MATERIAL: [f32; 6] = [88.0, 310.0, 330.0, 500.0, 980.0, 0.0];
        const MATERIAL_LANES: usize = 16;
        const LANE_SCALE: f32 = 500.0 * MATERIAL_LANES as f32;
        const PIECE_SQUARE_MIX: f32 = 0.125;

        let mut feature_bias = vec![0.24; NNUE_HIDDEN];
        let mut feature_weights = vec![0.0; NNUE_FEATURES * NNUE_HIDDEN];
        let mut output_weights = vec![0.0; OUTPUTS];
        for lane in 0..MATERIAL_LANES {
            feature_bias[lane] = 0.5;
            output_weights[lane] = 500.0;
        }
        for feature in 0..NNUE_FEATURES {
            let plane = feature / 64;
            let kind = PieceKind::ALL[plane % 6];
            let square = Square::new((feature % 64) as u8);
            let sign = if plane < 6 { 1.0 } else { -1.0 };
            let classical = (classical_piece_value(kind, square, false)
                + classical_piece_value(kind, square, true)) as f32
                / 2.0;
            let value =
                MATERIAL[kind.index()] + PIECE_SQUARE_MIX * (classical - MATERIAL[kind.index()]);
            for lane in 0..MATERIAL_LANES {
                feature_weights[feature * NNUE_HIDDEN + lane] = sign * value / LANE_SCALE;
            }
        }

        let mut state = 0x243f_6a88_85a3_08d3;
        for lane in MATERIAL_LANES..NNUE_HIDDEN {
            feature_bias[lane] = 0.2 + uniform(&mut state) * 0.1;
            output_weights[lane] = uniform(&mut state) * 0.5;
            output_weights[NNUE_HIDDEN + lane] = uniform(&mut state) * 0.5;
        }
        for feature in 0..NNUE_FEATURES {
            for lane in MATERIAL_LANES..NNUE_HIDDEN {
                feature_weights[feature * NNUE_HIDDEN + lane] = uniform(&mut state) * 0.003;
            }
        }
        Self {
            feature_bias,
            feature_weights,
            output_weights,
            output_bias: -(MATERIAL_LANES as f32 * 0.5 * 500.0),
        }
    }

    fn as_network(&self) -> FloatNnueNetwork {
        FloatNnueNetwork::new(
            self.feature_bias.clone(),
            self.feature_weights.clone(),
            self.output_weights.clone(),
            self.output_bias,
        )
        .expect("trainer model dimensions and values are finite")
    }

    fn forward(&self, sample: &Sample) -> (f32, [[f32; NNUE_HIDDEN]; 2]) {
        let mut accumulator = [[0.0; NNUE_HIDDEN]; 2];
        accumulator[0].copy_from_slice(&self.feature_bias);
        accumulator[1].copy_from_slice(&self.feature_bias);
        for features in &sample.features {
            for perspective in Color::ALL {
                let feature = usize::from(features[perspective.index()]);
                let row = &self.feature_weights[feature * NNUE_HIDDEN..(feature + 1) * NNUE_HIDDEN];
                for (value, weight) in accumulator[perspective.index()].iter_mut().zip(row) {
                    *value += weight;
                }
            }
        }

        let us = sample.side_to_move.index();
        let them = sample.side_to_move.other().index();
        let mut output = self.output_bias;
        for (value, weight) in accumulator[us]
            .iter()
            .chain(&accumulator[them])
            .zip(&self.output_weights)
        {
            output += value.clamp(0.0, 1.0) * weight;
        }
        (output, accumulator)
    }
}

struct Gradient {
    feature_bias: Vec<f32>,
    feature_weights: Vec<f32>,
    output_weights: Vec<f32>,
    output_bias: f32,
}

impl Gradient {
    fn zero() -> Self {
        Self {
            feature_bias: vec![0.0; NNUE_HIDDEN],
            feature_weights: vec![0.0; NNUE_FEATURES * NNUE_HIDDEN],
            output_weights: vec![0.0; OUTPUTS],
            output_bias: 0.0,
        }
    }
}

struct Adam {
    first: Gradient,
    second: Gradient,
    step: i32,
}

#[derive(Clone, Copy)]
struct Metrics {
    mae: f32,
    rmse: f32,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("ai-chess-nnue-trainer: {error}");
        std::process::exit(2);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args().skip(1);
    let hardneg_corpus = args
        .next()
        .ok_or("usage: ai-chess-nnue-trainer <hardneg corpus> <canonical sample> <output>")?;
    let canonical_corpus = args
        .next()
        .ok_or("usage: ai-chess-nnue-trainer <hardneg corpus> <canonical sample> <output>")?;
    let output = args
        .next()
        .ok_or("usage: ai-chess-nnue-trainer <hardneg corpus> <canonical sample> <output>")?;
    if args.next().is_some() {
        return Err("trainer accepts exactly three arguments".into());
    }

    let mut seen = HashSet::new();
    let (mut train, hardneg_validation, hardneg_test, hardneg_stats) =
        load(&hardneg_corpus, HARDNEG_TEACHER_SCALE, false, &mut seen)?;
    let (canonical_train, canonical_validation, canonical_test, canonical_stats) =
        load(&canonical_corpus, CANON_TEACHER_SCALE, true, &mut seen)?;
    let hardneg_train_count = train.len();
    let canonical_train_count = canonical_train.len();
    train.extend(canonical_train);
    let mut validation = hardneg_validation.clone();
    validation.extend(canonical_validation.clone());
    let mut test = hardneg_test.clone();
    test.extend(canonical_test.clone());
    if train.is_empty() || validation.is_empty() || test.is_empty() {
        return Err("deterministic split produced an empty dataset".into());
    }
    eprintln!(
        "source={hardneg_corpus} scale={HARDNEG_TEACHER_SCALE} rows={} fnv1a={:016x} train={} validation={} test={} rejected={}",
        hardneg_stats.rows,
        hardneg_stats.fnv1a,
        hardneg_train_count,
        hardneg_validation.len(),
        hardneg_test.len(),
        hardneg_stats.rejected
    );
    eprintln!(
        "source={canonical_corpus} scale={CANON_TEACHER_SCALE} rows={} fnv1a={:016x} train={} validation={} test={} rejected={}",
        canonical_stats.rows,
        canonical_stats.fnv1a,
        canonical_train_count,
        canonical_validation.len(),
        canonical_test.len(),
        canonical_stats.rejected
    );
    eprintln!(
        "combined train={} validation={} test={}",
        train.len(),
        validation.len(),
        test.len(),
    );

    let mut model = Model::classical_seed();
    let seed = model.clone();
    let mut best = model.clone();
    let mut best_metrics = metrics(&validation, &model);
    let validation_psqt_metrics = psqt_metrics(&validation);
    eprintln!(
        "seed mae={:.2} rmse={:.2} | tuned-PSQT mae={:.2} rmse={:.2}",
        best_metrics.mae,
        best_metrics.rmse,
        validation_psqt_metrics.mae,
        validation_psqt_metrics.rmse
    );
    let mut adam = Adam {
        first: Gradient::zero(),
        second: Gradient::zero(),
        step: 0,
    };

    for epoch in 0..EPOCHS {
        shuffle(&mut train, 0xa409_3822_299f_31d0 ^ epoch as u64);
        for batch in train.chunks(BATCH) {
            let mut gradient = Gradient::zero();
            for sample in batch {
                accumulate_gradient(&model, sample, &mut gradient);
            }
            adam_update(&mut model, &seed, &gradient, batch.len(), &mut adam);
        }
        let score = metrics(&validation, &model);
        eprintln!(
            "epoch {:>2}/{EPOCHS}: validation mae={:.2} rmse={:.2}",
            epoch + 1,
            score.mae,
            score.rmse
        );
        if score.rmse < best_metrics.rmse {
            best = model.clone();
            best_metrics = score;
        }
    }

    let float = best.as_network();
    let quantized = float.quantize();
    let test_metrics = metrics(&test, &best);
    let test_psqt_metrics = psqt_metrics(&test);
    let quantized_test_metrics = quantized_metrics(&test, &quantized);
    let max_parity = test
        .iter()
        .map(|sample| {
            (best.forward(sample).0 - quantized.evaluate_refresh(&sample.position) as f32).abs()
        })
        .fold(0.0, f32::max);
    if max_parity > 10.0 {
        return Err(format!(
            "quantized parity gate failed: worst held-out error {max_parity:.3}cp exceeds 10cp"
        )
        .into());
    }
    let mut disagreements: Vec<_> = test
        .iter()
        .map(|sample| {
            let nnue = quantized.evaluate_refresh(&sample.position) as f32;
            let improvement = (sample.psqt - sample.target).abs() - (nnue - sample.target).abs();
            (sample, nnue, improvement, (nnue - sample.psqt).abs())
        })
        .filter(|(_, _, improvement, _)| *improvement >= 50.0)
        .collect();
    disagreements.sort_by(|left, right| right.3.total_cmp(&left.3));
    for (rank, (sample, nnue, improvement, disagreement)) in
        disagreements.into_iter().take(8).enumerate()
    {
        eprintln!(
            "disagreement {} delta={:.0} improvement={:.0} target={:.0} psqt={:.0} nnue={:.0} fen={}",
            rank + 1,
            disagreement,
            improvement,
            sample.target,
            sample.psqt,
            nnue,
            sample.position.fen()
        );
    }
    let bytes = quantized.to_bytes();
    std::fs::write(&output, &bytes)?;
    eprintln!(
        "best validation mae={:.2} rmse={:.2} | held-out PSQT mae={:.2} rmse={:.2}",
        best_metrics.mae, best_metrics.rmse, test_psqt_metrics.mae, test_psqt_metrics.rmse,
    );
    eprintln!(
        "held-out float mae={:.2} rmse={:.2} | quantized mae={:.2} rmse={:.2} | max parity={:.3}cp",
        test_metrics.mae,
        test_metrics.rmse,
        quantized_test_metrics.mae,
        quantized_test_metrics.rmse,
        max_parity
    );
    report_source("hard-negative", &hardneg_test, &best, &quantized);
    report_source("canonical", &canonical_test, &best, &quantized);
    eprintln!(
        "wrote={} bytes={} checksum={:016x}",
        output,
        bytes.len(),
        quantized.checksum()
    );
    Ok(())
}

fn accumulate_gradient(model: &Model, sample: &Sample, gradient: &mut Gradient) {
    let (prediction, accumulator) = model.forward(sample);
    let derivative = (prediction - sample.target).clamp(-HUBER, HUBER);
    gradient.output_bias += derivative;

    let us = sample.side_to_move.index();
    let them = sample.side_to_move.other().index();
    let mut hidden = [[0.0; NNUE_HIDDEN]; 2];
    for lane in 0..NNUE_HIDDEN {
        let us_active = (0.0..1.0).contains(&accumulator[us][lane]);
        let them_active = (0.0..1.0).contains(&accumulator[them][lane]);
        gradient.output_weights[lane] += derivative * accumulator[us][lane].clamp(0.0, 1.0);
        gradient.output_weights[NNUE_HIDDEN + lane] +=
            derivative * accumulator[them][lane].clamp(0.0, 1.0);
        if us_active {
            hidden[us][lane] = derivative * model.output_weights[lane];
        }
        if them_active {
            hidden[them][lane] = derivative * model.output_weights[NNUE_HIDDEN + lane];
        }
        gradient.feature_bias[lane] += hidden[0][lane] + hidden[1][lane];
    }

    for features in &sample.features {
        for perspective in Color::ALL {
            let feature = usize::from(features[perspective.index()]);
            let row =
                &mut gradient.feature_weights[feature * NNUE_HIDDEN..(feature + 1) * NNUE_HIDDEN];
            for (value, derivative) in row.iter_mut().zip(&hidden[perspective.index()]) {
                *value += derivative;
            }
        }
    }
}

fn adam_update(
    model: &mut Model,
    seed: &Model,
    gradient: &Gradient,
    batch: usize,
    adam: &mut Adam,
) {
    adam.step += 1;
    let inverse = 1.0 / batch as f32;
    let first_correction = 1.0 - 0.9f32.powi(adam.step);
    let second_correction = 1.0 - 0.999f32.powi(adam.step);
    update_slice(
        &mut model.feature_bias,
        &gradient.feature_bias,
        &mut adam.first.feature_bias,
        &mut adam.second.feature_bias,
        FEATURE_LR,
        inverse,
        first_correction,
        second_correction,
    );
    update_slice(
        &mut model.feature_weights,
        &gradient.feature_weights,
        &mut adam.first.feature_weights,
        &mut adam.second.feature_weights,
        FEATURE_LR,
        inverse,
        first_correction,
        second_correction,
    );
    update_slice(
        &mut model.output_weights,
        &gradient.output_weights,
        &mut adam.first.output_weights,
        &mut adam.second.output_weights,
        OUTPUT_LR,
        inverse,
        first_correction,
        second_correction,
    );
    update_scalar(
        &mut model.output_bias,
        gradient.output_bias,
        &mut adam.first.output_bias,
        &mut adam.second.output_bias,
        OUTPUT_LR,
        inverse,
        first_correction,
        second_correction,
    );

    for value in &mut model.feature_bias {
        *value = value.clamp(-2.0, 2.0);
    }
    for value in &mut model.feature_weights {
        *value = value.clamp(-1.0, 1.0);
    }
    for value in &mut model.output_weights {
        *value = value.clamp(-500.0, 500.0);
    }
    model.output_bias = model.output_bias.clamp(-5_000.0, 5_000.0);

    pull_toward(&mut model.feature_bias, &seed.feature_bias);
    pull_toward(&mut model.feature_weights, &seed.feature_weights);
    pull_toward(&mut model.output_weights, &seed.output_weights);
    model.output_bias += SEED_PULL * (seed.output_bias - model.output_bias);
}

fn pull_toward(values: &mut [f32], seed: &[f32]) {
    for (value, seed) in values.iter_mut().zip(seed) {
        *value += SEED_PULL * (*seed - *value);
    }
}

#[allow(clippy::too_many_arguments)]
fn update_slice(
    values: &mut [f32],
    gradients: &[f32],
    first: &mut [f32],
    second: &mut [f32],
    learning_rate: f32,
    inverse_batch: f32,
    first_correction: f32,
    second_correction: f32,
) {
    for index in 0..values.len() {
        update_scalar(
            &mut values[index],
            gradients[index],
            &mut first[index],
            &mut second[index],
            learning_rate,
            inverse_batch,
            first_correction,
            second_correction,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn update_scalar(
    value: &mut f32,
    gradient: f32,
    first: &mut f32,
    second: &mut f32,
    learning_rate: f32,
    inverse_batch: f32,
    first_correction: f32,
    second_correction: f32,
) {
    let gradient = gradient * inverse_batch;
    *first = 0.9 * *first + 0.1 * gradient;
    *second = 0.999 * *second + 0.001 * gradient * gradient;
    let direction = (*first / first_correction) / ((*second / second_correction).sqrt() + 1e-8);
    *value -= learning_rate * direction;
}

fn metrics(samples: &[Sample], model: &Model) -> Metrics {
    errors(
        samples
            .iter()
            .map(|sample| model.forward(sample).0 - sample.target),
    )
}

fn psqt_metrics(samples: &[Sample]) -> Metrics {
    errors(samples.iter().map(|sample| sample.psqt - sample.target))
}

fn quantized_metrics(samples: &[Sample], network: &QuantizedNnueNetwork) -> Metrics {
    errors(
        samples
            .iter()
            .map(|sample| network.evaluate_refresh(&sample.position) as f32 - sample.target),
    )
}

fn report_source(label: &str, samples: &[Sample], model: &Model, network: &QuantizedNnueNetwork) {
    let psqt = psqt_metrics(samples);
    let float = metrics(samples, model);
    let quantized = quantized_metrics(samples, network);
    eprintln!(
        "{label} test: PSQT={:.2}/{:.2} float={:.2}/{:.2} quantized={:.2}/{:.2} mae/rmse",
        psqt.mae, psqt.rmse, float.mae, float.rmse, quantized.mae, quantized.rmse
    );
}

fn errors(values: impl Iterator<Item = f32>) -> Metrics {
    let mut count = 0;
    let mut absolute = 0.0;
    let mut squared = 0.0;
    for error in values {
        count += 1;
        absolute += error.abs();
        squared += error * error;
    }
    Metrics {
        mae: absolute / count as f32,
        rmse: (squared / count as f32).sqrt(),
    }
}

struct LoadStats {
    rows: usize,
    rejected: usize,
    fnv1a: u64,
}

type Dataset = (Vec<Sample>, Vec<Sample>, Vec<Sample>, LoadStats);

fn load(
    path: &str,
    teacher_scale: f32,
    broad_training: bool,
    seen: &mut HashSet<u64>,
) -> Result<Dataset, Box<dyn std::error::Error>> {
    let reader = BufReader::new(File::open(path)?);
    let mut train = Vec::new();
    let mut validation = Vec::new();
    let mut test = Vec::new();
    let mut stats = LoadStats {
        rows: 0,
        rejected: 0,
        fnv1a: 0xcbf2_9ce4_8422_2325,
    };

    for line in reader.lines() {
        let line = line?;
        stats.rows += 1;
        for byte in line.bytes().chain(std::iter::once(b'\n')) {
            stats.fnv1a ^= u64::from(byte);
            stats.fnv1a = stats.fnv1a.wrapping_mul(0x100_0000_01b3);
        }
        let Some((fen, raw_cp)) = parse_row(&line) else {
            stats.rejected += 1;
            continue;
        };
        let Ok(mut position) = Position::from_fen(fen) else {
            stats.rejected += 1;
            continue;
        };
        let bucket = mix64(position.key()) % BUCKETS;
        let is_train = bucket < TRAIN_BUCKETS || (broad_training && (4..24).contains(&bucket));
        let is_validation = bucket == VALIDATION_BUCKET;
        let is_test = bucket == TEST_BUCKET;
        if !is_train && !is_validation && !is_test {
            stats.rejected += 1;
            continue;
        }
        if !seen.insert(position.key())
            || position.in_check(position.side_to_move())
            || raw_cp.abs() > MAX_RAW_CP
        {
            stats.rejected += 1;
            continue;
        }
        let legal = position.legal_moves();
        if legal.is_empty()
            || legal.as_slice().iter().any(|mv| {
                mv.promotion().is_some()
                    || mv.kind() == MoveKind::EnPassant
                    || position.piece_at(mv.to()).is_some()
            })
        {
            stats.rejected += 1;
            continue;
        }
        let target_white = raw_cp / teacher_scale;
        let target = if position.side_to_move() == Color::White {
            target_white
        } else {
            -target_white
        };
        let psqt = evaluate(&position, EvaluationProfile::PieceSquare).total as f32;
        let sample = Sample {
            features: features(&position),
            side_to_move: position.side_to_move(),
            target,
            psqt,
            position,
        };
        if is_train {
            train.push(sample);
        } else if is_validation {
            validation.push(sample);
        } else {
            test.push(sample);
        }
    }
    Ok((train, validation, test, stats))
}

fn parse_row(line: &str) -> Option<(&str, f32)> {
    let mut fields = line.split('|').map(str::trim);
    let first = fields.next()?;
    let fen = first.rsplit_once('\t').map_or(first, |(_, fen)| fen.trim());
    let cp = fields.next()?.parse().ok()?;
    let result: f32 = fields.next()?.parse().ok()?;
    if fields.next().is_some() || !matches!(result, 0.0 | 0.5 | 1.0) {
        return None;
    }
    Some((fen, cp))
}

fn features(position: &Position) -> Vec<[u16; 2]> {
    let mut features = Vec::with_capacity(position.occupied().count_ones() as usize);
    for color in Color::ALL {
        for kind in PieceKind::ALL {
            let mut occupied = position.pieces(color, kind);
            while occupied != 0 {
                let square = ai_chess::Square::new(occupied.trailing_zeros() as u8);
                occupied &= occupied - 1;
                let piece = ai_chess::Piece::new(color, kind);
                features.push([
                    nnue_feature_index(piece, square, Color::White) as u16,
                    nnue_feature_index(piece, square, Color::Black) as u16,
                ]);
            }
        }
    }
    features
}

fn mix64(mut value: u64) -> u64 {
    value ^= value >> 30;
    value = value.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value ^= value >> 27;
    value = value.wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

fn shuffle<T>(values: &mut [T], mut state: u64) {
    for index in (1..values.len()).rev() {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        values.swap(index, state as usize % (index + 1));
    }
}

fn uniform(state: &mut u64) -> f32 {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    ((*state >> 40) as f32 / (1u32 << 24) as f32) * 2.0 - 1.0
}
