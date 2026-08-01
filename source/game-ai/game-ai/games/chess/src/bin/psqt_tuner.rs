//! Offline tapered-PSQT tuner.
//!
//! This deliberately lives outside the engine. It consumes lines shaped like
//! `FEN | white-pov teacher cp | white game result`, validates every selected
//! position with the production rule core, keeps only quiet positions, fits a
//! sparse linear model, and prints Rust constants. Runtime evaluation never
//! reads the corpus or performs floating-point inference.

use ai_chess::{Color, MoveKind, PieceKind, Position};
use std::collections::HashSet;
use std::env;
use std::fs::File;
use std::io::{self, BufRead, BufReader, Write};

const KINDS: usize = 6;
const SQUARES: usize = 64;
const PHASES: usize = 2;
const STRIDE: usize = 1 + SQUARES;
const PARAMS: usize = PHASES * KINDS * STRIDE;
const MAX_PHASE: f64 = 24.0;

// The local hard-negative artifact stores SF18 UCI centipawns multiplied by
// 1.94 to match Archon's historical training scale. Undo that corpus-specific
// transform before fitting a conventional centipawn evaluator.
const TEACHER_SCALE: f64 = 1.94;
const MAX_RAW_CP: f64 = 2_400.0;
const SAMPLE_BUCKETS: u64 = 32;
const TRAIN_BUCKETS: u64 = 4;
const VALIDATION_BUCKET: u64 = 4;
const EPOCHS: usize = 12;
const BATCH_SIZE: usize = 512;
const HUBER_DELTA: f64 = 200.0;
const LEARNING_RATE: f64 = 0.35;
const RIDGE_TO_SEED: f64 = 0.01;
const MAX_TABLE_DELTA: f64 = 32.0;

// PeSTO's public Texel-tuned material values and square tables are the
// defensible seed. The source order is the published a8..h1 mapping; the
// feature extractor uses a1..h8, so `seed_weights` flips ranks with `sq ^ 56`.
// Provenance:
// https://www.chessprogramming.org/PeSTO%27s_Evaluation_Function
const SEED_MIDDLEGAME_VALUE: [i32; KINDS] = [82, 337, 365, 477, 1025, 0];
const SEED_ENDGAME_VALUE: [i32; KINDS] = [94, 281, 297, 512, 936, 0];

