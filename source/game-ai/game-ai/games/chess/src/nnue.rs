//! A deliberately small, scalar NNUE foundation.
//!
//! The feature transformer is `768 → 128`. One shared transformer produces
//! White- and Black-perspective accumulators; evaluation concatenates
//! side-to-move first and applies one clipped-ReLU output head. The expensive
//! first layer can be updated by adding/removing only the rows touched by a
//! move.

use crate::{Color, Move, MoveKind, Piece, PieceKind, Position, Square};
use std::fmt;
use std::sync::LazyLock;

pub const FEATURE_COUNT: usize = 2 * 6 * 64;
pub const HIDDEN: usize = 128;
pub(crate) const OUTPUT_INPUTS: usize = HIDDEN * 2;
// This teaching net keeps scalar `i32` accumulators, so a finer transformer
// scale is safe and materially reduces rounding error in a small trained net.
pub const QA: i32 = 4096;
pub const QB: i32 = 64;

const MAGIC: [u8; 8] = *b"GAINNUE\0";
const VERSION: u16 = 1;
const ACTIVATION_CRELU: u8 = 1;
const HEADER_BYTES: usize = 32;
const PAYLOAD_BYTES: usize = HIDDEN * 2 + FEATURE_COUNT * HIDDEN * 2 + OUTPUT_INPUTS * 2 + 4;

static BUILTIN_NETWORK: LazyLock<QuantizedNetwork> = LazyLock::new(|| {
    QuantizedNetwork::from_bytes(include_bytes!("../networks/tiny-v1.gainnue"))
        .expect("the embedded tiny NNUE must pass its version and checksum gates")
});

pub fn builtin_network() -> &'static QuantizedNetwork {
    &BUILTIN_NETWORK
}

/// Trainable floating-point form of the fixed tiny NNUE architecture.
#[derive(Debug, Clone, PartialEq)]
pub struct FloatNetwork {
    feature_bias: Vec<f32>,
    feature_weights: Vec<f32>,
    output_weights: Vec<f32>,
    output_bias: f32,
}

impl FloatNetwork {
    pub fn new(
        feature_bias: Vec<f32>,
        feature_weights: Vec<f32>,
        output_weights: Vec<f32>,
        output_bias: f32,
    ) -> Result<Self, NetworkError> {
        if feature_bias.len() != HIDDEN
            || feature_weights.len() != FEATURE_COUNT * HIDDEN
            || output_weights.len() != OUTPUT_INPUTS
        {
            return Err(NetworkError::Dimensions);
        }
        if !output_bias.is_finite()
            || feature_bias.iter().any(|value| !value.is_finite())
            || feature_weights.iter().any(|value| !value.is_finite())
            || output_weights.iter().any(|value| !value.is_finite())
        {
            return Err(NetworkError::NonFinite);
        }
        Ok(Self {
            feature_bias,
            feature_weights,
            output_weights,
            output_bias,
        })
    }

    pub fn quantize(&self) -> QuantizedNetwork {
        QuantizedNetwork {
            feature_bias: self
                .feature_bias
                .iter()
                .map(|value| quantize_i16(*value, QA))
                .collect(),
            feature_weights: self
                .feature_weights
                .iter()
                .map(|value| quantize_i16(*value, QA))
                .collect(),
            output_weights: self
                .output_weights
                .iter()
                .map(|value| quantize_i16(*value, QB))
                .collect(),
            output_bias: quantize_i32(self.output_bias, QA * QB),
        }
    }

    pub fn evaluate(&self, position: &Position) -> f32 {
        let mut accumulators = [self.feature_bias.clone(), self.feature_bias.clone()];
        for square in Square::all() {
            let Some(piece) = position.piece_at(square) else {
                continue;
            };
            for perspective in Color::ALL {
                let feature = feature_index(piece, square, perspective);
                let weights = &self.feature_weights[feature * HIDDEN..(feature + 1) * HIDDEN];
                for (value, weight) in accumulators[perspective.index()].iter_mut().zip(weights) {
                    *value += weight;
                }
            }
        }
        let us = position.side_to_move().index();
        let them = position.side_to_move().other().index();
        let mut output = self.output_bias;
        for (lane, weight) in accumulators[us]
            .iter()
            .chain(&accumulators[them])
            .zip(&self.output_weights)
        {
            output += lane.clamp(0.0, 1.0) * weight;
        }
        output
    }
}

/// Serialized integer form used by the runtime evaluator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuantizedNetwork {
    feature_bias: Vec<i16>,
    feature_weights: Vec<i16>,
    output_weights: Vec<i16>,
    output_bias: i32,
}

