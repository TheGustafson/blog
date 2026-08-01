use crate::Player;
use crate::position::{FULL, LINES, has_line};

const STATE_COUNT: usize = 19_683;

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct MiniInfo {
    pub winner: Option<Player>,
    pub drawn: bool,
    pub empty: u16,
    pub winning_moves: [u16; 2],
    pub fork_moves: [u16; 2],
    pub potential: [i16; 2],
}

pub(crate) struct MiniTable {
    entries: Vec<MiniInfo>,
}

impl MiniTable {
    pub fn build() -> Self {
        let mut entries = Vec::with_capacity(STATE_COUNT);
        for state in 0..STATE_COUNT {
            let (x, o) = decode(state);
            entries.push(analyze(x, o));
        }
        Self { entries }
    }

    pub fn get(&self, x: u16, o: u16) -> MiniInfo {
        self.entries[encode(x, o)]
    }
}

fn encode(x: u16, o: u16) -> usize {
    let mut value = 0;
    let mut place = 1;
    for cell in 0..9 {
        let bit = 1 << cell;
        let digit = if x & bit != 0 {
            1
        } else if o & bit != 0 {
            2
        } else {
            0
        };
        value += digit * place;
        place *= 3;
    }
    value
}

fn decode(mut value: usize) -> (u16, u16) {
    let mut x = 0;
    let mut o = 0;
    for cell in 0..9 {
        match value % 3 {
            1 => x |= 1 << cell,
            2 => o |= 1 << cell,
            _ => {}
        }
        value /= 3;
    }
    (x, o)
}

fn analyze(x: u16, o: u16) -> MiniInfo {
    let empty = FULL & !(x | o);
    let x_won = has_line(x);
    let o_won = has_line(o);
    let mut info = MiniInfo {
        winner: if x_won && !o_won {
            Some(Player::X)
        } else if o_won && !x_won {
            Some(Player::O)
        } else {
            None
        },
        drawn: empty == 0 && !x_won && !o_won,
        empty,
        ..MiniInfo::default()
    };
    if x_won || o_won || info.drawn {
        return info;
    }

    for player in [Player::X, Player::O] {
        let index = player.index();
        let (ours, theirs) = match player {
            Player::X => (x, o),
            Player::O => (o, x),
        };
        let mut winning = 0;
        let mut cells = empty;
        while cells != 0 {
            let cell = cells.trailing_zeros() as u8;
            let bit = 1 << cell;
            if has_line(ours | bit) {
                winning |= bit;
            }
            cells &= cells - 1;
        }
        info.winning_moves[index] = winning;

        let mut forks = 0;
        let mut candidates = empty & !winning;
        while candidates != 0 {
            let cell = candidates.trailing_zeros() as u8;
            let bit = 1 << cell;
            let after = ours | bit;
            let replies = winning_moves(after, theirs, empty & !bit);
            if replies.count_ones() >= 2 {
                forks |= bit;
            }
            candidates &= candidates - 1;
        }
        info.fork_moves[index] = forks;
        info.potential[index] = potential(ours, theirs);
    }
    info
}

fn winning_moves(ours: u16, theirs: u16, empty: u16) -> u16 {
    let mut result = 0;
    let mut cells = empty;
    while cells != 0 {
        let cell = cells.trailing_zeros() as u8;
        let bit = 1 << cell;
        if has_line(ours | bit) && theirs & bit == 0 {
            result |= bit;
        }
        cells &= cells - 1;
    }
    result
}

fn potential(ours: u16, theirs: u16) -> i16 {
    let mut score = 0;
    for line in LINES {
        if line & theirs == 0 {
            score += match (line & ours).count_ones() {
                0 => 1,
                1 => 6,
                2 => 30,
                _ => 120,
            };
        }
    }
    score += if ours & (1 << 4) != 0 { 7 } else { 0 };
    score += (ours & 0b101_000_101).count_ones() as i16 * 3;
    score += (ours & 0b010_101_010).count_ones() as i16 * 2;
    score
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_covers_every_ternary_board() {
        let table = MiniTable::build();
        assert_eq!(table.entries.len(), STATE_COUNT);
        for state in 0..STATE_COUNT {
            let (x, o) = decode(state);
            assert_eq!(encode(x, o), state);
        }
    }

    #[test]
    fn immediate_wins_and_forks_are_precomputed() {
        let table = MiniTable::build();
        let win = table.get(0b000_000_011, 0b000_010_000);
        assert_eq!(win.winning_moves[Player::X.index()], 0b000_000_100);

        let fork = table.get(0b000_010_001, 0b100_000_000);
        assert!(fork.fork_moves[Player::X.index()] & (1 << 2) != 0);
    }

    #[test]
    fn terminal_status_is_precomputed() {
        let table = MiniTable::build();
        let won = table.get(0b000_000_111, 0b000_011_000);
        assert_eq!(won.winner, Some(Player::X));
        assert!(!won.drawn);

        let drawn = table.get(0b101_100_011, 0b010_011_100);
        assert_eq!(drawn.winner, None);
        assert!(drawn.drawn);
        assert_eq!(drawn.empty, 0);
    }
}
