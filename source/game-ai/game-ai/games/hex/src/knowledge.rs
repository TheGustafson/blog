use crate::board::{Cell, MAX_CELLS};
use crate::cell_set::CellSet;
use crate::connectivity::neighbors;
use crate::virtual_connection;
use crate::{Color, Move, Position, Seat};

const GRAPH_WORDS: usize = MAX_CELLS / 64 + 1;
const FIRST_EDGE: usize = MAX_CELLS;
const SECOND_EDGE: usize = MAX_CELLS + 1;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct Knowledge {
    pub(crate) pruned: CellSet,
    pub(crate) must_play: Option<CellSet>,
    pub(crate) proven_winner: Option<Seat>,
    pub(crate) virtual_connections: u32,
    pub(crate) semi_connections: u32,
    pub(crate) connection_search_truncated: bool,
}

impl Knowledge {
    pub(crate) fn allows(self, mv: Move) -> bool {
        let Move::Place(cell) = mv else {
            return true;
        };
        !self.pruned.contains(cell)
            && self
                .must_play
                .is_none_or(|must_play| must_play.contains(cell))
    }
}

pub(crate) fn analyze(position: Position, find_virtual_connections: bool) -> Knowledge {
    if position.swap_available() {
        return Knowledge::default();
    }

    let mut candidates = CellSet::default();
    for cell in position.empty_cells() {
        candidates.insert(cell);
    }

    let mover = position.color_to_move();
    let opponent = mover.other();
    let winning = immediate_wins(position, mover);
    let opponent_wins = immediate_wins(position, opponent);
    let own_connections = if find_virtual_connections {
        virtual_connection::analyze(position, mover)
    } else {
        Default::default()
    };
    let opponent_connections = if find_virtual_connections {
        virtual_connection::analyze(position, opponent)
    } else {
        Default::default()
    };

    let own_connection_win =
        own_connections.winning_vc.is_some() || own_connections.winning_sc_count > 0;
    let opponent_connection_win = opponent_connections.winning_vc.is_some()
        || opponent_connections
            .winning_sc_carrier_intersection()
            .is_some_and(CellSet::is_empty);
    let mut proven_winner = if !winning.is_empty() || own_connection_win {
        Some(position.seat_to_move())
    } else if opponent_wins.count() > 1 || opponent_connection_win {
        Some(position.seat_to_move().other())
    } else {
        None
    };

    let mut must_play = if !winning.is_empty() {
        Some(winning)
    } else if own_connections.winning_vc.is_some() {
        None
    } else if own_connections.winning_sc_count > 0 {
        Some(own_connections.winning_keys())
    } else if opponent_wins.count() == 1 {
        Some(opponent_wins)
    } else {
        opponent_connections.winning_sc_carrier_intersection()
    };
    if winning.is_empty() && !own_connection_win && opponent_wins.count() == 1 {
        if let Some(connection_defenses) = opponent_connections.winning_sc_carrier_intersection() {
            must_play = Some(opponent_wins.intersection(connection_defenses));
        }
    }
    if must_play.is_some_and(CellSet::is_empty) {
        proven_winner.get_or_insert(position.seat_to_move().other());
        must_play = None;
    }
    let domination_candidates = must_play.unwrap_or(candidates);
    let pruned = neighborhood_dominated(position, domination_candidates);

    Knowledge {
        pruned,
        must_play,
        proven_winner,
        virtual_connections: own_connections.virtual_connections
            + opponent_connections.virtual_connections,
        semi_connections: own_connections.semi_connections + opponent_connections.semi_connections,
        connection_search_truncated: own_connections.truncated || opponent_connections.truncated,
    }
}

fn immediate_wins(position: Position, color: Color) -> CellSet {
    let mut winning = CellSet::default();
    if position.stone_count(color) + 1 < u32::from(position.size().get()) {
        return winning;
    }
    for cell in position.empty_cells() {
        if position.connects_after(color, cell) {
            winning.insert(cell);
        }
    }
    winning
}

