//! Deterministic, fixed-node evaluator mini-match.
//!
//! Every player uses the same move generator, iterative search, quiescence,
//! ordering, transposition-table size, and node ceiling. Only the leaf
//! evaluator changes. This is a reproducible teaching measurement, not an Elo
//! framework.

use ai_chess::{
    Color, EvaluationProfile, GameResult, Position, SearchConfig, iterative_search_with_history,
};

const NODES_PER_MOVE: u64 = 20_000;
const MAX_DEPTH: u8 = 12;
const MAX_GAME_PLIES: usize = 120;

const OPENINGS: [(&str, &[&str]); 3] = [
    ("open game", &["e2e4", "e7e5", "g1f3", "b8c6"]),
    ("queen's gambit", &["d2d4", "d7d5", "c2c4", "e7e6"]),
    ("english", &["c2c4", "e7e5", "b1c3", "g8f6"]),
];

#[derive(Clone, Copy)]
struct Player {
    profile: EvaluationProfile,
}

#[derive(Default)]
struct Aggregate {
    wins: u32,
    draws: u32,
    losses: u32,
    moves: u64,
    completed_depth: u64,
    evaluations: u64,
    nnue_accumulator_ops: u64,
}

#[derive(Clone, Copy)]
enum Outcome {
    Win(Color),
    Draw,
}

struct Game {
    outcome: Outcome,
    plies: usize,
    white: Aggregate,
    black: Aggregate,
}

fn main() {
    let players = [
        Player {
            profile: EvaluationProfile::Material,
        },
        Player {
            profile: EvaluationProfile::PieceSquare,
        },
        Player {
            profile: EvaluationProfile::TinyNnue,
        },
    ];
    let mut totals = [
        Aggregate::default(),
        Aggregate::default(),
        Aggregate::default(),
    ];
    let mut pair_wins = [[0u32; 3]; 3];
    let mut pair_draws = [0u32; 3];
    let mut pair_index = 0;
    let mut games = 0;

    println!(
        "fixed-node evaluator match: nodes/move={NODES_PER_MOVE} max-depth={MAX_DEPTH} max-game-plies={MAX_GAME_PLIES} tt-entries=65536"
    );
    println!("options: quiescence=false ordering=true tt=true book=false tablebase=false");

    for left in 0..players.len() {
        for right in left + 1..players.len() {
            for (opening, moves) in OPENINGS {
                for swap in [false, true] {
                    let (white_index, black_index) =
                        if swap { (right, left) } else { (left, right) };
                    let game = play(players[white_index], players[black_index], moves);
                    games += 1;
                    record(
                        &mut totals[white_index],
                        &game.white,
                        game.outcome,
                        Color::White,
                    );
                    record(
                        &mut totals[black_index],
                        &game.black,
                        game.outcome,
                        Color::Black,
                    );
                    match game.outcome {
                        Outcome::Win(Color::White) => {
                            pair_wins[white_index][black_index] += 1;
                        }
                        Outcome::Win(Color::Black) => {
                            pair_wins[black_index][white_index] += 1;
                        }
                        Outcome::Draw => {
                            pair_draws[pair_index] += 1;
                        }
                    }
                    println!(
                        "game={games:02} opening={opening:?} white={} black={} result={} plies={} depth={:.2}/{:.2}",
                        players[white_index].profile,
                        players[black_index].profile,
                        outcome_name(game.outcome),
                        game.plies,
                        average(game.white.completed_depth, game.white.moves),
                        average(game.black.completed_depth, game.black.moves)
                    );
                }
            }
            pair_index += 1;
        }
    }

    println!("summary games={games}");
    let expected = [
        "evaluator=material wdl=1/6/5 avg-depth=3.59 evals/move=3422.1 nnue-accumulator-ops/move=0.0",
        "evaluator=piece-square wdl=4/5/3 avg-depth=3.38 evals/move=4544.5 nnue-accumulator-ops/move=0.0",
        "evaluator=tiny-nnue wdl=6/3/3 avg-depth=3.63 evals/move=4551.1 nnue-accumulator-ops/move=7067730.2",
    ];
    for (index, (player, total)) in players.iter().zip(&totals).enumerate() {
        let summary = format!(
            "evaluator={} wdl={}/{}/{} avg-depth={:.2} evals/move={:.1} nnue-accumulator-ops/move={:.1}",
            player.profile,
            total.wins,
            total.draws,
            total.losses,
            average(total.completed_depth, total.moves),
            average(total.evaluations, total.moves),
            average(total.nnue_accumulator_ops, total.moves)
        );
        println!("{summary}");
        assert_eq!(
            summary, expected[index],
            "the published evaluator-match summary changed"
        );
    }
    assert_eq!(games, 18);
    assert_eq!(
        (pair_wins[1][0], pair_wins[0][1], pair_draws[0]),
        (2, 0, 4),
        "the published PSQT/material pairing changed"
    );
    assert_eq!(
        (pair_wins[2][0], pair_wins[0][2], pair_draws[1]),
        (3, 1, 2),
        "the published NNUE/material pairing changed"
    );
    assert_eq!(
        (pair_wins[2][1], pair_wins[1][2], pair_draws[2]),
        (3, 2, 1),
        "the published NNUE/PSQT pairing changed"
    );
}