#[rustfmt::skip]
const SEED_MIDDLEGAME_PIECE_SQUARE: [[i32; SQUARES]; KINDS] = [
    [
          0,   0,   0,   0,   0,   0,   0,   0,
         98, 134,  61,  95,  68, 126,  34, -11,
         -6,   7,  26,  31,  65,  56,  25, -20,
        -14,  13,   6,  21,  23,  12,  17, -23,
        -27,  -2,  -5,  12,  17,   6,  10, -25,
        -26,  -4,  -4, -10,   3,   3,  33, -12,
        -35,  -1, -20, -23, -15,  24,  38, -22,
          0,   0,   0,   0,   0,   0,   0,   0,
    ],
    [
        -167, -89, -34, -49,  61, -97, -15, -107,
         -73, -41,  72,  36,  23,  62,   7,  -17,
         -47,  60,  37,  65,  84, 129,  73,   44,
          -9,  17,  19,  53,  37,  69,  18,   22,
         -13,   4,  16,  13,  28,  19,  21,   -8,
         -23,  -9,  12,  10,  19,  17,  25,  -16,
         -29, -53, -12,  -3,  -1,  18, -14,  -19,
        -105, -21, -58, -33, -17, -28, -19,  -23,
    ],
    [
        -29,   4, -82, -37, -25, -42,   7,  -8,
        -26,  16, -18, -13,  30,  59,  18, -47,
        -16,  37,  43,  40,  35,  50,  37,  -2,
         -4,   5,  19,  50,  37,  37,   7,  -2,
         -6,  13,  13,  26,  34,  12,  10,   4,
          0,  15,  15,  15,  14,  27,  18,  10,
          4,  15,  16,   0,   7,  21,  33,   1,
        -33,  -3, -14, -21, -13, -12, -39, -21,
    ],
    [
         32,  42,  32,  51,  63,   9,  31,  43,
         27,  32,  58,  62,  80,  67,  26,  44,
         -5,  19,  26,  36,  17,  45,  61,  16,
        -24, -11,   7,  26,  24,  35,  -8, -20,
        -36, -26, -12,  -1,   9,  -7,   6, -23,
        -45, -25, -16, -17,   3,   0,  -5, -33,
        -44, -16, -20,  -9,  -1,  11,  -6, -71,
        -19, -13,   1,  17,  16,   7, -37, -26,
    ],
    [
        -28,   0,  29,  12,  59,  44,  43,  45,
        -24, -39,  -5,   1, -16,  57,  28,  54,
        -13, -17,   7,   8,  29,  56,  47,  57,
        -27, -27, -16, -16,  -1,  17,  -2,   1,
         -9, -26,  -9, -10,  -2,  -4,   3,  -3,
        -14,   2, -11,  -2,  -5,   2,  14,   5,
        -35,  -8,  11,   2,   8,  15,  -3,   1,
         -1, -18,  -9,  10, -15, -25, -31, -50,
    ],
    [
        -65,  23,  16, -15, -56, -34,   2,  13,
         29,  -1, -20,  -7,  -8,  -4, -38, -29,
         -9,  24,   2, -16, -20,   6,  22, -22,
        -17, -20, -12, -27, -30, -25, -14, -36,
        -49,  -1, -27, -39, -46, -44, -33, -51,
        -14, -14, -22, -46, -44, -30, -15, -27,
          1,   7,  -8, -64, -43, -16,   9,   8,
        -15,  36,  12, -54,   8, -28,  24,  14,
    ],
];

#[rustfmt::skip]
const SEED_ENDGAME_PIECE_SQUARE: [[i32; SQUARES]; KINDS] = [
    [
          0,   0,   0,   0,   0,   0,   0,   0,
        178, 173, 158, 134, 147, 132, 165, 187,
         94, 100,  85,  67,  56,  53,  82,  84,
         32,  24,  13,   5,  -2,   4,  17,  17,
         13,   9,  -3,  -7,  -7,  -8,   3,  -1,
          4,   7,  -6,   1,   0,  -5,  -1,  -8,
         13,   8,   8,  10,  13,   0,   2,  -7,
          0,   0,   0,   0,   0,   0,   0,   0,
    ],
    [
        -58, -38, -13, -28, -31, -27, -63, -99,
        -25,  -8, -25,  -2,  -9, -25, -24, -52,
        -24, -20,  10,   9,  -1,  -9, -19, -41,
        -17,   3,  22,  22,  22,  11,   8, -18,
        -18,  -6,  16,  25,  16,  17,   4, -18,
        -23,  -3,  -1,  15,  10,  -3, -20, -22,
        -42, -20, -10,  -5,  -2, -20, -23, -44,
        -29, -51, -23, -15, -22, -18, -50, -64,
    ],
    [
        -14, -21, -11,  -8,  -7,  -9, -17, -24,
         -8,  -4,   7, -12,  -3, -13,  -4, -14,
          2,  -8,   0,  -1,  -2,   6,   0,   4,
         -3,   9,  12,   9,  14,  10,   3,   2,
         -6,   3,  13,  19,   7,  10,  -3,  -9,
        -12,  -3,   8,  10,  13,   3,  -7, -15,
        -14, -18,  -7,  -1,   4,  -9, -15, -27,
        -23,  -9, -23,  -5,  -9, -16,  -5, -17,
    ],
    [
         13,  10,  18,  15,  12,  12,   8,   5,
         11,  13,  13,  11,  -3,   3,   8,   3,
          7,   7,   7,   5,   4,  -3,  -5,  -3,
          4,   3,  13,   1,   2,   1,  -1,   2,
          3,   5,   8,   4,  -5,  -6,  -8, -11,
         -4,   0,  -5,  -1,  -7, -12,  -8, -16,
         -6,  -6,   0,   2,  -9,  -9, -11,  -3,
         -9,   2,   3,  -1,  -5, -13,   4, -20,
    ],
    [
         -9,  22,  22,  27,  27,  19,  10,  20,
        -17,  20,  32,  41,  58,  25,  30,   0,
        -20,   6,   9,  49,  47,  35,  19,   9,
          3,  22,  24,  45,  57,  40,  57,  36,
        -18,  28,  19,  47,  31,  34,  39,  23,
        -16, -27,  15,   6,   9,  17,  10,   5,
        -22, -23, -30, -16, -16, -23, -36, -32,
        -33, -28, -22, -43,  -5, -32, -20, -41,
    ],
    [
        -74, -35, -18, -18, -11,  15,   4, -17,
        -12,  17,  14,  17,  17,  38,  23,  11,
         10,  17,  23,  15,  20,  45,  44,  13,
         -8,  22,  24,  27,  26,  33,  26,   3,
        -18,  -4,  21,  24,  27,  23,   9, -11,
        -19,  -3,  11,  21,  23,  16,   7,  -9,
        -27, -11,   4,  13,  14,   4,  -5, -17,
        -53, -34, -21, -11, -28, -14, -24, -43,
    ],
];

