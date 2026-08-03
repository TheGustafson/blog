use crate::{GameKind, Player, Point, Position};

const WIN_SINGLE: usize = 0;
const WIN_GAMMON: usize = 1;
const WIN_BACKGAMMON: usize = 2;
const LOSE_SINGLE: usize = 3;
const LOSE_GAMMON: usize = 4;
const LOSE_BACKGAMMON: usize = 5;

#[derive(Clone, Copy, Debug, PartialEq)]
/// Probabilities for the six possible cubeless game outcomes.
pub struct Equity {
    outcomes: [f32; 6],
}

impl Equity {
    pub const fn win(kind: GameKind) -> Self {
        let mut outcomes = [0.0; 6];
        outcomes[match kind {
            GameKind::Single => WIN_SINGLE,
            GameKind::Gammon => WIN_GAMMON,
            GameKind::Backgammon => WIN_BACKGAMMON,
        }] = 1.0;
        Self { outcomes }
    }

    pub const fn loss(kind: GameKind) -> Self {
        let mut outcomes = [0.0; 6];
        outcomes[match kind {
            GameKind::Single => LOSE_SINGLE,
            GameKind::Gammon => LOSE_GAMMON,
            GameKind::Backgammon => LOSE_BACKGAMMON,
        }] = 1.0;
        Self { outcomes }
    }

    /// Returns `[win single, win gammon, win backgammon, lose single,
    /// lose gammon, lose backgammon]`.
    pub const fn outcomes(self) -> [f32; 6] {
        self.outcomes
    }

    pub fn win_probability(self) -> f32 {
        self.outcomes[..=WIN_BACKGAMMON].iter().sum()
    }

    pub fn expected_points(self) -> f32 {
        self.outcomes[WIN_SINGLE]
            + 2.0 * self.outcomes[WIN_GAMMON]
            + 3.0 * self.outcomes[WIN_BACKGAMMON]
            - self.outcomes[LOSE_SINGLE]
            - 2.0 * self.outcomes[LOSE_GAMMON]
            - 3.0 * self.outcomes[LOSE_BACKGAMMON]
    }

    pub const fn reversed(self) -> Self {
        Self {
            outcomes: [
                self.outcomes[LOSE_SINGLE],
                self.outcomes[LOSE_GAMMON],
                self.outcomes[LOSE_BACKGAMMON],
                self.outcomes[WIN_SINGLE],
                self.outcomes[WIN_GAMMON],
                self.outcomes[WIN_BACKGAMMON],
            ],
        }
    }

    pub(crate) const fn zero() -> Self {
        Self { outcomes: [0.0; 6] }
    }

    pub(crate) fn add_weighted(&mut self, other: Self, weight: f32) {
        for (total, value) in self.outcomes.iter_mut().zip(other.outcomes) {
            *total += value * weight;
        }
    }
}

pub fn pip_count(position: Position, player: Player) -> u16 {
    let board: u16 = (1..=24)
        .map(|number| {
            let point = Point::new(number).expect("board point is valid");
            let distance = match player {
                Player::White => number,
                Player::Black => 25 - number,
            };
            u16::from(position.count(player, point)) * u16::from(distance)
        })
        .sum();
    board + 25 * u16::from(position.bar(player))
}

/// Evaluates a position from the side-to-move's perspective.
pub fn evaluate_position(position: Position) -> Equity {
    if let Some(outcome) = position.game_outcome() {
        return if outcome.winner == position.side_to_move() {
            Equity::win(outcome.kind)
        } else {
            Equity::loss(outcome.kind)
        };
    }

    let player = position.side_to_move();
    let opponent = player.other();
    let score = relative_score(position, player);
    let win_probability = logistic(score / 42.0);
    let (win_gammon, win_backgammon) = gammon_rates(position, player);
    let (lose_gammon, lose_backgammon) = gammon_rates(position, opponent);

    let win_backgammon = win_probability * win_backgammon;
    let win_gammon = win_probability * win_gammon;
    let lose_probability = 1.0 - win_probability;
    let lose_backgammon = lose_probability * lose_backgammon;
    let lose_gammon = lose_probability * lose_gammon;
    Equity {
        outcomes: [
            win_probability - win_gammon - win_backgammon,
            win_gammon,
            win_backgammon,
            lose_probability - lose_gammon - lose_backgammon,
            lose_gammon,
            lose_backgammon,
        ],
    }
}

