use ai_connect4::verification::{ORACLE_CASES, OracleOutcome, probe_oracle};
use ai_connect4::{Algorithm, Column, Move, Position, ScoreKind, SearchLimits, search};

fn position(notation: &str) -> Position {
    let moves: Vec<_> = notation
        .bytes()
        .map(|digit| Move::new(Column::new(digit - b'1')))
        .collect();
    Position::from_moves(&moves).expect("valid oracle notation")
}

#[test]
fn published_reference_cases_are_legal_and_probe_by_position() {
    for case in ORACLE_CASES {
        let position = position(case.notation);
        assert_eq!(probe_oracle(position), Some(&case));
        assert_eq!(
            case.outcome,
            match case.pons_score.cmp(&0) {
                std::cmp::Ordering::Greater => OracleOutcome::Win,
                std::cmp::Ordering::Less => OracleOutcome::Loss,
                std::cmp::Ordering::Equal => OracleOutcome::Draw,
            }
        );
    }
    assert_eq!(probe_oracle(Position::start()), None);
}

#[test]
fn live_search_agrees_on_the_two_tutorial_tactics_without_becoming_the_oracle() {
    let win = search(
        position("4455"),
        Algorithm::TranspositionTable,
        SearchLimits::depth(3),
    );
    assert_eq!(win.score.kind(), ScoreKind::ForcedWin { plies: 3 });

    let loss = search(
        position("44455554221"),
        Algorithm::TranspositionTable,
        SearchLimits::depth(2),
    );
    assert_eq!(loss.score.kind(), ScoreKind::ForcedLoss { plies: 2 });
}