impl QuantizedNetwork {
    /// Parses a versioned network and validates its dimensions and checksum.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, NetworkError> {
        if bytes.len() < HEADER_BYTES {
            return Err(NetworkError::Truncated);
        }
        if bytes[..8] != MAGIC {
            return Err(NetworkError::Magic);
        }
        let version = read_u16(bytes, 8)?;
        if version != VERSION {
            return Err(NetworkError::Version(version));
        }
        if usize::from(read_u16(bytes, 10)?) != FEATURE_COUNT
            || usize::from(read_u16(bytes, 12)?) != HIDDEN
        {
            return Err(NetworkError::Dimensions);
        }
        if bytes[14] != ACTIVATION_CRELU {
            return Err(NetworkError::Activation(bytes[14]));
        }
        if i32::from(read_u16(bytes, 16)?) != QA || i32::from(read_u16(bytes, 18)?) != QB {
            return Err(NetworkError::Scale);
        }
        let payload_len = read_u32(bytes, 20)? as usize;
        if payload_len != PAYLOAD_BYTES || bytes.len() != HEADER_BYTES + payload_len {
            return Err(NetworkError::Dimensions);
        }
        let payload = &bytes[HEADER_BYTES..];
        let expected_checksum = read_u64(bytes, 24)?;
        let actual_checksum = fnv1a(payload);
        if actual_checksum != expected_checksum {
            return Err(NetworkError::Checksum {
                expected: expected_checksum,
                actual: actual_checksum,
            });
        }

        let mut cursor = 0;
        let feature_bias = read_i16s(payload, &mut cursor, HIDDEN)?;
        let feature_weights = read_i16s(payload, &mut cursor, FEATURE_COUNT * HIDDEN)?;
        let output_weights = read_i16s(payload, &mut cursor, OUTPUT_INPUTS)?;
        let output_bias = read_i32(payload, cursor)?;
        cursor += 4;
        if cursor != payload.len() {
            return Err(NetworkError::Dimensions);
        }
        Ok(Self {
            feature_bias,
            feature_weights,
            output_weights,
            output_bias,
        })
    }

    /// Serializes the network in the versioned `GAINNUE` format.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut payload = Vec::with_capacity(PAYLOAD_BYTES);
        for value in self
            .feature_bias
            .iter()
            .chain(&self.feature_weights)
            .chain(&self.output_weights)
        {
            payload.extend_from_slice(&value.to_le_bytes());
        }
        payload.extend_from_slice(&self.output_bias.to_le_bytes());
        debug_assert_eq!(payload.len(), PAYLOAD_BYTES);

        let mut bytes = Vec::with_capacity(HEADER_BYTES + PAYLOAD_BYTES);
        bytes.extend_from_slice(&MAGIC);
        bytes.extend_from_slice(&VERSION.to_le_bytes());
        bytes.extend_from_slice(&(FEATURE_COUNT as u16).to_le_bytes());
        bytes.extend_from_slice(&(HIDDEN as u16).to_le_bytes());
        bytes.push(ACTIVATION_CRELU);
        bytes.push(0);
        bytes.extend_from_slice(&(QA as u16).to_le_bytes());
        bytes.extend_from_slice(&(QB as u16).to_le_bytes());
        bytes.extend_from_slice(&(PAYLOAD_BYTES as u32).to_le_bytes());
        bytes.extend_from_slice(&fnv1a(&payload).to_le_bytes());
        debug_assert_eq!(bytes.len(), HEADER_BYTES);
        bytes.extend_from_slice(&payload);
        bytes
    }

    pub fn checksum(&self) -> u64 {
        fnv1a(&self.to_bytes()[HEADER_BYTES..])
    }

    pub fn evaluate_refresh(&self, position: &Position) -> i32 {
        Accumulator::refresh(position, self).evaluate(position.side_to_move(), self)
    }
}

/// Incrementally maintainable pair of perspective accumulators.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Accumulator {
    values: [[i32; HIDDEN]; 2],
}

impl Accumulator {
    pub fn refresh(position: &Position, network: &QuantizedNetwork) -> Self {
        let mut bias = [0; HIDDEN];
        for (lane, value) in bias.iter_mut().zip(&network.feature_bias) {
            *lane = i32::from(*value);
        }
        let mut accumulator = Self {
            values: [bias, bias],
        };
        for square in Square::all() {
            if let Some(piece) = position.piece_at(square) {
                accumulator.update_feature(network, piece, square, 1);
            }
        }
        accumulator
    }