#[derive(Clone, Copy)]
struct Feature {
    kind: usize,
    square: usize,
    sign: f64,
}

struct Sample {
    pieces: Vec<Feature>,
    middlegame: f64,
    endgame: f64,
    target: f64,
}

#[derive(Default)]
struct LoadStats {
    lines: usize,
    malformed: usize,
    outlier: usize,
    unsampled: usize,
    invalid_fen: usize,
    duplicate: usize,
    in_check: usize,
    tactical: usize,
    terminal: usize,
    train: usize,
    validation: usize,
    fnv1a: u64,
}

#[derive(Clone, Copy)]
struct Metrics {
    mae: f64,
    rmse: f64,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("ai-chess-psqt-tuner: {error}");
        std::process::exit(2);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let path = env::args()
        .nth(1)
        .ok_or("usage: ai-chess-psqt-tuner <FEN|cp|result corpus>")?;
    let file = File::open(&path)?;
    let (mut train, validation, stats) = load_samples(BufReader::new(file))?;
    if train.is_empty() || validation.is_empty() {
        return Err("the deterministic split produced an empty train or validation set".into());
    }

    let seed = seed_weights();
    let mut weights = seed.clone();
    let seed_metrics = metrics(&validation, &seed);
    let mut adam_m = vec![0.0; PARAMS];
    let mut adam_v = vec![0.0; PARAMS];
    let mut update = 0i32;

    eprintln!(
        "source={path} lines={} fnv1a={:016x}",
        stats.lines, stats.fnv1a
    );
    eprintln!(
        "kept train={} validation={} | rejected malformed={} outlier={} unsampled={} invalid_fen={} duplicate={} check={} tactical={} terminal={}",
        stats.train,
        stats.validation,
        stats.malformed,
        stats.outlier,
        stats.unsampled,
        stats.invalid_fen,
        stats.duplicate,
        stats.in_check,
        stats.tactical,
        stats.terminal,
    );
    eprintln!(
        "seed validation mae={:.2}cp rmse={:.2}cp",
        seed_metrics.mae, seed_metrics.rmse
    );

