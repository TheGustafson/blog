use ai_othello::{EvaluationProfile, Move, Position, ScoreKind, SearchConfig, evaluate, search};

fn curated_position() -> Position {
    let moves: Vec<Move> = ["d3", "c3", "b3", "b2", "b1", "a1", "c4", "c1", "c2"]
        .into_iter()
        .map(|mv| mv.parse().unwrap())
        .collect();
    Position::from_moves(&moves).unwrap()
}

#[test]
fn greedy_material_and_future_control_disagree_on_a_locked_position() {
    let position = curated_position();
    let material = search(
        position,
        SearchConfig::fixed_depth(4, EvaluationProfile::Material),
    );
    let mobility = search(
        position,
        SearchConfig::fixed_depth(4, EvaluationProfile::Mobility),
    );
    let corners = search(
        position,
        SearchConfig::fixed_depth(4, EvaluationProfile::Corners),
    );
    let frontier = search(
        position,
        SearchConfig::fixed_depth(4, EvaluationProfile::Frontier),
    );
    let phase = search(
        position,
        SearchConfig::fixed_depth(4, EvaluationProfile::Phase),
    );

    assert_eq!(material.best_move.unwrap().to_string(), "c5");
    assert_eq!(mobility.best_move.unwrap().to_string(), "d2");
    assert_eq!(corners.best_move.unwrap().to_string(), "c5");
    assert_eq!(frontier.best_move.unwrap().to_string(), "d2");
    assert_eq!(phase.best_move.unwrap().to_string(), "d2");
    assert_eq!(
        [
            material.stats.nodes,
            mobility.stats.nodes,
            corners.stats.nodes,
            frontier.stats.nodes,
            phase.stats.nodes,
        ],
        [724, 487, 582, 528, 529]
    );
    assert_eq!(
        [
            material.score.kind(),
            mobility.score.kind(),
            corners.score.kind(),
            frontier.score.kind(),
            phase.score.kind(),
        ],
        [
            ScoreKind::Estimate(10),
            ScoreKind::Estimate(24),
            ScoreKind::Estimate(106),
            ScoreKind::Estimate(134),
            ScoreKind::Estimate(139),
        ]
    );
}

#[test]
fn the_greedy_move_flips_four_while_the_control_move_flips_one() {
    let position = curated_position();
    let greedy: ai_othello::Square = "c5".parse().unwrap();
    let control: ai_othello::Square = "d2".parse().unwrap();
    assert_eq!(position.flips_for(greedy).count_ones(), 4);
    assert_eq!(position.flips_for(control).count_ones(), 1);

    let mut greedy_child = position;
    greedy_child.make_move(Move::Place(greedy)).unwrap();
    let mut control_child = position;
    control_child.make_move(Move::Place(control)).unwrap();
    assert!(
        -evaluate(greedy_child, EvaluationProfile::Material).total
            > -evaluate(control_child, EvaluationProfile::Material).total
    );
    assert!(
        -evaluate(control_child, EvaluationProfile::Phase).total
            > -evaluate(greedy_child, EvaluationProfile::Phase).total
    );
}
