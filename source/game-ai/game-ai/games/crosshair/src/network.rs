use crate::{CELLS, Color, Move, Position};
use std::path::Path;

/// Number of spatial feature planes encoded per board position.
pub const INPUT_CHANNELS: usize = 6;

/// Total number of `f32` inputs to the network (`6 * 49 = 294`).
pub const INPUTS: usize = INPUT_CHANNELS * CELLS;

/// Width of the stem and residual trunk hidden layers.
pub const HIDDEN: usize = 128;

/// Width of the value head bottleneck hidden layer.
pub const VALUE_HIDDEN: usize = 32;

/// Number of discrete board actions (`49`).
pub const ACTIONS: usize = CELLS;

/// Size in bytes of the `CXHRNET1` binary header.
pub const HEADER_BYTES: usize = 32;

/// Magic header bytes for serialized Crosshair policy-value networks.
pub const MAGIC: &[u8; 8] = b"CXHRNET1";

/// Binary format version for `CXHRNET1`.
pub const VERSION: u32 = 1;

/// Parameter offset of `W_stem` (`[128, 294]`).
pub const W_STEM: usize = 0;
/// Parameter offset of `b_stem` (`[128]`).
pub const B_STEM: usize = W_STEM + HIDDEN * INPUTS;

/// Parameter offset of `W_res1a` (`[128, 128]`).
pub const W_RES1A: usize = B_STEM + HIDDEN;
/// Parameter offset of `b_res1a` (`[128]`).
pub const B_RES1A: usize = W_RES1A + HIDDEN * HIDDEN;
/// Parameter offset of `W_res1b` (`[128, 128]`).
pub const W_RES1B: usize = B_RES1A + HIDDEN;
/// Parameter offset of `b_res1b` (`[128]`).
pub const B_RES1B: usize = W_RES1B + HIDDEN * HIDDEN;

/// Parameter offset of `W_res2a` (`[128, 128]`).
pub const W_RES2A: usize = B_RES1B + HIDDEN;
/// Parameter offset of `b_res2a` (`[128]`).
pub const B_RES2A: usize = W_RES2A + HIDDEN * HIDDEN;
/// Parameter offset of `W_res2b` (`[128, 128]`).
pub const W_RES2B: usize = B_RES2A + HIDDEN;
/// Parameter offset of `b_res2b` (`[128]`).
pub const B_RES2B: usize = W_RES2B + HIDDEN * HIDDEN;

/// Parameter offset of `W_pol` (`[49, 128]`).
pub const W_POL: usize = B_RES2B + HIDDEN;
/// Parameter offset of `b_pol` (`[49]`).
pub const B_POL: usize = W_POL + ACTIONS * HIDDEN;

/// Parameter offset of `W_val1` (`[32, 128]`).
pub const W_VAL1: usize = B_POL + ACTIONS;
/// Parameter offset of `b_val1` (`[32]`).
pub const B_VAL1: usize = W_VAL1 + VALUE_HIDDEN * HIDDEN;
/// Parameter offset of `W_val2` (`[1, 32]`).
pub const W_VAL2: usize = B_VAL1 + VALUE_HIDDEN;
/// Parameter offset of `b_val2` (`[1]`).
pub const B_VAL2: usize = W_VAL2 + VALUE_HIDDEN;

/// Total number of `f32` parameters in a `PolicyValueNetwork` (`114,290`).
pub const PARAM_COUNT: usize = B_VAL2 + 1;

const EMBEDDED_NETWORK: &[u8] = include_bytes!("networks/az-default.bin");

/// Two-residual-block MLP policy-value network for Crosshair (`CXHRNET1`).
#[derive(Clone, Debug, PartialEq)]
pub struct PolicyValueNetwork {
    parameters: Box<[f32]>,
}

