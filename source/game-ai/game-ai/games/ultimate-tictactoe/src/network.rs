use crate::{GameResult, MiniResult, Move, Position};

pub(crate) const INPUTS: usize = 200;
pub(crate) const HIDDEN: usize = 32;
pub(crate) const POLICY_OUTPUTS: usize = 81;
pub(crate) const PARAM_COUNT: usize =
    INPUTS * HIDDEN + HIDDEN + POLICY_OUTPUTS * HIDDEN + POLICY_OUTPUTS;

const MAGIC: &[u8; 8] = b"UTTTPOLY";
const VERSION: u32 = 1;
const HEADER_BYTES: usize = 32;
pub(crate) const W1: usize = 0;
pub(crate) const B1: usize = W1 + INPUTS * HIDDEN;
pub(crate) const W_POLICY: usize = B1 + HIDDEN;
pub(crate) const B_POLICY: usize = W_POLICY + POLICY_OUTPUTS * HIDDEN;

const EMBEDDED_POLICY: &[u8] = include_bytes!("networks/mcts-policy.bin");

#[derive(Clone)]
pub(crate) struct PolicyNetwork {
    pub(crate) parameters: Box<[f32]>,
}

impl PolicyNetwork {
    pub(crate) fn embedded() -> Result<Self, &'static str> {
        Self::from_bytes(EMBEDDED_POLICY)
    }

    pub(crate) fn from_bytes(bytes: &[u8]) -> Result<Self, &'static str> {
        if bytes.len() != HEADER_BYTES + PARAM_COUNT * size_of::<f32>() {
            return Err("wrong policy network size");
        }
        if &bytes[..8] != MAGIC {
            return Err("wrong policy network magic");
        }
        if read_u32(bytes, 8) != VERSION {
            return Err("unsupported policy network version");
        }
        if usize::from(read_u16(bytes, 12)) != INPUTS
            || usize::from(read_u16(bytes, 14)) != HIDDEN
            || usize::from(read_u16(bytes, 16)) != POLICY_OUTPUTS
            || read_u16(bytes, 18) != 0
            || read_u32(bytes, 20) as usize != PARAM_COUNT
        {
            return Err("wrong policy network shape");
        }
        let payload = &bytes[HEADER_BYTES..];
        if read_u64(bytes, 24) != checksum(payload) {
            return Err("policy network checksum mismatch");
        }
        let parameters = payload
            .chunks_exact(4)
            .map(|chunk| f32::from_le_bytes(chunk.try_into().expect("chunks are four bytes")))
            .collect::<Vec<_>>()
            .into_boxed_slice();
        if parameters.iter().any(|value| !value.is_finite()) {
            return Err("policy network contains a non-finite weight");
        }
        Ok(Self { parameters })
    }

    #[cfg(any(feature = "training", test))]
    pub(crate) fn to_bytes(&self) -> Vec<u8> {
        let mut payload = Vec::with_capacity(PARAM_COUNT * size_of::<f32>());
        for parameter in &self.parameters {
            payload.extend_from_slice(&parameter.to_le_bytes());
        }
        let mut bytes = Vec::with_capacity(HEADER_BYTES + payload.len());
        bytes.extend_from_slice(MAGIC);
        bytes.extend_from_slice(&VERSION.to_le_bytes());
        bytes.extend_from_slice(&(INPUTS as u16).to_le_bytes());
        bytes.extend_from_slice(&(HIDDEN as u16).to_le_bytes());
        bytes.extend_from_slice(&(POLICY_OUTPUTS as u16).to_le_bytes());
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes.extend_from_slice(&(PARAM_COUNT as u32).to_le_bytes());
        bytes.extend_from_slice(&checksum(&payload).to_le_bytes());
        bytes.extend_from_slice(&payload);
        bytes
    }

    #[cfg(test)]
    pub(crate) fn random(seed: u64) -> Self {
        let mut random = SplitMix64(seed);
        let mut parameters = vec![0.0; PARAM_COUNT].into_boxed_slice();
        let hidden_scale = (6.0 / (INPUTS + HIDDEN) as f32).sqrt();
        for parameter in &mut parameters[W1..B1] {
            *parameter = random.signed() * hidden_scale;
        }
        let output_scale = (6.0 / (HIDDEN + POLICY_OUTPUTS) as f32).sqrt();
        for parameter in &mut parameters[W_POLICY..B_POLICY] {
            *parameter = random.signed() * output_scale;
        }
        Self { parameters }
    }

    pub(crate) fn predict(&self, position: Position) -> [f64; POLICY_OUTPUTS] {
        let input = encode(position, 0);
        let legal = position.legal_moves();
        let mut legal_mask = [false; POLICY_OUTPUTS];
        for mv in legal.iter() {
            legal_mask[mv.global_index() as usize] = true;
        }
        self.predict_encoded(&input, &legal_mask)
    }

    pub(crate) fn predict_encoded(
        &self,
        input: &[f32; INPUTS],
        legal: &[bool; POLICY_OUTPUTS],
    ) -> [f64; POLICY_OUTPUTS] {
        let hidden = self.hidden(input);
        let mut logits = [f32::NEG_INFINITY; POLICY_OUTPUTS];
        let mut maximum = f32::NEG_INFINITY;
        for action in 0..POLICY_OUTPUTS {
            if !legal[action] {
                continue;
            }
            let mut value = self.parameters[B_POLICY + action];
            let offset = W_POLICY + action * HIDDEN;
            for (hidden_index, hidden_value) in hidden.iter().enumerate() {
                value += self.parameters[offset + hidden_index] * hidden_value;
            }
            logits[action] = value;
            maximum = maximum.max(value);
        }
        let mut total = 0.0_f32;
        let mut priors = [0.0; POLICY_OUTPUTS];
        for action in 0..POLICY_OUTPUTS {
            if legal[action] {
                priors[action] = (logits[action] - maximum).exp();
                total += priors[action];
            }
        }
        if total > 0.0 {
            for prior in &mut priors {
                *prior /= total;
            }
        }

        priors.map(f64::from)
    }

    pub(crate) fn hidden(&self, input: &[f32; INPUTS]) -> [f32; HIDDEN] {
        let mut hidden = [0.0; HIDDEN];
        for (hidden_index, hidden_value) in hidden.iter_mut().enumerate() {
            let mut value = self.parameters[B1 + hidden_index];
            let offset = W1 + hidden_index * INPUTS;
            for (input_index, input_value) in input.iter().enumerate() {
                value += self.parameters[offset + input_index] * input_value;
            }
            *hidden_value = value.max(0.0);
        }
        hidden
    }
}