    for epoch in 0..EPOCHS {
        shuffle(&mut train, 0x9e37_79b9_7f4a_7c15 ^ epoch as u64);
        for batch in train.chunks(BATCH_SIZE) {
            update += 1;
            let mut gradient = vec![0.0; PARAMS];
            for sample in batch {
                let residual =
                    (predict(sample, &weights) - sample.target).clamp(-HUBER_DELTA, HUBER_DELTA);
                accumulate_gradient(sample, residual, &mut gradient);
            }
            let inverse_batch = 1.0 / batch.len() as f64;
            let beta1_correction = 1.0 - 0.9f64.powi(update);
            let beta2_correction = 1.0 - 0.999f64.powi(update);
            for index in 0..PARAMS {
                if index % STRIDE == 0 {
                    continue;
                }
                let gradient = gradient[index] * inverse_batch
                    + RIDGE_TO_SEED * (weights[index] - seed[index]);
                adam_m[index] = 0.9 * adam_m[index] + 0.1 * gradient;
                adam_v[index] = 0.999 * adam_v[index] + 0.001 * gradient * gradient;
                let first = adam_m[index] / beta1_correction;
                let second = adam_v[index] / beta2_correction;
                weights[index] -= LEARNING_RATE * first / (second.sqrt() + 1e-8);
                weights[index] = weights[index]
                    .clamp(seed[index] - MAX_TABLE_DELTA, seed[index] + MAX_TABLE_DELTA);
            }
        }
        let score = metrics(&validation, &weights);
        eprintln!(
            "epoch {:>2}/{EPOCHS}: validation mae={:.2}cp rmse={:.2}cp",
            epoch + 1,
            score.mae,
            score.rmse
        );
    }

    let tuned_metrics = metrics(&validation, &weights);
    let rounded = rounded_weights(&weights);
    let rounded_metrics = metrics_i16(&validation, &rounded);
    eprintln!(
        "tuned validation mae={:.2}cp rmse={:.2}cp | rounded mae={:.2}cp rmse={:.2}cp",
        tuned_metrics.mae, tuned_metrics.rmse, rounded_metrics.mae, rounded_metrics.rmse
    );
    emit_constants(
        io::stdout().lock(),
        &rounded,
        &stats,
        seed_metrics,
        rounded_metrics,
    )?;
    Ok(())
}

fn load_samples(reader: impl BufRead) -> Result<(Vec<Sample>, Vec<Sample>, LoadStats), io::Error> {
    let mut train = Vec::new();
    let mut validation = Vec::new();
    let mut seen = HashSet::new();
    let mut stats = LoadStats {
        fnv1a: 0xcbf2_9ce4_8422_2325,
        ..LoadStats::default()
    };

    for line in reader.lines() {
        let line = line?;
        stats.lines += 1;
        for byte in line.bytes().chain(std::iter::once(b'\n')) {
            stats.fnv1a ^= u64::from(byte);
            stats.fnv1a = stats.fnv1a.wrapping_mul(0x100_0000_01b3);
        }

        let mut fields = line.split('|').map(str::trim);
        let (Some(fen), Some(raw_cp), Some(result), None) =
            (fields.next(), fields.next(), fields.next(), fields.next())
        else {
            stats.malformed += 1;
            continue;
        };
        let (Ok(raw_cp), Ok(result)) = (raw_cp.parse::<f64>(), result.parse::<f64>()) else {
            stats.malformed += 1;
            continue;
        };
        if !matches!(result, 0.0 | 0.5 | 1.0) {
            stats.malformed += 1;
            continue;
        }
        if !raw_cp.is_finite() || raw_cp.abs() > MAX_RAW_CP {
            stats.outlier += 1;
            continue;
        }

        let Ok(mut position) = Position::from_fen(fen) else {
            stats.invalid_fen += 1;
            continue;
        };
        let bucket = mix64(position.key()) % SAMPLE_BUCKETS;
        let is_train = bucket < TRAIN_BUCKETS;
        let is_validation = bucket == VALIDATION_BUCKET;
        if !is_train && !is_validation {
            stats.unsampled += 1;
            continue;
        }
        if !seen.insert(position.key()) {
            stats.duplicate += 1;
            continue;
        }
        if position.in_check(position.side_to_move()) {
            stats.in_check += 1;
            continue;
        }
        let legal = position.legal_moves();
        if legal.is_empty() {
            stats.terminal += 1;
            continue;
        }
        if legal.as_slice().iter().any(|mv| {
            mv.promotion().is_some()
                || mv.kind() == MoveKind::EnPassant
                || position.piece_at(mv.to()).is_some()
        }) {
            stats.tactical += 1;
            continue;
        }

        let sample = sample_from_position(&position, raw_cp / TEACHER_SCALE);
        if is_train {
            train.push(sample);
            stats.train += 1;
        } else {
            validation.push(sample);
            stats.validation += 1;
        }
    }
    Ok((train, validation, stats))
}

