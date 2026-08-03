use crate::{Dice, DiceError, GameOutcome, Play, PlayError, Player, Position};
use std::fmt;

/// The action currently expected by a single game.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GamePhase {
    OpeningRoll,
    PreRoll,
    CheckerPlay(Dice),
    GameOver(GameOutcome),
}

/// A cubeless game from the opening roll through its final checker play.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Game {
    position: Position,
    phase: GamePhase,
}

impl Default for Game {
    fn default() -> Self {
        Self::new()
    }
}

impl Game {
    pub const fn new() -> Self {
        Self {
            position: Position::new(),
            phase: GamePhase::OpeningRoll,
        }
    }

    pub const fn position(&self) -> Position {
        self.position
    }

    pub const fn phase(&self) -> GamePhase {
        self.phase
    }

    pub fn restart(&mut self) {
        *self = Self::new();
    }

    /// Applies the opening roll, returning `false` when the dice tie and must be rerolled.
    pub fn opening_roll(&mut self, white: u8, black: u8) -> Result<bool, GameError> {
        if self.phase != GamePhase::OpeningRoll {
            return Err(GameError::WrongPhase);
        }
        let dice = Dice::new(white, black)?;
        if white == black {
            return Ok(false);
        }
        self.position.set_side_to_move(if white > black {
            Player::White
        } else {
            Player::Black
        });
        self.phase = GamePhase::CheckerPlay(dice);
        Ok(true)
    }

    /// Starts the next checker turn with dice supplied by the caller.
    pub fn roll(&mut self, dice: Dice) -> Result<(), GameError> {
        if self.phase != GamePhase::PreRoll {
            return Err(GameError::WrongPhase);
        }
        self.phase = GamePhase::CheckerPlay(dice);
        Ok(())
    }

    pub fn legal_plays(&self) -> Vec<Play> {
        match self.phase {
            GamePhase::CheckerPlay(dice) => self.position.legal_plays(dice),
            _ => Vec::new(),
        }
    }

    /// Applies the current turn and advances to the next roll or game over.
    pub fn play(&mut self, play: &Play) -> Result<(), GameError> {
        let GamePhase::CheckerPlay(dice) = self.phase else {
            return Err(GameError::WrongPhase);
        };
        self.position = self.position.play(dice, play)?;
        self.phase = self
            .position
            .game_outcome()
            .map_or(GamePhase::PreRoll, GamePhase::GameOver);
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GameError {
    WrongPhase,
    InvalidDice(DiceError),
    InvalidPlay(PlayError),
}

impl From<DiceError> for GameError {
    fn from(error: DiceError) -> Self {
        Self::InvalidDice(error)
    }
}

impl From<PlayError> for GameError {
    fn from(error: PlayError) -> Self {
        Self::InvalidPlay(error)
    }
}

impl fmt::Display for GameError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongPhase => formatter.write_str("that action is not available in this phase"),
            Self::InvalidDice(error) => error.fmt(formatter),
            Self::InvalidPlay(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for GameError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::GameKind;

    #[test]
    fn a_natural_gammon_ends_the_game() {
        let mut points = [0_i8; 24];
        points[0] = 1;
        points[18] = -15;
        let position = Position::from_parts(points, [0, 0], [14, 0], Player::White).unwrap();
        let mut game = Game {
            position,
            phase: GamePhase::CheckerPlay(Dice::new(6, 5).unwrap()),
        };

        let play = game.legal_plays().remove(0);
        game.play(&play).unwrap();

        let GamePhase::GameOver(outcome) = game.phase() else {
            panic!("the game should be over");
        };
        assert_eq!(outcome.winner, Player::White);
        assert_eq!(outcome.kind, GameKind::Gammon);
    }
}