    pub fn apply(&mut self, network: &QuantizedNetwork, delta: &FeatureDelta) {
        for change in delta.as_slice() {
            self.update_feature(network, change.piece, change.square, i32::from(change.sign));
        }
    }

    pub fn revert(&mut self, network: &QuantizedNetwork, delta: &FeatureDelta) {
        for change in delta.as_slice().iter().rev() {
            self.update_feature(
                network,
                change.piece,
                change.square,
                -i32::from(change.sign),
            );
        }
    }

    pub fn evaluate(&self, side_to_move: Color, network: &QuantizedNetwork) -> i32 {
        let us = side_to_move.index();
        let them = side_to_move.other().index();
        let mut sum = i64::from(network.output_bias);
        for (lane, weight) in self.values[us]
            .iter()
            .chain(&self.values[them])
            .zip(&network.output_weights)
        {
            let activated = (*lane).clamp(0, QA);
            sum += i64::from(activated) * i64::from(*weight);
        }
        rounded_division(sum, i64::from(QA * QB)) as i32
    }

    pub fn perspective(&self, perspective: Color) -> &[i32] {
        &self.values[perspective.index()]
    }

    fn update_feature(
        &mut self,
        network: &QuantizedNetwork,
        piece: Piece,
        square: Square,
        sign: i32,
    ) {
        for perspective in Color::ALL {
            let feature = feature_index(piece, square, perspective);
            let weights = &network.feature_weights[feature * HIDDEN..(feature + 1) * HIDDEN];
            for (value, weight) in self.values[perspective.index()].iter_mut().zip(weights) {
                *value += sign * i32::from(*weight);
            }
        }
    }
}

/// Bounded set of NNUE feature changes caused by one chess move.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeatureDelta {
    changes: [FeatureChange; Self::CAPACITY],
    len: u8,
}

impl FeatureDelta {
    const CAPACITY: usize = 5;

    pub fn from_move(position: &Position, mv: Move) -> Result<Self, &'static str> {
        let moved = position
            .piece_at(mv.from())
            .ok_or("NNUE delta move has no source piece")?;
        if moved.color != position.side_to_move() {
            return Err("NNUE delta move belongs to the wrong side");
        }
        let mut delta = Self {
            changes: [FeatureChange::PLACEHOLDER; Self::CAPACITY],
            len: 0,
        };
        delta.push(FeatureChange::remove(moved, mv.from()));

        match mv.kind() {
            MoveKind::EnPassant => {
                let captured_square = mv
                    .to()
                    .offset(0, -moved.color.pawn_step())
                    .ok_or("en-passant capture square is outside the board")?;
                let captured = position
                    .piece_at(captured_square)
                    .ok_or("en-passant delta has no captured pawn")?;
                delta.push(FeatureChange::remove(captured, captured_square));
            }
            MoveKind::Normal | MoveKind::Promotion(_) => {
                if let Some(captured) = position.piece_at(mv.to()) {
                    delta.push(FeatureChange::remove(captured, mv.to()));
                }
            }
            MoveKind::CastleKingSide | MoveKind::CastleQueenSide => {}
        }

        let placed = Piece::new(moved.color, mv.promotion().unwrap_or(moved.kind));
        delta.push(FeatureChange::add(placed, mv.to()));

