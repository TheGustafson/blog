use crate::{Dice, Play, Position};

pub fn legal_plays(position: Position, dice: Dice) -> Vec<Play> {
    if position.game_outcome().is_some() {
        return Vec::new();
    }
    let remaining = dice.moves();
    let mut plays = Vec::new();
    let mut steps = Vec::new();
    generate(position, &remaining, &mut steps, &mut plays);
    normalize(plays, dice)
}

fn generate(
    position: Position,
    remaining: &[u8],
    steps: &mut Vec<crate::Step>,
    plays: &mut Vec<Play>,
) {
    if remaining.is_empty() || position.game_outcome().is_some() {
        plays.push(Play::new(steps.clone()));
        return;
    }

    let mut moved = false;
    let mut previous_die = None;
    for (index, &die) in remaining.iter().enumerate() {
        if previous_die == Some(die) {
            continue;
        }
        previous_die = Some(die);
        for step in position.legal_steps(die) {
            moved = true;
            let mut next = position;
            next.apply_step_unchecked(step);
            let mut rest = remaining.to_vec();
            rest.remove(index);
            steps.push(step);
            generate(next, &rest, steps, plays);
            steps.pop();
        }
    }
    if !moved {
        plays.push(Play::new(steps.clone()));
    }
}

fn normalize(mut plays: Vec<Play>, dice: Dice) -> Vec<Play> {
    let used = plays.iter().map(Play::len).max().unwrap_or(0);
    plays.retain(|play| play.len() == used);
    if !dice.is_double() && used == 1 {
        let high_is_legal = plays
            .iter()
            .any(|play| play.steps()[0].die() == dice.high());
        if high_is_legal {
            plays.retain(|play| play.steps()[0].die() == dice.high());
        }
    }
    plays.sort();
    plays.dedup();
    if plays.is_empty() {
        vec![Play::pass()]
    } else {
        plays
    }
}