impl PolicyValueNetwork {
    /// Loads the compile-time embedded default network (`src/networks/az-default.bin`).
    pub fn embedded() -> Result<Self, &'static str> {
        Self::from_bytes(EMBEDDED_NETWORK)
    }

    /// Parses and validates a `CXHRNET1` binary artifact from raw bytes.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, &'static str> {
        if bytes.len() != HEADER_BYTES + PARAM_COUNT * size_of::<f32>() {
            return Err("wrong policy-value network byte length");
        }
        if &bytes[..8] != MAGIC {
            return Err("wrong policy-value network magic");
        }
        if read_u32(bytes, 8) != VERSION {
            return Err("unsupported policy-value network version");
        }
        if usize::from(read_u16(bytes, 12)) != INPUTS
            || usize::from(read_u16(bytes, 14)) != HIDDEN
            || usize::from(read_u16(bytes, 16)) != ACTIONS
            || usize::from(read_u16(bytes, 18)) != VALUE_HIDDEN
            || read_u32(bytes, 20) as usize != PARAM_COUNT
        {
            return Err("wrong policy-value network shape");
        }
        let payload = &bytes[HEADER_BYTES..];
        if read_u64(bytes, 24) != checksum(payload) {
            return Err("policy-value network checksum mismatch");
        }
        let parameters = payload
            .chunks_exact(4)
            .map(|chunk| f32::from_le_bytes(chunk.try_into().expect("chunks are four bytes")))
            .collect::<Vec<_>>()
            .into_boxed_slice();
        if parameters.iter().any(|value| !value.is_finite()) {
            return Err("policy-value network contains a non-finite weight");
        }
        Ok(Self { parameters })
    }

    /// Loads and validates a `CXHRNET1` binary artifact from disk.
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, String> {
        let path = path.as_ref();
        let bytes = std::fs::read(path)
            .map_err(|err| format!("failed to read network file {}: {err}", path.display()))?;
        Self::from_bytes(&bytes).map_err(|err| format!("{}: {err}", path.display()))
    }

    /// Serializes the network into the 32-byte `CXHRNET1` header followed by `114,290` LE `f32`s.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut payload = Vec::with_capacity(PARAM_COUNT * size_of::<f32>());
        for &parameter in &self.parameters {
            payload.extend_from_slice(&parameter.to_le_bytes());
        }
        let mut bytes = Vec::with_capacity(HEADER_BYTES + payload.len());
        bytes.extend_from_slice(MAGIC);
        bytes.extend_from_slice(&VERSION.to_le_bytes());
        bytes.extend_from_slice(&(INPUTS as u16).to_le_bytes());
        bytes.extend_from_slice(&(HIDDEN as u16).to_le_bytes());
        bytes.extend_from_slice(&(ACTIONS as u16).to_le_bytes());
        bytes.extend_from_slice(&(VALUE_HIDDEN as u16).to_le_bytes());
        bytes.extend_from_slice(&(PARAM_COUNT as u32).to_le_bytes());
        bytes.extend_from_slice(&checksum(&payload).to_le_bytes());
        bytes.extend_from_slice(&payload);
        bytes
    }

    /// Returns a slice of all `114,290` `f32` network parameters.
    pub fn parameters(&self) -> &[f32] {
        &self.parameters
    }

    /// Deterministically initializes a network from `seed` with Xavier/He scaling and tactical priors.
    pub fn seeded(seed: u64) -> Self {
        let mut rng = SplitMix64(seed);
        let mut params = vec![0.0_f32; PARAM_COUNT].into_boxed_slice();

        let stem_scale = (2.0 / (INPUTS + HIDDEN) as f32).sqrt() * 0.25;
        for w in &mut params[W_STEM..B_STEM] {
            *w = rng.signed() * stem_scale;
        }
        let res_scale = (2.0 / (HIDDEN + HIDDEN) as f32).sqrt() * 0.2;
        for offset in [W_RES1A, W_RES1B, W_RES2A, W_RES2B] {
            for w in &mut params[offset..offset + HIDDEN * HIDDEN] {
                *w = rng.signed() * res_scale;
            }
        }
        let pol_scale = (2.0 / (HIDDEN + ACTIONS) as f32).sqrt() * 0.25;
        for w in &mut params[W_POL..B_POL] {
            *w = rng.signed() * pol_scale;
        }
        let val1_scale = (2.0 / (HIDDEN + VALUE_HIDDEN) as f32).sqrt() * 0.25;
        for w in &mut params[W_VAL1..B_VAL1] {
            *w = rng.signed() * val1_scale;
        }
        let val2_scale = (2.0 / (VALUE_HIDDEN + 1) as f32).sqrt() * 0.25;
        for w in &mut params[W_VAL2..B_VAL2] {
            *w = rng.signed() * val2_scale;
        }

        // Embed spatial/tactical inductive bias into hidden units 0..51 so the initial
        // checkpoint immediately favors linear & 2x2 crosscut flips, central connectivity, and stone lead.
        for cell in 0..CELLS {
            let file = (cell % 7) as f32 - 3.0;
            let rank = (cell / 7) as f32 - 3.0;
            let center_bonus = (1.0 - (file * file + rank * rank) / 18.0).max(0.0);
            let stem_row = W_STEM + cell * INPUTS;
            // Channel 2: legal move indicator, Channel 3: normalized flips if played
            params[stem_row + 2 * CELLS + cell] += 0.6 + 0.35 * center_bonus;
            params[stem_row + 3 * CELLS + cell] += 2.6;
            params[W_POL + cell * HIDDEN + cell] += 1.6;
            params[B_POL + cell] += 0.15 * center_bonus;
        }
        let own_unit = 49;
        let opp_unit = 50;
        for cell in 0..CELLS {
            params[W_STEM + own_unit * INPUTS + cell] += 0.18;
            params[W_STEM + own_unit * INPUTS + 3 * CELLS + cell] += 0.35;
            params[W_STEM + opp_unit * INPUTS + CELLS + cell] += 0.18;
        }
        params[W_VAL1 + own_unit] += 0.9;
        params[W_VAL1 + opp_unit] -= 0.9;
        params[W_VAL1 + HIDDEN + opp_unit] += 0.9;
        params[W_VAL1 + HIDDEN + own_unit] -= 0.9;
        params[W_VAL2] += 0.85;
        params[W_VAL2 + 1] -= 0.85;

        Self { parameters: params }
    }

    /// Evaluates `position` and returns `(policy_priors, value)` where `policy_priors` sums to `1.0`
    /// over legal moves in real board coordinates and `value` is in `[-1.0, 1.0]` from `side_to_move`'s perspective.
    pub fn evaluate(&self, position: Position) -> ([f64; ACTIONS], f64) {
        let side = position.side_to_move();
        let input = encode(position, 0);
        let legal_moves = position.legal_moves();
        let mut canonical_legal = [false; ACTIONS];
        for mv in legal_moves {
            let c = canonical_index(mv.index(), side, 0) as usize;
            canonical_legal[c] = true;
        }
        let (canonical_priors, value) = self.evaluate_encoded(&input, &canonical_legal);
        let mut real_priors = [0.0_f64; ACTIONS];
        for idx in 0..CELLS as u8 {
            let c = canonical_index(idx, side, 0) as usize;
            real_priors[idx as usize] = canonical_priors[c];
        }
        (real_priors, value)
    }

    /// Runs the forward pass on an already-encoded canonical `[f32; 294]` input tensor and canonical legal-move mask.
    pub fn evaluate_encoded(
        &self,
        input: &[f32; INPUTS],
        legal: &[bool; ACTIONS],
    ) -> ([f64; ACTIONS], f64) {
        let h2 = self.trunk(input);

        let mut logits = [f32::NEG_INFINITY; ACTIONS];
        let mut max_logit = f32::NEG_INFINITY;
        let mut any_legal = false;
        for action in 0..ACTIONS {
            if !legal[action] {
                continue;
            }
            any_legal = true;
            let mut sum = self.parameters[B_POL + action];
            let row = &self.parameters[W_POL + action * HIDDEN..W_POL + (action + 1) * HIDDEN];
            for j in 0..HIDDEN {
                sum += row[j] * h2[j];
            }
            logits[action] = sum;
            if sum > max_logit {
                max_logit = sum;
            }
        }

        let mut priors = [0.0_f64; ACTIONS];
        if any_legal {
            let mut total = 0.0_f64;
            for action in 0..ACTIONS {
                if legal[action] {
                    let p = f64::from((logits[action] - max_logit).exp());
                    priors[action] = p;
                    total += p;
                }
            }
            if total > 0.0 {
                for prior in &mut priors {
                    *prior /= total;
                }
            }
        }

        let mut v_hid = [0.0_f32; VALUE_HIDDEN];
        for (i, out) in v_hid.iter_mut().enumerate() {
            let mut sum = self.parameters[B_VAL1 + i];
            let row = &self.parameters[W_VAL1 + i * HIDDEN..W_VAL1 + (i + 1) * HIDDEN];
            for j in 0..HIDDEN {
                sum += row[j] * h2[j];
            }
            *out = sum.max(0.0);
        }

        let mut v_raw = self.parameters[B_VAL2];
        let val2_row = &self.parameters[W_VAL2..W_VAL2 + VALUE_HIDDEN];
        for i in 0..VALUE_HIDDEN {
            v_raw += val2_row[i] * v_hid[i];
        }
        let value = f64::from(v_raw.tanh());

        (priors, value)
    }

    fn trunk(&self, input: &[f32; INPUTS]) -> [f32; HIDDEN] {
        let mut h0 = [0.0_f32; HIDDEN];
        for (i, out) in h0.iter_mut().enumerate() {
            let mut sum = self.parameters[B_STEM + i];
            let row = &self.parameters[W_STEM + i * INPUTS..W_STEM + (i + 1) * INPUTS];
            for j in 0..INPUTS {
                sum += row[j] * input[j];
            }
            *out = sum.max(0.0);
        }

        let u1 = linear_relu(&self.parameters, W_RES1A, B_RES1A, &h0);
        let h1 = residual_relu(&self.parameters, W_RES1B, B_RES1B, &h0, &u1);

        let u2 = linear_relu(&self.parameters, W_RES2A, B_RES2A, &h1);
        residual_relu(&self.parameters, W_RES2B, B_RES2B, &h1, &u2)
    }
}

