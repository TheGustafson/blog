use crate::{Player, Point, Position};
use std::fmt;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Location {
    Bar,
    Point(Point),
    Off,
}

impl fmt::Display for Location {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bar => formatter.write_str("bar"),
            Self::Point(point) => point.fmt(formatter),
            Self::Off => formatter.write_str("off"),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
/// One checker movement using one die.
pub struct Step {
    from: Location,
    to: Location,
    die: u8,
}

impl Step {
    /// Creates a structurally valid checker step.
    ///
    /// This does not test the step against a position. Use [`crate::Turn`] or
    /// [`crate::Position::play`] when game-rule validation is required.
    pub fn new(from: Location, to: Location, die: u8) -> Result<Self, StepError> {
        if !(1..=6).contains(&die) || from == Location::Off || to == Location::Bar {
            return Err(StepError);
        }
        Ok(Self { from, to, die })
    }

    pub const fn from(self) -> Location {
        self.from
    }

    pub const fn to(self) -> Location {
        self.to
    }

    pub const fn die(self) -> u8 {
        self.die
    }

    pub(crate) const fn unchecked(from: Location, to: Location, die: u8) -> Self {
        Self { from, to, die }
    }
}

impl fmt::Display for Step {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}/{}", self.from, self.to)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StepError;

impl fmt::Display for StepError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a checker step needs a valid die, source, and destination")
    }
}

impl std::error::Error for StepError {}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
/// A complete turn, preserving the order in which its checker steps occur.
pub struct Play {
    steps: Vec<Step>,
}

impl Play {
    /// Creates an ordered checker play without validating it against a position.
    pub fn new(steps: Vec<Step>) -> Self {
        Self { steps }
    }

    pub const fn pass() -> Self {
        Self { steps: Vec::new() }
    }

    pub fn steps(&self) -> &[Step] {
        &self.steps
    }

    pub fn len(&self) -> usize {
        self.steps.len()
    }

    pub fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }
}

impl fmt::Display for Play {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.steps.is_empty() {
            return formatter.write_str("pass");
        }
        for (index, step) in self.steps.iter().enumerate() {
            if index > 0 {
                formatter.write_str(" ")?;
            }
            step.fmt(formatter)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
/// One resulting position and every legal step sequence that reaches it.
pub struct PlayOutcome {
    position: Position,
    sequences: Vec<Play>,
}

impl PlayOutcome {
    pub(crate) fn new(position: Position, sequence: Play) -> Self {
        Self {
            position,
            sequences: vec![sequence],
        }
    }

    pub const fn position(&self) -> Position {
        self.position
    }

    pub fn representative(&self) -> &Play {
        &self.sequences[0]
    }

    pub fn sequences(&self) -> &[Play] {
        &self.sequences
    }

    pub(crate) fn push(&mut self, sequence: Play) {
        self.sequences.push(sequence);
        self.sequences.sort();
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlayError {
    GameOver,
    IllegalPlay,
}

impl fmt::Display for PlayError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::GameOver => formatter.write_str("the game is already over"),
            Self::IllegalPlay => formatter.write_str("the play is not legal for these dice"),
        }
    }
}

impl std::error::Error for PlayError {}

pub(crate) fn destination(player: Player, point: Point, die: u8) -> Location {
    match player {
        Player::White if point.number() <= die => Location::Off,
        Player::White => Location::Point(Point::new(point.number() - die).expect("point is valid")),
        Player::Black if point.number() + die > 24 => Location::Off,
        Player::Black => Location::Point(Point::new(point.number() + die).expect("point is valid")),
    }
}
