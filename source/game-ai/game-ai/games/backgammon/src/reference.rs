use crate::{Dice, Play, Position, Step};

pub fn legal_plays(position: Position, dice: Dice) -> Vec<Play> {
    if position.game_outcome().is_some() {
        return Vec::new();
    }
    let orders = if dice.is_double() {
        vec![vec![dice.high(); 4]]
    } else {
        vec![vec![dice.high(), dice.low()], vec![dice.low(), dice.high()]]
    };
    let mut plays = Vec::new();
    for order in orders {
        walk_order(position, &order, 0, &mut Vec::new(), &mut plays);
    }
    let max_steps = plays.iter().map(Play::len).max().unwrap_or(0);
    let mut filtered: Vec<Play> = plays
        .into_iter()
        .filter(|play| play.len() == max_steps)
        .collect();
    if !dice.is_double() && max_steps == 1 {
        let high_is_legal = filtered
            .iter()
            .any(|play| play.steps()[0].die() == dice.high());
        if high_is_legal {
            filtered.retain(|play| play.steps()[0].die() == dice.high());
        }
    }
    filtered.sort();
    filtered.dedup();
    if filtered.is_empty() {
        vec![Play::pass()]
    } else {
        filtered
    }
}

fn walk_order(
    position: Position,
    order: &[u8],
    index: usize,
    steps: &mut Vec<Step>,
    plays: &mut Vec<Play>,
) {
    if index == order.len() || position.game_outcome().is_some() {
        plays.push(Play::new(steps.clone()));
        return;
    }
    let candidates = position.legal_steps(order[index]);
    if candidates.is_empty() {
        plays.push(Play::new(steps.clone()));
        return;
    }
    for step in candidates {
        let mut next = position;
        next.apply_step_unchecked(step);
        steps.push(step);
        walk_order(next, order, index + 1, steps, plays);
        steps.pop();
    }
}