#[inline]
fn linear_relu(
    params: &[f32],
    w_offset: usize,
    b_offset: usize,
    input: &[f32; HIDDEN],
) -> [f32; HIDDEN] {
    let mut out = [0.0_f32; HIDDEN];
    for (i, slot) in out.iter_mut().enumerate() {
        let mut sum = params[b_offset + i];
        let row = &params[w_offset + i * HIDDEN..w_offset + (i + 1) * HIDDEN];
        for j in 0..HIDDEN {
            sum += row[j] * input[j];
        }
        *slot = sum.max(0.0);
    }
    out
}

#[inline]
fn residual_relu(
    params: &[f32],
    w_offset: usize,
    b_offset: usize,
    skip: &[f32; HIDDEN],
    input: &[f32; HIDDEN],
) -> [f32; HIDDEN] {
    let mut out = [0.0_f32; HIDDEN];
    for (i, slot) in out.iter_mut().enumerate() {
        let mut sum = skip[i] + params[b_offset + i];
        let row = &params[w_offset + i * HIDDEN..w_offset + (i + 1) * HIDDEN];
        for j in 0..HIDDEN {
            sum += row[j] * input[j];
        }
        *slot = sum.max(0.0);
    }
    out
}

/// Encodes `position` into a canonical `294`-element `f32` vector under Klein four-group symmetry `symmetry` (`0..4`).
///
/// Red connects top (`rank 0`) to bottom (`rank 6`), whereas Blue connects left (`file 0`) to
/// right (`file 6`). When `position.side_to_move() == Color::Blue`, the board is transposed
/// `(file, rank) <-> (rank, file)` before applying `symmetry`. Because transposition swaps the
/// two pairs of target edges while preserving 4-orthogonal adjacency, crosshair constraints,
/// linear pinches, and 2×2 crosscuts, Channel 0 (`side_to_move`) always connects top to bottom
/// in canonical coordinates. Only the four Klein four-group ($V_4$) reflections are applied
/// afterward because 90° rotations would swap the top–bottom and left–right goal axes.
pub fn encode(position: Position, symmetry: u8) -> [f32; INPUTS] {
    let mut input = [0.0_f32; INPUTS];
    let side = position.side_to_move();
    let legal = position.legal_moves();
    let last_move = position.last_move();
    let ply_norm = f32::from(position.ply()) / (CELLS as f32);

    for idx in 0..CELLS as u8 {
        let mv = Move::from_index(idx);
        let t = canonical_index(idx, side, symmetry) as usize;
        match position.color_at(mv) {
            Some(color) if color == side => input[t] = 1.0,
            Some(_) => input[CELLS + t] = 1.0,
            None => {}
        }
        if legal.contains(mv) {
            input[2 * CELLS + t] = 1.0;
            let flips = (position.flips_for_move(mv).len() as f32).min(4.0);
            input[3 * CELLS + t] = flips / 4.0;
        }
        if last_move == Some(mv) {
            input[4 * CELLS + t] = 1.0;
        }
        input[5 * CELLS + t] = ply_norm;
    }
    input
}

