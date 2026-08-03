use std::fmt;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
/// An unordered roll stored with the larger die first.
pub struct Dice {
    high: u8,
    low: u8,
}

impl Dice {
    pub fn new(first: u8, second: u8) -> Result<Self, DiceError> {
        if !(1..=6).contains(&first) || !(1..=6).contains(&second) {
            return Err(DiceError);
        }
        Ok(Self {
            high: first.max(second),
            low: first.min(second),
        })
    }

    pub const fn high(self) -> u8 {
        self.high
    }

    pub const fn low(self) -> u8 {
        self.low
    }

    pub const fn is_double(self) -> bool {
        self.high == self.low
    }

    pub fn moves(self) -> Vec<u8> {
        if self.is_double() {
            vec![self.high; 4]
        } else {
            vec![self.high, self.low]
        }
    }
}

impl fmt::Display for Dice {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}{}", self.high, self.low)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DiceError;

impl fmt::Display for DiceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("dice values must be between one and six")
    }
}

impl std::error::Error for DiceError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DiceOutcome {
    pub dice: Dice,
    pub weight: u8,
}

const fn dice(high: u8, low: u8) -> Dice {
    Dice { high, low }
}

/// The 21 unordered rolls and their multiplicities among 36 equally likely outcomes.
pub const DICE_OUTCOMES: [DiceOutcome; 21] = [
    DiceOutcome {
        dice: dice(1, 1),
        weight: 1,
    },
    DiceOutcome {
        dice: dice(2, 1),
        weight: 2,
    },
    DiceOutcome {
        dice: dice(2, 2),
        weight: 1,
    },
    DiceOutcome {
        dice: dice(3, 1),
        weight: 2,
    },
    DiceOutcome {
        dice: dice(3, 2),
        weight: 2,
    },
    DiceOutcome {
        dice: dice(3, 3),
        weight: 1,
    },
    DiceOutcome {
        dice: dice(4, 1),
        weight: 2,
    },
    DiceOutcome {
        dice: dice(4, 2),
        weight: 2,
    },
    DiceOutcome {
        dice: dice(4, 3),
        weight: 2,
    },
    DiceOutcome {
        dice: dice(4, 4),
        weight: 1,
    },
    DiceOutcome {
        dice: dice(5, 1),
        weight: 2,
    },
    DiceOutcome {
        dice: dice(5, 2),
        weight: 2,
    },
    DiceOutcome {
        dice: dice(5, 3),
        weight: 2,
    },
    DiceOutcome {
        dice: dice(5, 4),
        weight: 2,
    },
    DiceOutcome {
        dice: dice(5, 5),
        weight: 1,
    },
    DiceOutcome {
        dice: dice(6, 1),
        weight: 2,
    },
    DiceOutcome {
        dice: dice(6, 2),
        weight: 2,
    },
    DiceOutcome {
        dice: dice(6, 3),
        weight: 2,
    },
    DiceOutcome {
        dice: dice(6, 4),
        weight: 2,
    },
    DiceOutcome {
        dice: dice(6, 5),
        weight: 2,
    },
    DiceOutcome {
        dice: dice(6, 6),
        weight: 1,
    },
];
