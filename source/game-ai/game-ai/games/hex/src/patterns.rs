use crate::connectivity::neighbors;
use crate::{Cell, Color, Position};

pub(crate) fn bridge_responses(
    position: Position,
    color: Color,
    attacked: Cell,
) -> ([Cell; 6], usize) {
    let mut endpoints = [attacked; 6];
    let mut endpoint_count = 0;
    for cell in neighbors(attacked, position.size()).into_iter().flatten() {
        if position.color_at(cell) == Some(color) {
            endpoints[endpoint_count] = cell;
            endpoint_count += 1;
        }
    }
    let mut responses = [attacked; 6];
    let mut count = 0;

    for left in 0..endpoint_count {
        for right in left + 1..endpoint_count {
            if are_neighbors(endpoints[left], endpoints[right], position) {
                continue;
            }
            for candidate in neighbors(endpoints[left], position.size())
                .into_iter()
                .flatten()
            {
                if candidate == attacked
                    || position.color_at(candidate).is_some()
                    || !are_neighbors(candidate, endpoints[right], position)
                    || responses[..count].contains(&candidate)
                {
                    continue;
                }
                debug_assert!(count < responses.len());
                responses[count] = candidate;
                count += 1;
            }
        }
    }
    (responses, count)
}

fn are_neighbors(left: Cell, right: Cell, position: Position) -> bool {
    neighbors(left, position.size()).contains(&Some(right))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BoardSize, Move, SwapRule};

    fn cell(name: &str) -> Cell {
        name.parse::<Move>().unwrap().cell().unwrap()
    }

    #[test]
    fn finds_the_other_carrier_of_an_interior_bridge() {
        let moves = ["d4", "a1", "e5", "e4"].map(|mv| mv.parse().expect("valid move"));
        let position =
            Position::from_moves(BoardSize::new(9).unwrap(), SwapRule::Disabled, &moves).unwrap();
        let attacked = cell("e4");

        let (responses, count) = bridge_responses(position, Color::Red, attacked);

        assert_eq!(&responses[..count], &[cell("d5")]);
    }
}