fn neighborhood_dominated(position: Position, candidates: CellSet) -> CellSet {
    let color = position.color_to_move();
    let mut pruned = CellSet::default();
    for candidate in position.empty_cells() {
        if !candidates.contains(candidate) {
            continue;
        }
        let candidate_neighborhood = player_neighborhood(position, color, candidate);
        for dominator in neighbors(candidate, position.size()).into_iter().flatten() {
            if !candidates.contains(dominator) || position.color_at(dominator).is_some() {
                continue;
            }
            let dominator_neighborhood = player_neighborhood(position, color, dominator);
            if candidate_neighborhood.is_subset_of(dominator_neighborhood)
                && (candidate_neighborhood != dominator_neighborhood
                    || dominator.index() < candidate.index())
            {
                pruned.insert(candidate);
                break;
            }
        }
    }
    pruned
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct GraphSet([u64; GRAPH_WORDS]);

impl GraphSet {
    const fn empty() -> Self {
        Self([0; GRAPH_WORDS])
    }

    fn insert(&mut self, index: usize) {
        self.0[index / 64] |= 1_u64 << (index % 64);
    }

    fn is_subset_of(self, other: Self) -> bool {
        self.0
            .iter()
            .zip(other.0)
            .all(|(left, right)| left & !right == 0)
    }
}

fn player_neighborhood(position: Position, color: Color, cell: Cell) -> GraphSet {
    let mut neighborhood = GraphSet::empty();
    neighborhood.insert(usize::from(cell.index()));
    for neighbor in neighbors(cell, position.size()).into_iter().flatten() {
        if position.color_at(neighbor) != Some(color.other()) {
            neighborhood.insert(usize::from(neighbor.index()));
        }
    }

    let limit = position.size().get() - 1;
    match color {
        Color::Red => {
            if cell.rank() == 0 {
                neighborhood.insert(FIRST_EDGE);
            }
            if cell.rank() == limit {
                neighborhood.insert(SECOND_EDGE);
            }
        }
        Color::Blue => {
            if cell.file() == 0 {
                neighborhood.insert(FIRST_EDGE);
            }
            if cell.file() == limit {
                neighborhood.insert(SECOND_EDGE);
            }
        }
    }
    neighborhood
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BoardSize, GameResult, Seat, SwapRule};
    use std::collections::{HashMap, HashSet};

    fn position(moves: &[&str]) -> Position {
        let moves = moves
            .iter()
            .map(|mv| mv.parse().expect("valid move"))
            .collect::<Vec<_>>();
        Position::from_moves(BoardSize::new(9).unwrap(), SwapRule::Disabled, &moves).unwrap()
    }

    fn cell(name: &str) -> Cell {
        name.parse::<Move>().unwrap().cell().unwrap()
    }

    #[test]
    fn immediate_win_restricts_play_to_winning_cells() {
        let position = position(&[
            "a1", "h1", "a2", "h2", "a3", "h3", "a4", "h4", "a5", "h5", "a6", "h6", "a7", "h7",
            "a8", "h8",
        ]);

        let knowledge = analyze(position, true);

        assert_eq!(knowledge.must_play.unwrap().count(), 1);
        assert!(knowledge.allows(Move::Place(cell("a9"))));
        assert!(!knowledge.allows(Move::Place(cell("e5"))));
    }

    #[test]
    fn unique_opponent_win_becomes_a_mandatory_block() {
        let position = position(&[
            "a9", "a1", "b9", "b1", "c9", "c1", "d9", "d1", "e9", "e1", "f9", "f1", "g9", "g1",
            "h9", "h1",
        ]);

        let knowledge = analyze(position, true);

        assert_eq!(knowledge.must_play.unwrap().count(), 1);
        assert!(knowledge.allows(Move::Place(cell("i1"))));
        assert!(!knowledge.allows(Move::Place(cell("e5"))));
    }

    #[test]
    fn graph_neighborhood_superset_prunes_the_weaker_move() {
        let position = position(&["a9", "d5", "b9", "e4", "c9", "d6"]);

        let knowledge = analyze(position, true);

        assert!(knowledge.pruned.contains(cell("e5")));
        assert!(!knowledge.pruned.contains(cell("f5")));
        assert!(!knowledge.allows(Move::Place(cell("e5"))));
        assert!(knowledge.allows(Move::Place(cell("f5"))));
    }

    #[test]
    fn an_intact_bridge_alone_does_not_prove_an_intrusion_reversible() {
        let position = Position::from_moves(
            BoardSize::for_test(3),
            SwapRule::Disabled,
            &["a1", "b1", "c3", "c2"].map(|mv| mv.parse().unwrap()),
        )
        .unwrap();

        let knowledge = analyze(position, true);

        assert!(knowledge.allows(Move::Place(cell("b2"))));
    }

    #[test]
    fn every_pruned_move_has_an_unpruned_domination_witness() {
        let positions = [
            position(&["a9", "d5", "b9", "e4", "c9", "d6"]),
            position(&["i1", "f5", "i2", "e6", "i3", "f4"]),
        ];

        for position in positions {
            let knowledge = analyze(position, true);
            for candidate in position.empty_cells() {
                if !knowledge.pruned.contains(candidate) {
                    continue;
                }
                let candidate_neighborhood =
                    player_neighborhood(position, position.color_to_move(), candidate);
                let witnessed = neighbors(candidate, position.size())
                    .into_iter()
                    .flatten()
                    .filter(|&dominator| {
                        position.color_at(dominator).is_none()
                            && !knowledge.pruned.contains(dominator)
                            && knowledge
                                .must_play
                                .is_none_or(|moves| moves.contains(dominator))
                    })
                    .any(|dominator| {
                        candidate_neighborhood.is_subset_of(player_neighborhood(
                            position,
                            position.color_to_move(),
                            dominator,
                        ))
                    });
                assert!(witnessed, "{candidate} has no surviving dominator");
            }
        }
    }

    #[test]
    fn pie_rule_keeps_swap_and_all_placements_available() {
        let position = Position::new(BoardSize::new(9).unwrap(), SwapRule::Enabled)
            .play("e5".parse().unwrap())
            .unwrap();

        let knowledge = analyze(position, true);

        assert_eq!(knowledge, Knowledge::default());
        assert!(knowledge.allows(Move::Swap));
    }

    #[test]
    fn exhaustive_three_by_three_audit_preserves_a_winning_move() {
        let position = Position::new(BoardSize::for_test(3), SwapRule::Disabled);
        let mut solved = HashMap::new();
        let mut audited = HashSet::new();

        audit_reachable_positions(position, &mut solved, &mut audited);

        assert_eq!(audited.len(), 4_520);
    }

    #[test]
    fn sampled_four_by_four_audit_preserves_a_winning_move() {
        let mut random = 0x9e37_79b9_7f4a_7c15_u64;
        let mut solved = HashMap::new();
        let mut audited = HashSet::new();
        for _ in 0..512 {
            let mut position = Position::new(BoardSize::for_test(4), SwapRule::Disabled);
            for _ in 0..10 {
                if position.result() != GameResult::Ongoing {
                    break;
                }
                random = random.wrapping_add(0x9e37_79b9_7f4a_7c15).rotate_left(17);
                let legal = position.legal_moves();
                position = position.play(legal[random as usize % legal.len()]).unwrap();
            }
            if position.result() == GameResult::Ongoing {
                audit_position(position, &mut solved, &mut audited);
            }
        }

        assert!(
            audited.len() >= 300,
            "only audited {} states",
            audited.len()
        );
    }

    fn audit_position(
        position: Position,
        solved: &mut HashMap<u32, bool>,
        audited: &mut HashSet<u32>,
    ) {
        if !audited.insert(position_key(position)) {
            return;
        }
        let knowledge = analyze(position, true);
        let allowed = position
            .legal_moves()
            .into_iter()
            .filter(|&mv| knowledge.allows(mv))
            .collect::<Vec<_>>();
        let winning = can_force_win(position, solved);
        if let Some(proven) = knowledge.proven_winner {
            assert_eq!(
                proven == position.seat_to_move(),
                winning,
                "false knowledge proof at {}",
                position_key(position),
            );
        }
        assert!(!allowed.is_empty(), "knowledge removed every legal move");
        if winning {
            assert!(
                allowed
                    .iter()
                    .any(|&mv| move_preserves_win(position, mv, solved)),
                "knowledge removed every winning move at {}",
                position_key(position),
            );
        }
    }

    fn audit_reachable_positions(
        position: Position,
        solved: &mut HashMap<u32, bool>,
        audited: &mut HashSet<u32>,
    ) {
        if position.result() != GameResult::Ongoing || !audited.insert(position_key(position)) {
            return;
        }

        let knowledge = analyze(position, true);
        let legal = position.legal_moves();
        let allowed = legal
            .iter()
            .copied()
            .filter(|&mv| knowledge.allows(mv))
            .collect::<Vec<_>>();
        let winning = can_force_win(position, solved);
        if let Some(proven) = knowledge.proven_winner {
            assert_eq!(
                proven == position.seat_to_move(),
                winning,
                "false knowledge proof at {}",
                position_key(position),
            );
        }
        assert!(
            !allowed.is_empty(),
            "no allowed move at {}: {knowledge:?}",
            position_key(position)
        );
        if winning {
            assert!(
                allowed
                    .iter()
                    .copied()
                    .any(|mv| move_preserves_win(position, mv, solved)),
                "knowledge removed every winning move from {}",
                position_key(position),
            );
        }

        for mv in legal {
            audit_reachable_positions(position.play(mv).unwrap(), solved, audited);
        }
    }

    fn can_force_win(position: Position, solved: &mut HashMap<u32, bool>) -> bool {
        let key = position_key(position);
        if let Some(&winner) = solved.get(&key) {
            return winner;
        }
        let winner = position
            .legal_moves()
            .into_iter()
            .any(|mv| move_preserves_win(position, mv, solved));
        solved.insert(key, winner);
        winner
    }

    fn move_preserves_win(position: Position, mv: Move, solved: &mut HashMap<u32, bool>) -> bool {
        let mover = position.seat_to_move();
        let child = position.play(mv).unwrap();
        match child.result() {
            GameResult::Win(winner) => winner == mover,
            GameResult::Ongoing => !can_force_win(child, solved),
        }
    }

    fn position_key(position: Position) -> u32 {
        let mut key = u32::from(position.seat_to_move() == Seat::Two);
        for dense in 0..position.size().cell_count() {
            let cell = Cell::from_dense(dense, position.size());
            let digit = match position.color_at(cell) {
                None => 0,
                Some(Color::Red) => 1,
                Some(Color::Blue) => 2,
            };
            key = key * 3 + digit;
        }
        key
    }
}
