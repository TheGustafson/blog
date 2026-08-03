use crate::Dice;
use crate::movegen;
use crate::play::{Location, Play, PlayError, PlayOutcome, Step, destination};
use std::fmt;

const CHECKERS: u8 = 15;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
/// A side, with White moving toward point 1 and Black toward point 24.
pub enum Player {
    White,
    Black,
}

impl Player {
    pub const fn other(self) -> Self {
        match self {
            Self::White => Self::Black,
            Self::Black => Self::White,
        }
    }

    pub const fn index(self) -> usize {
        match self {
            Self::White => 0,
            Self::Black => 1,
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::White => "white",
            Self::Black => "black",
        }
    }
}

impl fmt::Display for Player {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Point(u8);

impl Point {
    pub const fn new(number: u8) -> Option<Self> {
        if number >= 1 && number <= 24 {
            Some(Self(number))
        } else {
            None
        }
    }

    pub const fn number(self) -> u8 {
        self.0
    }

    pub(crate) const fn index(self) -> usize {
        (self.0 - 1) as usize
    }
}

impl fmt::Display for Point {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GameKind {
    Single,
    Gammon,
    Backgammon,
}

impl GameKind {
    pub const fn multiplier(self) -> u32 {
        match self {
            Self::Single => 1,
            Self::Gammon => 2,
            Self::Backgammon => 3,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GameOutcome {
    pub winner: Player,
    pub kind: GameKind,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
/// A complete checker position.
///
/// Positive point counts are White checkers and negative counts are Black.
pub struct Position {
    points: [i8; 24],
    bar: [u8; 2],
    off: [u8; 2],
    side_to_move: Player,
}

impl Default for Position {
    fn default() -> Self {
        Self::new()
    }
}

impl Position {
    pub const fn new() -> Self {
        let mut points = [0; 24];
        points[23] = 2;
        points[12] = 5;
        points[7] = 3;
        points[5] = 5;
        points[0] = -2;
        points[11] = -5;
        points[16] = -3;
        points[18] = -5;
        Self {
            points,
            bar: [0; 2],
            off: [0; 2],
            side_to_move: Player::White,
        }
    }

    /// Builds a position from signed point counts and the remaining game state.
    ///
    /// Positive point counts are White checkers, negative counts are Black,
    /// and both `bar` and `off` are ordered `[White, Black]`.
    pub fn from_parts(
        points: [i8; 24],
        bar: [u8; 2],
        off: [u8; 2],
        side_to_move: Player,
    ) -> Result<Self, PositionError> {
        let position = Self {
            points,
            bar,
            off,
            side_to_move,
        };
        position.validate()?;
        Ok(position)
    }

    pub const fn side_to_move(self) -> Player {
        self.side_to_move
    }

    /// Returns points 1 through 24 as signed checker counts.
    pub const fn points(self) -> [i8; 24] {
        self.points
    }

    pub const fn bar(self, player: Player) -> u8 {
        self.bar[player.index()]
    }

    pub const fn off(self, player: Player) -> u8 {
        self.off[player.index()]
    }

    pub fn count(self, player: Player, point: Point) -> u8 {
        let count = self.points[point.index()];
        match player {
            Player::White if count > 0 => count as u8,
            Player::Black if count < 0 => count.unsigned_abs(),
            _ => 0,
        }
    }

    /// Returns the legal next checker steps for one die.
    ///
    /// This is a partial-turn helper. [`Self::legal_plays`] enforces using the
    /// maximum number of dice and the higher-die rule across the complete turn.
    pub fn legal_steps(self, die: u8) -> Vec<Step> {
        if !(1..=6).contains(&die) || self.game_outcome().is_some() {
            return Vec::new();
        }
        let player = self.side_to_move;
        if self.bar(player) > 0 {
            let point = match player {
                Player::White => Point::new(25 - die).expect("entry die is valid"),
                Player::Black => Point::new(die).expect("entry die is valid"),
            };
            return self
                .is_open(player, point)
                .then(|| Step::unchecked(Location::Bar, Location::Point(point), die))
                .into_iter()
                .collect();
        }

        let mut steps = Vec::new();
        for number in 1..=24 {
            let point = Point::new(number).expect("board point is valid");
            if self.count(player, point) == 0 {
                continue;
            }
            let to = destination(player, point, die);
            match to {
                Location::Point(target) if self.is_open(player, target) => {
                    steps.push(Step::unchecked(Location::Point(point), to, die));
                }
                Location::Off if self.can_bear_off(player, point, die) => {
                    steps.push(Step::unchecked(Location::Point(point), Location::Off, die));
                }
                _ => {}
            }
        }
        steps.sort();
        steps
    }

    /// Generates every legal complete turn for `dice`, retaining checker-step order.
    pub fn legal_plays(self, dice: Dice) -> Vec<Play> {
        movegen::legal_plays(self, dice)
    }

    /// Groups legal step sequences that reach the same resulting position.
    pub fn legal_outcomes(self, dice: Dice) -> Vec<PlayOutcome> {
        let mut outcomes: Vec<PlayOutcome> = Vec::new();
        for play in self.legal_plays(dice) {
            let mut next = self;
            next.apply_play_unchecked(&play);
            if let Some(outcome) = outcomes
                .iter_mut()
                .find(|outcome| outcome.position() == next)
            {
                outcome.push(play);
            } else {
                outcomes.push(PlayOutcome::new(next, play));
            }
        }
        outcomes.sort_by(|left, right| left.representative().cmp(right.representative()));
        outcomes
    }

    /// Validates and applies one complete checker play.
    pub fn play(self, dice: Dice, play: &Play) -> Result<Self, PlayError> {
        let mut next = self;
        next.make_play(dice, play)?;
        Ok(next)
    }

    /// Applies a complete legal turn and returns the state needed to undo it.
    pub fn make_play(&mut self, dice: Dice, play: &Play) -> Result<Undo, PlayError> {
        if self.game_outcome().is_some() {
            return Err(PlayError::GameOver);
        }
        if !self.legal_plays(dice).iter().any(|legal| legal == play) {
            return Err(PlayError::IllegalPlay);
        }
        let undo = Undo { previous: *self };
        self.apply_play_unchecked(play);
        Ok(undo)
    }

    pub fn unmake(&mut self, undo: Undo) {
        *self = undo.previous;
    }

    pub fn game_outcome(self) -> Option<GameOutcome> {
        let winner = if self.off(Player::White) == CHECKERS {
            Player::White
        } else if self.off(Player::Black) == CHECKERS {
            Player::Black
        } else {
            return None;
        };
        let loser = winner.other();
        let kind = if self.off(loser) > 0 {
            GameKind::Single
        } else if self.bar(loser) > 0 || self.has_checker_in_home(loser, winner) {
            GameKind::Backgammon
        } else {
            GameKind::Gammon
        };
        Some(GameOutcome { winner, kind })
    }

    /// Returns a deterministic transposition key for this crate version.
    ///
    /// This is an in-memory search key, not a stable serialization format.
    pub fn hash(self) -> u64 {
        let mut hash = 0xcbf2_9ce4_8422_2325_u64;
        for value in self.points {
            hash ^= u64::from(value as u8);
            hash = hash.wrapping_mul(0x100_0000_01b3);
        }
        for value in self.bar.into_iter().chain(self.off) {
            hash ^= u64::from(value);
            hash = hash.wrapping_mul(0x100_0000_01b3);
        }
        hash ^ (self.side_to_move.index() as u64)
    }

    pub fn validate(self) -> Result<(), PositionError> {
        if self
            .points
            .iter()
            .any(|count| count.unsigned_abs() > CHECKERS)
        {
            return Err(PositionError::PointOverflow);
        }
        for player in [Player::White, Player::Black] {
            let on_board: u16 = (1..=24)
                .map(|number| {
                    self.count(player, Point::new(number).expect("board point is valid")) as u16
                })
                .sum();
            let total = on_board + u16::from(self.bar(player)) + u16::from(self.off(player));
            if total != u16::from(CHECKERS) {
                return Err(PositionError::CheckerCount {
                    player,
                    found: total,
                });
            }
        }
        if self.off(Player::White) == CHECKERS && self.off(Player::Black) == CHECKERS {
            return Err(PositionError::TwoWinners);
        }
        Ok(())
    }

    pub(crate) fn apply_step_unchecked(&mut self, step: Step) {
        let player = self.side_to_move;
        match step.from() {
            Location::Bar => self.bar[player.index()] -= 1,
            Location::Point(point) => self.remove_checker(player, point),
            Location::Off => unreachable!("borne-off checkers cannot move"),
        }
        match step.to() {
            Location::Point(point) => {
                let opponent = player.other();
                if self.count(opponent, point) == 1 {
                    self.remove_checker(opponent, point);
                    self.bar[opponent.index()] += 1;
                }
                self.add_checker(player, point);
            }
            Location::Off => self.off[player.index()] += 1,
            Location::Bar => unreachable!("checkers cannot move to the bar directly"),
        }
    }

    pub(crate) fn apply_play_unchecked(&mut self, play: &Play) {
        for &step in play.steps() {
            self.apply_step_unchecked(step);
        }
        self.side_to_move = self.side_to_move.other();
    }

    pub(crate) fn set_side_to_move(&mut self, player: Player) {
        self.side_to_move = player;
    }

    fn is_open(self, player: Player, point: Point) -> bool {
        self.count(player.other(), point) < 2
    }

    fn all_in_home(self, player: Player) -> bool {
        if self.bar(player) > 0 {
            return false;
        }
        match player {
            Player::White => (7..=24).all(|number| {
                self.count(player, Point::new(number).expect("board point is valid")) == 0
            }),
            Player::Black => (1..=18).all(|number| {
                self.count(player, Point::new(number).expect("board point is valid")) == 0
            }),
        }
    }

    fn can_bear_off(self, player: Player, point: Point, die: u8) -> bool {
        if !self.all_in_home(player) {
            return false;
        }
        let distance = match player {
            Player::White => point.number(),
            Player::Black => 25 - point.number(),
        };
        if die == distance {
            return true;
        }
        if die < distance {
            return false;
        }
        match player {
            Player::White => ((point.number() + 1)..=6).all(|number| {
                self.count(player, Point::new(number).expect("home point is valid")) == 0
            }),
            Player::Black => (19..point.number()).all(|number| {
                self.count(player, Point::new(number).expect("home point is valid")) == 0
            }),
        }
    }

    fn has_checker_in_home(self, checker: Player, owner: Player) -> bool {
        let mut range = match owner {
            Player::White => 1..=6,
            Player::Black => 19..=24,
        };
        range
            .any(|number| self.count(checker, Point::new(number).expect("home point is valid")) > 0)
    }

    fn remove_checker(&mut self, player: Player, point: Point) {
        match player {
            Player::White => self.points[point.index()] -= 1,
            Player::Black => self.points[point.index()] += 1,
        }
    }

    fn add_checker(&mut self, player: Player, point: Point) {
        match player {
            Player::White => self.points[point.index()] += 1,
            Player::Black => self.points[point.index()] -= 1,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Undo {
    previous: Position,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PositionError {
    CheckerCount { player: Player, found: u16 },
    PointOverflow,
    TwoWinners,
}

impl fmt::Display for PositionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CheckerCount { player, found } => {
                write!(
                    formatter,
                    "{player} has {found} checkers instead of fifteen"
                )
            }
            Self::PointOverflow => {
                formatter.write_str("a point contains more than fifteen checkers")
            }
            Self::TwoWinners => {
                formatter.write_str("both players cannot have borne off every checker")
            }
        }
    }
}

impl std::error::Error for PositionError {}