/// Transposes `(file, rank)` to `(rank, file)` on the 7×7 grid (`index -> (index % 7) * 7 + (index / 7)`).
pub const fn transpose_index(index: u8) -> u8 {
    let width = 7_u8;
    let row = index / width;
    let column = index % width;
    column * width + row
}

/// Maps a real board index `index` (`0..49`) to canonical network coordinates for `side` under $V_4$ `symmetry`.
pub const fn canonical_index(index: u8, side: Color, symmetry: u8) -> u8 {
    let oriented = match side {
        Color::Red => index,
        Color::Blue => transpose_index(index),
    };
    transform_index(oriented, symmetry)
}

/// Maps a 7×7 cell index `index` (`0..49`) under Klein four-group ($V_4$) reflection `symmetry & 3`.
///
/// All 4 transformations preserve the Top–Bottom (`rank 0..6`) and Left–Right (`file 0..6`) axes:
/// - `0`: identity `(f, r)`
/// - `1`: horizontal flip `(6 - f, r)`
/// - `2`: vertical flip `(f, 6 - r)`
/// - `3`: 180° rotation `(6 - f, 6 - r)`
pub const fn transform_index(index: u8, symmetry: u8) -> u8 {
    let width = 7_u8;
    let row = index / width;
    let column = index % width;
    let last = width - 1;
    match symmetry & 3 {
        0 => row * width + column,
        1 => row * width + (last - column),
        2 => (last - row) * width + column,
        _ => (last - row) * width + (last - column),
    }
}