pub(crate) fn encode(position: Position, symmetry: u8) -> [f32; INPUTS] {
    let mut input = [0.0; INPUTS];
    let side = position.side_to_move();
    for index in 0..81_u8 {
        let transformed = transform_index(index, 9, symmetry) as usize;
        match position.player_at(Move::from_global_index(index)) {
            Some(player) if player == side => input[transformed] = 1.0,
            Some(_) => input[81 + transformed] = 1.0,
            None => {}
        }
    }
    for board in 0..9_u8 {
        let transformed = transform_index(board, 3, symmetry) as usize;
        match position.mini_result(board) {
            MiniResult::Win(player) if player == side => input[162 + transformed] = 1.0,
            MiniResult::Win(_) => input[171 + transformed] = 1.0,
            MiniResult::Draw => input[180 + transformed] = 1.0,
            MiniResult::Open => {}
        }
    }
    if let Some(board) = position.active_board() {
        input[189 + transform_index(board, 3, symmetry) as usize] = 1.0;
    } else if position.result() == GameResult::Ongoing {
        input[198] = 1.0;
    }
    input[199] = f32::from(position.ply()) / 81.0;
    input
}

pub(crate) fn transform_index(index: u8, width: u8, symmetry: u8) -> u8 {
    let row = index / width;
    let column = index % width;
    let last = width - 1;
    match symmetry % 8 {
        0 => row * width + column,
        1 => column * width + (last - row),
        2 => (last - row) * width + (last - column),
        3 => (last - column) * width + row,
        4 => row * width + (last - column),
        5 => (last - column) * width + (last - row),
        6 => (last - row) * width + column,
        _ => column * width + row,
    }
}

fn checksum(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
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

#[cfg(test)]
struct SplitMix64(u64);

#[cfg(test)]
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
    fn every_symmetry_is_a_permutation() {
        for width in [3, 9] {
            for symmetry in 0..8 {
                let mut seen = vec![false; usize::from(width * width)];
                for index in 0..width * width {
                    let transformed = transform_index(index, width, symmetry) as usize;
                    assert!(!seen[transformed]);
                    seen[transformed] = true;
                }
            }
        }
    }

    #[test]
    fn action_symmetries_preserve_the_nested_board_geometry() {
        for symmetry in 0..8 {
            for board in 0..9 {
                for cell in 0..9 {
                    let mv = Move::new(board, cell);
                    let transformed =
                        Move::from_global_index(transform_index(mv.global_index(), 9, symmetry));
                    assert_eq!(transformed.board(), transform_index(board, 3, symmetry));
                    assert_eq!(transformed.cell(), transform_index(cell, 3, symmetry));
                }
            }
        }
    }

    #[test]
    fn serialized_network_round_trips_and_checks_integrity() {
        let network = PolicyNetwork::random(41);
        let bytes = network.to_bytes();
        let restored = PolicyNetwork::from_bytes(&bytes).unwrap();
        assert_eq!(network.parameters, restored.parameters);

        let mut corrupt = bytes;
        *corrupt.last_mut().unwrap() ^= 1;
        assert_eq!(
            PolicyNetwork::from_bytes(&corrupt).err(),
            Some("policy network checksum mismatch")
        );
    }

    #[test]
    fn prediction_masks_illegal_moves_and_normalizes_priors() {
        let network = PolicyNetwork::random(7);
        let position = Position::start().play(Move::new(4, 0)).unwrap();
        let output = network.predict(position);
        let legal = position.legal_moves();

        assert!((output.iter().sum::<f64>() - 1.0).abs() < 1e-6);
        for index in 0..81 {
            let mv = Move::from_global_index(index);
            assert_eq!(output[index as usize] > 0.0, legal.contains(mv));
        }
    }

    #[test]
    fn embedded_network_matches_the_published_policy() {
        assert_eq!(read_u64(EMBEDDED_POLICY, 24), 0x0b62_9b24_2ff3_8bfb);
        let network = PolicyNetwork::embedded().unwrap();
        assert_eq!(network.parameters.len(), PARAM_COUNT);
        let priors = network.predict(Position::start());
        assert!((priors.iter().sum::<f64>() - 1.0).abs() < 1e-6);
        let mut ranked = priors.iter().copied().enumerate().collect::<Vec<_>>();
        ranked.sort_by(|left, right| right.1.total_cmp(&left.1));
        assert_eq!(
            ranked
                .iter()
                .take(8)
                .map(|(index, _)| *index)
                .collect::<Vec<_>>(),
            [40, 32, 48, 50, 16, 30, 10, 20],
        );
    }
}