fn sample_from_position(position: &Position, target: f64) -> Sample {
    let mut pieces = Vec::with_capacity(position.occupied().count_ones() as usize);
    let mut phase = 0u8;
    for color in Color::ALL {
        for kind in PieceKind::ALL {
            let mut occupied = position.pieces(color, kind);
            phase =
                phase.saturating_add(phase_value(kind).saturating_mul(occupied.count_ones() as u8));
            while occupied != 0 {
                let square = occupied.trailing_zeros() as usize;
                occupied &= occupied - 1;
                pieces.push(Feature {
                    kind: kind.index(),
                    square: if color == Color::White {
                        square
                    } else {
                        square ^ 56
                    },
                    sign: if color == Color::White { 1.0 } else { -1.0 },
                });
            }
        }
    }
    let middlegame = f64::from(phase.min(MAX_PHASE as u8)) / MAX_PHASE;
    Sample {
        pieces,
        middlegame,
        endgame: 1.0 - middlegame,
        target,
    }
}

fn predict(sample: &Sample, weights: &[f64]) -> f64 {
    let mut middlegame = 0.0;
    let mut endgame = 0.0;
    for piece in &sample.pieces {
        middlegame += piece.sign
            * (weights[index(0, piece.kind, 0)] + weights[index(0, piece.kind, 1 + piece.square)]);
        endgame += piece.sign
            * (weights[index(1, piece.kind, 0)] + weights[index(1, piece.kind, 1 + piece.square)]);
    }
    middlegame * sample.middlegame + endgame * sample.endgame
}

fn predict_i16(sample: &Sample, weights: &[i16]) -> f64 {
    let mut middlegame = 0.0;
    let mut endgame = 0.0;
    for piece in &sample.pieces {
        middlegame += piece.sign
            * f64::from(
                weights[index(0, piece.kind, 0)] + weights[index(0, piece.kind, 1 + piece.square)],
            );
        endgame += piece.sign
            * f64::from(
                weights[index(1, piece.kind, 0)] + weights[index(1, piece.kind, 1 + piece.square)],
            );
    }
    middlegame * sample.middlegame + endgame * sample.endgame
}

fn accumulate_gradient(sample: &Sample, derivative: f64, gradient: &mut [f64]) {
    for piece in &sample.pieces {
        let middlegame = derivative * piece.sign * sample.middlegame;
        gradient[index(0, piece.kind, 0)] += middlegame;
        gradient[index(0, piece.kind, 1 + piece.square)] += middlegame;
        let endgame = derivative * piece.sign * sample.endgame;
        gradient[index(1, piece.kind, 0)] += endgame;
        gradient[index(1, piece.kind, 1 + piece.square)] += endgame;
    }
}

fn metrics(samples: &[Sample], weights: &[f64]) -> Metrics {
    let mut absolute = 0.0;
    let mut squared = 0.0;
    for sample in samples {
        let error = predict(sample, weights) - sample.target;
        absolute += error.abs();
        squared += error * error;
    }
    Metrics {
        mae: absolute / samples.len() as f64,
        rmse: (squared / samples.len() as f64).sqrt(),
    }
}

