use crate::{Dice, Play, Position, Step};
use std::fmt;

#[derive(Clone, Debug, Eq, PartialEq)]
/// A checker turn entered one legal step at a time.
///
/// The continuation set is filtered from complete legal plays, so partial
/// input cannot bypass the maximum-dice or higher-die rules.
pub struct Turn {
    initial: Position,
    preview: Position,
    dice: Dice,
    plays: Vec<Play>,
    steps: Vec<Step>,
}

impl Turn {
    pub fn new(position: Position, dice: Dice) -> Result<Self, TurnError> {
        let plays = position.legal_plays(dice);
        if plays.is_empty() {
            return Err(TurnError::GameOver);
        }
        Ok(Self {
            initial: position,
            preview: position,
            dice,
            plays,
            steps: Vec::new(),
        })
    }

    pub const fn initial_position(&self) -> Position {
        self.initial
    }

    pub const fn preview_position(&self) -> Position {
        self.preview
    }

    pub const fn dice(&self) -> Dice {
        self.dice
    }

    pub fn steps(&self) -> &[Step] {
        &self.steps
    }

    pub fn remaining_dice(&self) -> Vec<u8> {
        let mut remaining = self.dice.moves();
        for step in &self.steps {
            if let Some(index) = remaining.iter().position(|die| *die == step.die()) {
                remaining.remove(index);
            }
        }
        remaining
    }

    pub fn legal_steps(&self) -> Vec<Step> {
        let mut next = self
            .plays
            .iter()
            .filter(|play| play.steps().starts_with(&self.steps))
            .filter_map(|play| play.steps().get(self.steps.len()).copied())
            .collect::<Vec<_>>();
        next.sort();
        next.dedup();
        next
    }

    pub fn select(&mut self, step: Step) -> Result<(), TurnError> {
        if self.is_complete() || !self.legal_steps().contains(&step) {
            return Err(TurnError::IllegalStep);
        }
        self.preview.apply_step_unchecked(step);
        self.steps.push(step);
        Ok(())
    }

    pub fn undo(&mut self) -> bool {
        if self.steps.pop().is_none() {
            return false;
        }
        self.preview = self.initial;
        for &step in &self.steps {
            self.preview.apply_step_unchecked(step);
        }
        true
    }

    pub fn is_pass(&self) -> bool {
        self.steps.is_empty() && self.plays.iter().any(Play::is_empty)
    }

    pub fn is_complete(&self) -> bool {
        self.plays.iter().any(|play| play.steps() == self.steps)
    }

    pub fn completed_play(&self) -> Option<&Play> {
        self.plays.iter().find(|play| play.steps() == self.steps)
    }

    pub fn finish(self) -> Result<Play, TurnError> {
        self.plays
            .into_iter()
            .find(|play| play.steps() == self.steps)
            .ok_or(TurnError::Incomplete)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TurnError {
    GameOver,
    IllegalStep,
    Incomplete,
}

impl fmt::Display for TurnError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::GameOver => formatter.write_str("the game is already over"),
            Self::IllegalStep => {
                formatter.write_str("that checker step is not a legal continuation")
            }
            Self::Incomplete => formatter.write_str("the checker turn is not complete"),
        }
    }
}

impl std::error::Error for TurnError {}