        match mv.kind() {
            MoveKind::CastleKingSide => {
                let rank = moved.color.home_rank();
                let from = Square::from_file_rank(7, rank);
                let to = Square::from_file_rank(5, rank);
                let rook = position
                    .piece_at(from)
                    .ok_or("king-side castle delta has no rook")?;
                delta.push(FeatureChange::remove(rook, from));
                delta.push(FeatureChange::add(rook, to));
            }
            MoveKind::CastleQueenSide => {
                let rank = moved.color.home_rank();
                let from = Square::from_file_rank(0, rank);
                let to = Square::from_file_rank(3, rank);
                let rook = position
                    .piece_at(from)
                    .ok_or("queen-side castle delta has no rook")?;
                delta.push(FeatureChange::remove(rook, from));
                delta.push(FeatureChange::add(rook, to));
            }
            MoveKind::Normal | MoveKind::EnPassant | MoveKind::Promotion(_) => {}
        }
        Ok(delta)
    }

    pub fn changes(&self) -> impl ExactSizeIterator<Item = (Piece, Square, i8)> + '_ {
        self.as_slice()
            .iter()
            .map(|change| (change.piece, change.square, change.sign))
    }

    fn push(&mut self, change: FeatureChange) {
        debug_assert!((self.len as usize) < Self::CAPACITY);
        self.changes[self.len as usize] = change;
        self.len += 1;
    }

    fn as_slice(&self) -> &[FeatureChange] {
        &self.changes[..self.len as usize]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FeatureChange {
    piece: Piece,
    square: Square,
    sign: i8,
}

impl FeatureChange {
    const PLACEHOLDER: Self =
        Self::remove(Piece::new(Color::White, PieceKind::Pawn), Square::new(0));

    const fn add(piece: Piece, square: Square) -> Self {
        Self {
            piece,
            square,
            sign: 1,
        }
    }

    const fn remove(piece: Piece, square: Square) -> Self {
        Self {
            piece,
            square,
            sign: -1,
        }
    }
}

/// Shared feature transformer index, normalized into `perspective`'s view.
///
/// Feature planes are `[our P..K, their P..K]`, each in a1..h8 order.
pub const fn feature_index(piece: Piece, square: Square, perspective: Color) -> usize {
    let relative_color = if piece.color as u8 == perspective as u8 {
        0
    } else {
        1
    };
    let relative_square = match perspective {
        Color::White => square.index(),
        Color::Black => square.index() ^ 56,
    };
    (relative_color * 6 + piece.kind.index()) * 64 + relative_square
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NetworkError {
    Truncated,
    Magic,
    Version(u16),
    Dimensions,
    Activation(u8),
    Scale,
    Checksum { expected: u64, actual: u64 },
    NonFinite,
}

impl fmt::Display for NetworkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Truncated => f.write_str("network file is truncated"),
            Self::Magic => f.write_str("network magic does not match GAINNUE"),
            Self::Version(version) => write!(f, "unsupported network version {version}"),
            Self::Dimensions => f.write_str("network dimensions or payload length do not match"),
            Self::Activation(activation) => {
                write!(f, "unsupported network activation {activation}")
            }
            Self::Scale => f.write_str("network quantization scales do not match"),
            Self::Checksum { expected, actual } => write!(
                f,
                "network checksum mismatch: expected {expected:016x}, got {actual:016x}"
            ),
            Self::NonFinite => f.write_str("float network contains a non-finite value"),
        }
    }
}

impl std::error::Error for NetworkError {}

fn quantize_i16(value: f32, scale: i32) -> i16 {
    let scaled = (value * scale as f32).round();
    if scaled < i16::MIN as f32 {
        i16::MIN
    } else if scaled > i16::MAX as f32 {
        i16::MAX
    } else {
        scaled as i16
    }
}

fn quantize_i32(value: f32, scale: i32) -> i32 {
    let scaled = (value * scale as f32).round();
    if scaled < i32::MIN as f32 {
        i32::MIN
    } else if scaled > i32::MAX as f32 {
        i32::MAX
    } else {
        scaled as i32
    }
}

const fn rounded_division(numerator: i64, denominator: i64) -> i64 {
    if numerator >= 0 {
        (numerator + denominator / 2) / denominator
    } else {
        -((-numerator + denominator / 2) / denominator)
    }
}

fn fnv1a(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100_0000_01b3);
    }
    hash
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16, NetworkError> {
    let value = bytes
        .get(offset..offset + 2)
        .ok_or(NetworkError::Truncated)?;
    Ok(u16::from_le_bytes([value[0], value[1]]))
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, NetworkError> {
    let value = bytes
        .get(offset..offset + 4)
        .ok_or(NetworkError::Truncated)?;
    Ok(u32::from_le_bytes([value[0], value[1], value[2], value[3]]))
}

fn read_u64(bytes: &[u8], offset: usize) -> Result<u64, NetworkError> {
    let value = bytes
        .get(offset..offset + 8)
        .ok_or(NetworkError::Truncated)?;
    Ok(u64::from_le_bytes([
        value[0], value[1], value[2], value[3], value[4], value[5], value[6], value[7],
    ]))
}

fn read_i32(bytes: &[u8], offset: usize) -> Result<i32, NetworkError> {
    let value = bytes
        .get(offset..offset + 4)
        .ok_or(NetworkError::Truncated)?;
    Ok(i32::from_le_bytes([value[0], value[1], value[2], value[3]]))
}

fn read_i16s(bytes: &[u8], cursor: &mut usize, count: usize) -> Result<Vec<i16>, NetworkError> {
    let byte_count = count.checked_mul(2).ok_or(NetworkError::Dimensions)?;
    let values = bytes
        .get(*cursor..*cursor + byte_count)
        .ok_or(NetworkError::Truncated)?;
    let parsed = values
        .chunks_exact(2)
        .map(|bytes| i16::from_le_bytes([bytes[0], bytes[1]]))
        .collect();
    *cursor += byte_count;
    Ok(parsed)
}