/// Computes the 64-bit FNV-1a checksum of `bytes`.
pub fn checksum(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for &byte in bytes {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

fn read_u16(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(
        bytes[offset..offset + 2]
            .try_into()
            .expect("checked header"),
    )
}

fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(
        bytes[offset..offset + 4]
            .try_into()
            .expect("checked header"),
    )
}

fn read_u64(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(
        bytes[offset..offset + 8]
            .try_into()
            .expect("checked header"),
    )
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

    fn signed(&mut self) -> f32 {
        let unit = (self.next() >> 40) as f32 / (1_u32 << 24) as f32;
        unit * 2.0 - 1.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parameter_count_matches_architecture_specification() {
        let expected = (294 * 128 + 128)
            + 4 * (128 * 128 + 128)
            + (49 * 128 + 49)
            + (32 * 128 + 32)
            + (32 + 1);
        assert_eq!(PARAM_COUNT, 114_290);
        assert_eq!(PARAM_COUNT, expected);
    }

    #[test]
    fn v4_symmetries_are_self_inverse_bijections_preserving_axes() {
        for sym in 0..8 {
            let mut seen = [false; CELLS];
            for idx in 0..CELLS as u8 {
                let t = transform_index(idx, sym);
                assert!(!seen[t as usize]);
                seen[t as usize] = true;
                // Every V4 reflection is its own inverse
                assert_eq!(transform_index(t, sym), idx);
            }
        }
    }

    #[test]
    fn blue_canonical_transpose_round_trips_for_both_colors() {
        for side in [Color::Red, Color::Blue] {
            for sym in 0..4 {
                for idx in 0..CELLS as u8 {
                    let c = canonical_index(idx, side, sym);
                    let unrot = transform_index(c, sym);
                    let restored = match side {
                        Color::Red => unrot,
                        Color::Blue => transpose_index(unrot),
                    };
                    assert_eq!(restored, idx);
                }
            }
        }
    }

    #[test]
    fn network_serialization_round_trips_and_validates_checksum() {
        let net = PolicyValueNetwork::seeded(42);
        let bytes = net.to_bytes();
        assert_eq!(bytes.len(), HEADER_BYTES + PARAM_COUNT * 4);
        let restored = PolicyValueNetwork::from_bytes(&bytes).unwrap();
        assert_eq!(net, restored);

        let mut corrupted = bytes;
        *corrupted.last_mut().unwrap() ^= 1;
        assert_eq!(
            PolicyValueNetwork::from_bytes(&corrupted),
            Err("policy-value network checksum mismatch")
        );
    }

    #[test]
    fn embedded_network_evaluates_legal_priors_and_bounded_value() {
        let net = PolicyValueNetwork::embedded().unwrap();
        let pos = Position::start().play("c2".parse().unwrap()).unwrap();
        assert_eq!(pos.side_to_move(), Color::Blue);
        let (priors, value) = net.evaluate(pos);
        let legal = pos.legal_moves();

        assert!((priors.iter().sum::<f64>() - 1.0).abs() < 1e-6);
        assert!((-1.0..=1.0).contains(&value));
        for idx in 0..49_u8 {
            let mv = Move::from_index(idx);
            assert_eq!(priors[idx as usize] > 0.0, legal.contains(mv));
        }
    }
}