fn play(white: Player, black: Player, opening: &[&str]) -> Game {
    let mut position = Position::start();
    let mut keys = vec![position.key()];
    for notation in opening {
        let mv = position
            .find_move(notation)
            .expect("the fixed opening suite must contain legal moves");
        position
            .make_move(mv)
            .expect("a selected opening move must be makeable");
        keys.push(position.key());
    }

    let mut white_stats = Aggregate::default();
    let mut black_stats = Aggregate::default();
    let mut plies = opening.len();
    loop {
        if keys.iter().filter(|&&key| key == position.key()).count() >= 3 {
            return Game {
                outcome: Outcome::Draw,
                plies,
                white: white_stats,
                black: black_stats,
            };
        }
        match position.result() {
            GameResult::Checkmate { winner } => {
                return Game {
                    outcome: Outcome::Win(winner),
                    plies,
                    white: white_stats,
                    black: black_stats,
                };
            }
            GameResult::Stalemate
            | GameResult::FiftyMoveDraw
            | GameResult::InsufficientMaterialDraw => {
                return Game {
                    outcome: Outcome::Draw,
                    plies,
                    white: white_stats,
                    black: black_stats,
                };
            }
            GameResult::Ongoing => {}
        }
        if plies >= MAX_GAME_PLIES {
            return Game {
                outcome: Outcome::Draw,
                plies,
                white: white_stats,
                black: black_stats,
            };
        }

        let side = position.side_to_move();
        let player = if side == Color::White { white } else { black };
        let mut config =
            SearchConfig::classical(MAX_DEPTH, player.profile).with_nodes(NODES_PER_MOVE);
        config.quiescence = false;
        let prior = &keys[..keys.len().saturating_sub(1)];
        let search = iterative_search_with_history(position.clone(), config, prior);
        assert!(
            search.completed_depth >= 1,
            "the fixed match budget must complete at least depth one"
        );
        let stats = if side == Color::White {
            &mut white_stats
        } else {
            &mut black_stats
        };
        stats.moves += 1;
        stats.completed_depth += u64::from(search.completed_depth);
        stats.evaluations += search.result.stats.evaluations;
        stats.nnue_accumulator_ops += search.result.stats.nnue_accumulator_ops;

        let Some(mv) = search.result.best_move else {
            return Game {
                outcome: Outcome::Draw,
                plies,
                white: white_stats,
                black: black_stats,
            };
        };
        position
            .make_move(mv)
            .expect("a searched best move must remain legal at its root");
        keys.push(position.key());
        plies += 1;
    }
}

fn record(total: &mut Aggregate, game: &Aggregate, outcome: Outcome, color: Color) {
    match outcome {
        Outcome::Win(winner) if winner == color => total.wins += 1,
        Outcome::Win(_) => total.losses += 1,
        Outcome::Draw => total.draws += 1,
    }
    total.moves += game.moves;
    total.completed_depth += game.completed_depth;
    total.evaluations += game.evaluations;
    total.nnue_accumulator_ops += game.nnue_accumulator_ops;
}

fn average(total: u64, count: u64) -> f64 {
    if count == 0 {
        0.0
    } else {
        total as f64 / count as f64
    }
}

fn outcome_name(outcome: Outcome) -> &'static str {
    match outcome {
        Outcome::Win(Color::White) => "1-0",
        Outcome::Win(Color::Black) => "0-1",
        Outcome::Draw => "1/2-1/2",
    }
}
