use crate::{
    Dice, GameKind, Play, Player, Position, SearchOptions, Searcher, evaluate_position, pip_count,
};
use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AgentConfig {
    FirstLegal,
    Pip,
    Static,
    Expectimax(SearchOptions),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ArenaConfig {
    pub pairs: u32,
    pub seed: u64,
    pub max_turns: u32,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ArenaReport {
    pub games: u32,
    pub a_as_white: u32,
    pub a_as_black: u32,
    pub a_wins: u32,
    pub b_wins: u32,
    pub a_points: u32,
    pub b_points: u32,
    pub singles: u32,
    pub gammons: u32,
    pub backgammons: u32,
    pub a_searches: u64,
    pub b_searches: u64,
    pub a_nodes: u64,
    pub b_nodes: u64,
    pub a_depth: u64,
    pub b_depth: u64,
    pub a_stops: u64,
    pub b_stops: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArenaError {
    TurnLimit { pair: u32, game: u8 },
}

impl fmt::Display for ArenaError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TurnLimit { pair, game } => {
                write!(formatter, "pair {pair} game {game} exceeded the turn limit")
            }
        }
    }
}

impl std::error::Error for ArenaError {}

pub fn run_paired(
    a: AgentConfig,
    b: AgentConfig,
    config: ArenaConfig,
) -> Result<ArenaReport, ArenaError> {
    let mut report = ArenaReport::default();
    for pair in 0..config.pairs {
        let seed = config
            .seed
            .wrapping_add(u64::from(pair).wrapping_mul(0x9e37_79b9_7f4a_7c15));
        for game in 0..2_u8 {
            let a_is_white = game == 0;
            let game_report = play_game(a, b, a_is_white, seed, config.max_turns)
                .ok_or(ArenaError::TurnLimit { pair, game })?;
            let outcome = game_report.outcome;
            report.a_searches += game_report.a.searches;
            report.b_searches += game_report.b.searches;
            report.a_nodes += game_report.a.nodes;
            report.b_nodes += game_report.b.nodes;
            report.a_depth += game_report.a.depth;
            report.b_depth += game_report.b.depth;
            report.a_stops += game_report.a.stops;
            report.b_stops += game_report.b.stops;
            report.games += 1;
            if a_is_white {
                report.a_as_white += 1;
            } else {
                report.a_as_black += 1;
            }
            let a_won = (outcome.winner == Player::White) == a_is_white;
            let points = outcome.kind.multiplier();
            if a_won {
                report.a_wins += 1;
                report.a_points += points;
            } else {
                report.b_wins += 1;
                report.b_points += points;
            }
            match outcome.kind {
                GameKind::Single => report.singles += 1,
                GameKind::Gammon => report.gammons += 1,
                GameKind::Backgammon => report.backgammons += 1,
            }
        }
    }
    Ok(report)
}

fn play_game(
    a: AgentConfig,
    b: AgentConfig,
    a_is_white: bool,
    seed: u64,
    max_turns: u32,
) -> Option<GameReport> {
    let mut rng = DiceRng::new(seed);
    let mut position = Position::new();
    let (white, black) = loop {
        let roll = (rng.die(), rng.die());
        if roll.0 != roll.1 {
            break roll;
        }
    };
    position.set_side_to_move(if white > black {
        Player::White
    } else {
        Player::Black
    });
    let mut dice = Dice::new(white, black).expect("generated dice are valid");
    let mut agent_a = Agent::new(a);
    let mut agent_b = Agent::new(b);

    for _ in 0..max_turns {
        let a_to_move = (position.side_to_move() == Player::White) == a_is_white;
        let play = if a_to_move {
            agent_a.choose(position, dice)
        } else {
            agent_b.choose(position, dice)
        };
        position = position
            .play(dice, &play)
            .expect("agents choose legal plays");
        if let Some(outcome) = position.game_outcome() {
            return Some(GameReport {
                outcome,
                a: agent_a.stats,
                b: agent_b.stats,
            });
        }
        dice = Dice::new(rng.die(), rng.die()).expect("generated dice are valid");
    }
    None
}

struct Agent {
    config: AgentConfig,
    searcher: Searcher,
    stats: SearchStats,
}

impl Agent {
    fn new(config: AgentConfig) -> Self {
        Self {
            config,
            searcher: Searcher::new(),
            stats: SearchStats::default(),
        }
    }

    fn choose(&mut self, position: Position, dice: Dice) -> Play {
        match self.config {
            AgentConfig::FirstLegal => position.legal_plays(dice)[0].clone(),
            AgentConfig::Pip => best_play(position, dice, |child, player| {
                -f32::from(pip_count(child, player))
            }),
            AgentConfig::Static => best_play(position, dice, |child, _| {
                evaluate_position(child).reversed().expected_points()
            }),
            AgentConfig::Expectimax(options) => {
                let report = self.searcher.search(position, dice, options);
                self.stats.searches += 1;
                self.stats.nodes += report.nodes;
                self.stats.depth += u64::from(report.depth);
                self.stats.stops += u64::from(report.stopped);
                report
                    .best_play
                    .expect("nonterminal positions have a legal play")
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct SearchStats {
    searches: u64,
    nodes: u64,
    depth: u64,
    stops: u64,
}

struct GameReport {
    outcome: crate::GameOutcome,
    a: SearchStats,
    b: SearchStats,
}

fn best_play<F: Fn(Position, Player) -> f32>(position: Position, dice: Dice, score: F) -> Play {
    let player = position.side_to_move();
    position
        .legal_outcomes(dice)
        .into_iter()
        .max_by(|left, right| {
            score(left.position(), player)
                .total_cmp(&score(right.position(), player))
                .then_with(|| right.representative().cmp(left.representative()))
        })
        .expect("nonterminal positions have a legal outcome")
        .representative()
        .clone()
}

struct DiceRng(u64);

impl DiceRng {
    const fn new(seed: u64) -> Self {
        Self(seed)
    }

    fn die(&mut self) -> u8 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        ((self.0 >> 32) % 6 + 1) as u8
    }
}