fn metrics_i16(samples: &[Sample], weights: &[i16]) -> Metrics {
    let mut absolute = 0.0;
    let mut squared = 0.0;
    for sample in samples {
        let error = predict_i16(sample, weights) - sample.target;
        absolute += error.abs();
        squared += error * error;
    }
    Metrics {
        mae: absolute / samples.len() as f64,
        rmse: (squared / samples.len() as f64).sqrt(),
    }
}

fn seed_weights() -> Vec<f64> {
    let mut weights = vec![0.0; PARAMS];
    for kind in PieceKind::ALL {
        weights[index(0, kind.index(), 0)] = f64::from(SEED_MIDDLEGAME_VALUE[kind.index()]);
        weights[index(1, kind.index(), 0)] = f64::from(SEED_ENDGAME_VALUE[kind.index()]);
        for square in 0..SQUARES {
            weights[index(0, kind.index(), 1 + square)] =
                f64::from(SEED_MIDDLEGAME_PIECE_SQUARE[kind.index()][square ^ 56]);
            weights[index(1, kind.index(), 1 + square)] =
                f64::from(SEED_ENDGAME_PIECE_SQUARE[kind.index()][square ^ 56]);
        }
    }
    weights
}

const fn phase_value(kind: PieceKind) -> u8 {
    match kind {
        PieceKind::Pawn | PieceKind::King => 0,
        PieceKind::Knight | PieceKind::Bishop => 1,
        PieceKind::Rook => 2,
        PieceKind::Queen => 4,
    }
}

const fn index(phase: usize, kind: usize, feature: usize) -> usize {
    phase * KINDS * STRIDE + kind * STRIDE + feature
}

fn rounded_weights(weights: &[f64]) -> Vec<i16> {
    weights
        .iter()
        .map(|value| {
            value
                .round()
                .clamp(f64::from(i16::MIN), f64::from(i16::MAX)) as i16
        })
        .collect()
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

fn emit_constants(
    mut output: impl Write,
    weights: &[i16],
    stats: &LoadStats,
    seed: Metrics,
    tuned: Metrics,
) -> io::Result<()> {
    writeln!(
        output,
        "//! Generated by `ai-chess-psqt-tuner`; do not hand-edit.\n\
         //!\n\
         //! Source rows: {} (FNV-1a {:016x}); quiet train/validation: {}/{}.\n\
         //! Held-out teacher error: seed {:.2}/{:.2} MAE/RMSE cp; tuned {:.2}/{:.2}.\n\
         //! Tables use white-relative a1..h8 order. Black mirrors ranks.\n",
        stats.lines,
        stats.fnv1a,
        stats.train,
        stats.validation,
        seed.mae,
        seed.rmse,
        tuned.mae,
        tuned.rmse
    )?;
    for (name, phase) in [("MIDDLEGAME", 0), ("ENDGAME", 1)] {
        write!(output, "pub const {name}_VALUE: [i32; 6] = [")?;
        for kind in 0..KINDS {
            if kind != 0 {
                write!(output, ", ")?;
            }
            write!(output, "{}", weights[index(phase, kind, 0)])?;
        }
        writeln!(output, "];")?;
        writeln!(
            output,
            "#[rustfmt::skip]\npub const {name}_PIECE_SQUARE: [[i16; 64]; 6] = ["
        )?;
        for kind in 0..KINDS {
            writeln!(output, "    [")?;
            for rank in 0..8 {
                write!(output, "        ")?;
                for file in 0..8 {
                    if file != 0 {
                        write!(output, ", ")?;
                    }
                    write!(
                        output,
                        "{:>4}",
                        weights[index(phase, kind, 1 + rank * 8 + file)]
                    )?;
                }
                writeln!(output, ",")?;
            }
            writeln!(output, "    ],")?;
        }
        writeln!(output, "];\n")?;
    }
    Ok(())
}