fn relative_score(position: Position, player: Player) -> f32 {
    let opponent = player.other();
    let pip_lead =
        f32::from(pip_count(position, opponent)) - f32::from(pip_count(position, player));
    let off_lead = f32::from(position.off(player)) - f32::from(position.off(opponent));
    let bar_lead = f32::from(position.bar(opponent)) - f32::from(position.bar(player));
    let made_lead = made_points(position, player) - made_points(position, opponent);
    let home_lead = home_points(position, player) - home_points(position, opponent);
    let prime_lead = longest_prime(position, player) - longest_prime(position, opponent);
    let blot_lead = blots(position, opponent) - blots(position, player);
    let anchor_lead = anchors(position, player) - anchors(position, opponent);

    if has_contact(position) {
        pip_lead * 0.55
            + off_lead * 9.0
            + bar_lead * 18.0
            + made_lead * 3.5
            + home_lead * 5.5
            + prime_lead * 4.5
            + blot_lead * 2.5
            + anchor_lead * 2.0
    } else {
        let wastage_lead = bearoff_wastage(position, opponent) - bearoff_wastage(position, player);
        pip_lead * 1.15 + off_lead * 8.0 + wastage_lead * 1.5
    }
}

fn made_points(position: Position, player: Player) -> f32 {
    (1..=24)
        .filter(|&number| position.count(player, Point::new(number).unwrap()) >= 2)
        .count() as f32
}

fn home_points(position: Position, player: Player) -> f32 {
    home_range(player)
        .filter(|&number| position.count(player, Point::new(number).unwrap()) >= 2)
        .count() as f32
}

fn anchors(position: Position, player: Player) -> f32 {
    home_range(player.other())
        .filter(|&number| position.count(player, Point::new(number).unwrap()) >= 2)
        .count() as f32
}

fn blots(position: Position, player: Player) -> f32 {
    (1..=24)
        .filter(|&number| position.count(player, Point::new(number).unwrap()) == 1)
        .count() as f32
}

fn longest_prime(position: Position, player: Player) -> f32 {
    let mut longest = 0;
    let mut current = 0;
    for number in 1..=24 {
        if position.count(player, Point::new(number).unwrap()) >= 2 {
            current += 1;
            longest = longest.max(current);
        } else {
            current = 0;
        }
    }
    longest as f32
}

fn bearoff_wastage(position: Position, player: Player) -> f32 {
    home_range(player)
        .map(|number| {
            let point = Point::new(number).unwrap();
            let count = position.count(player, point).saturating_sub(2);
            let distance = match player {
                Player::White => number,
                Player::Black => 25 - number,
            };
            f32::from(count) * f32::from(7 - distance)
        })
        .sum()
}

fn has_contact(position: Position) -> bool {
    if position.bar(Player::White) > 0 || position.bar(Player::Black) > 0 {
        return true;
    }
    let highest_white = (1..=24)
        .rev()
        .find(|&number| position.count(Player::White, Point::new(number).unwrap()) > 0)
        .unwrap_or(0);
    let lowest_black = (1..=24)
        .find(|&number| position.count(Player::Black, Point::new(number).unwrap()) > 0)
        .unwrap_or(25);
    highest_white >= lowest_black
}

fn gammon_rates(position: Position, player: Player) -> (f32, f32) {
    let opponent = player.other();
    if position.off(opponent) > 0 {
        return (0.0, 0.0);
    }
    let progress = f32::from(position.off(player)) / 15.0;
    let home = home_points(position, player) / 6.0;
    let trapped = f32::from(position.bar(opponent)) / 4.0
        + checkers_in_home(position, opponent, player) / 15.0;
    let total = (0.04 + progress * 0.28 + home * 0.08 + trapped * 0.08).clamp(0.0, 0.42);
    let backgammon =
        if position.bar(opponent) > 0 || checkers_in_home(position, opponent, player) > 0.0 {
            (0.01 + trapped * 0.055).min(total * 0.35)
        } else {
            0.0
        };
    (total - backgammon, backgammon)
}

fn checkers_in_home(position: Position, player: Player, home_owner: Player) -> f32 {
    home_range(home_owner)
        .map(|number| f32::from(position.count(player, Point::new(number).unwrap())))
        .sum()
}

fn home_range(player: Player) -> std::ops::RangeInclusive<u8> {
    match player {
        Player::White => 1..=6,
        Player::Black => 19..=24,
    }
}

fn logistic(value: f32) -> f32 {
    if value >= 0.0 {
        1.0 / (1.0 + (-value).exp())
    } else {
        let exponent = value.exp();
        exponent / (1.0 + exponent)
    }
}
