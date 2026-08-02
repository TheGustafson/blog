use crate::board::{Cell, MAX_CELLS};
use crate::cell_set::CellSet;
use crate::connectivity::neighbors;
use crate::{Color, Position};
use std::collections::{HashMap, VecDeque};

const MAX_CARRIER_CELLS: u32 = 32;
const MAX_CONNECTIONS_PER_PAIR: usize = 4;
const MAX_DERIVED_CONNECTIONS: u32 = 12_000;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct ConnectionSummary {
    pub(crate) winning_vc: Option<CellSet>,
    pub(crate) winning_scs: [Option<SemiConnection>; MAX_CONNECTIONS_PER_PAIR],
    pub(crate) winning_sc_count: u8,
    pub(crate) virtual_connections: u32,
    pub(crate) semi_connections: u32,
    pub(crate) truncated: bool,
}

impl ConnectionSummary {
    pub(crate) fn winning_sc_carrier_intersection(self) -> Option<CellSet> {
        let mut connections = self.winning_scs[..usize::from(self.winning_sc_count)]
            .iter()
            .flatten();
        let first = connections.next()?.carrier;
        Some(connections.fold(first, |shared, connection| {
            shared.intersection(connection.carrier)
        }))
    }

    pub(crate) fn winning_keys(self) -> CellSet {
        self.winning_scs[..usize::from(self.winning_sc_count)]
            .iter()
            .flatten()
            .fold(CellSet::default(), |keys, connection| {
                keys.union(connection.keys)
            })
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct SemiConnection {
    pub(crate) carrier: CellSet,
    pub(crate) keys: CellSet,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Point {
    Chain,
    Empty(Cell),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Connection {
    carrier: CellSet,
    keys: CellSet,
}

#[derive(Clone, Debug, Default)]
struct PairConnections {
    virtuals: Vec<Connection>,
    semis: Vec<Connection>,
}

#[derive(Clone, Copy)]
struct QueuedVirtual {
    left: u16,
    right: u16,
    connection: Connection,
}

struct Search {
    points: Vec<Point>,
    edge_one: u16,
    edge_two: u16,
    pairs: HashMap<(u16, u16), PairConnections>,
    virtual_neighbors: Vec<Vec<u16>>,
    queue: VecDeque<QueuedVirtual>,
    virtual_connections: u32,
    semi_connections: u32,
    truncated: bool,
}

pub(crate) fn analyze(position: Position, color: Color) -> ConnectionSummary {
    let mut search = Search::new(position, color);
    search.run();
    search.summary()
}

impl Search {
    fn new(position: Position, color: Color) -> Self {
        let (points, point_for_cell, edge_one, edge_two) = build_points(position, color);
        let mut search = Self {
            virtual_neighbors: vec![Vec::new(); points.len()],
            points,
            edge_one,
            edge_two,
            pairs: HashMap::new(),
            queue: VecDeque::new(),
            virtual_connections: 0,
            semi_connections: 0,
            truncated: false,
        };

        let limit = position.size().get() - 1;
        for cell in position.empty_cells() {
            let empty = point_for_cell[usize::from(cell.index())].expect("empty point exists");
            let mut adjacent = Vec::with_capacity(8);
            for neighbor in neighbors(cell, position.size()).into_iter().flatten() {
                if position.color_at(neighbor) == Some(color) {
                    let chain = point_for_cell[usize::from(neighbor.index())]
                        .expect("friendly chain point exists");
                    if !adjacent.contains(&chain) {
                        adjacent.push(chain);
                    }
                }
            }
            match color {
                Color::Red => {
                    if cell.rank() == 0 {
                        adjacent.push(edge_one);
                    }
                    if cell.rank() == limit {
                        adjacent.push(edge_two);
                    }
                }
                Color::Blue => {
                    if cell.file() == 0 {
                        adjacent.push(edge_one);
                    }
                    if cell.file() == limit {
                        adjacent.push(edge_two);
                    }
                }
            }
            adjacent.sort_unstable();
            adjacent.dedup();
            for chain in adjacent {
                search.insert_virtual(empty, chain, CellSet::default());
            }
        }
        search
    }

    fn run(&mut self) {
        while let Some(queued) = self.queue.pop_front() {
            if self.virtual_connections + self.semi_connections >= MAX_DERIVED_CONNECTIONS {
                self.truncated = true;
                break;
            }
            self.extend_from(queued.left, queued.right, queued.connection);
            self.extend_from(queued.right, queued.left, queued.connection);
        }
    }

    fn extend_from(&mut self, midpoint: u16, outer: u16, connection: Connection) {
        let neighbors = self.virtual_neighbors[usize::from(midpoint)].clone();
        for other_outer in neighbors {
            if other_outer == outer {
                continue;
            }
            let other_connections = self
                .pairs
                .get(&pair(midpoint, other_outer))
                .map(|connections| connections.virtuals.clone())
                .unwrap_or_default();
            for other in other_connections {
                if connection.carrier.intersects(other.carrier)
                    || endpoint_in_carrier(&self.points, outer, other.carrier)
                    || endpoint_in_carrier(&self.points, other_outer, connection.carrier)
                {
                    continue;
                }
                if matches!(self.points[usize::from(outer)], Point::Empty(_))
                    && matches!(self.points[usize::from(other_outer)], Point::Empty(_))
                {
                    continue;
                }
                let carrier = connection.carrier.union(other.carrier);
                match self.points[usize::from(midpoint)] {
                    Point::Chain => self.insert_virtual(outer, other_outer, carrier),
                    Point::Empty(key) => {
                        let mut carrier = carrier;
                        carrier.insert(key);
                        let mut keys = CellSet::default();
                        keys.insert(key);
                        self.insert_semi(outer, other_outer, carrier, keys);
                    }
                }
            }
        }
    }

    fn insert_virtual(&mut self, left: u16, right: u16, carrier: CellSet) {
        if left == right || carrier.count() > MAX_CARRIER_CELLS {
            return;
        }
        let key = pair(left, right);
        let existed = self
            .pairs
            .get(&key)
            .is_some_and(|connections| !connections.virtuals.is_empty());
        let connection = Connection {
            carrier,
            keys: CellSet::default(),
        };
        if !insert_minimal(&mut self.pairs.entry(key).or_default().virtuals, connection) {
            return;
        }
        if !existed {
            self.virtual_neighbors[usize::from(left)].push(right);
            self.virtual_neighbors[usize::from(right)].push(left);
        }
        self.virtual_connections += 1;
        self.queue.push_back(QueuedVirtual {
            left,
            right,
            connection,
        });
    }

    fn insert_semi(&mut self, left: u16, right: u16, carrier: CellSet, keys: CellSet) {
        if left == right || carrier.count() > MAX_CARRIER_CELLS {
            return;
        }
        let key = pair(left, right);
        let connection = Connection { carrier, keys };
        if !insert_minimal(&mut self.pairs.entry(key).or_default().semis, connection) {
            return;
        }
        self.semi_connections += 1;
        self.apply_or_rule(left, right);
    }

    fn apply_or_rule(&mut self, left: u16, right: u16) {
        let semis = self
            .pairs
            .get(&pair(left, right))
            .map(|connections| connections.semis.clone())
            .unwrap_or_default();
        for first in 0..semis.len() {
            for second in first + 1..semis.len() {
                if !semis[first].carrier.intersects(semis[second].carrier) {
                    self.insert_virtual(
                        left,
                        right,
                        semis[first].carrier.union(semis[second].carrier),
                    );
                    continue;
                }
                for third in second + 1..semis.len() {
                    if semis[first]
                        .carrier
                        .intersection(semis[second].carrier)
                        .intersects(semis[third].carrier)
                    {
                        continue;
                    }
                    self.insert_virtual(
                        left,
                        right,
                        semis[first]
                            .carrier
                            .union(semis[second].carrier)
                            .union(semis[third].carrier),
                    );
                }
            }
        }
    }

    fn summary(&self) -> ConnectionSummary {
        let mut summary = ConnectionSummary {
            virtual_connections: self.virtual_connections,
            semi_connections: self.semi_connections,
            truncated: self.truncated,
            ..ConnectionSummary::default()
        };
        let Some(connections) = self.pairs.get(&pair(self.edge_one, self.edge_two)) else {
            return summary;
        };
        summary.winning_vc = connections
            .virtuals
            .first()
            .map(|connection| connection.carrier);
        for (slot, connection) in connections
            .semis
            .iter()
            .take(MAX_CONNECTIONS_PER_PAIR)
            .enumerate()
        {
            summary.winning_scs[slot] = Some(SemiConnection {
                carrier: connection.carrier,
                keys: connection.keys,
            });
            summary.winning_sc_count += 1;
        }
        summary
    }
}

fn insert_minimal(connections: &mut Vec<Connection>, connection: Connection) -> bool {
    if let Some(existing) = connections
        .iter_mut()
        .find(|existing| existing.carrier == connection.carrier)
    {
        existing.keys = existing.keys.union(connection.keys);
        return false;
    }
    if connections
        .iter()
        .any(|existing| existing.carrier.is_subset_of(connection.carrier))
    {
        return false;
    }
    connections.retain(|existing| !connection.carrier.is_subset_of(existing.carrier));
    connections.push(connection);
    connections.sort_by_key(|connection| connection.carrier.count());
    if connections.len() > MAX_CONNECTIONS_PER_PAIR {
        let retained = connections[..MAX_CONNECTIONS_PER_PAIR]
            .iter()
            .any(|existing| existing.carrier == connection.carrier);
        connections.truncate(MAX_CONNECTIONS_PER_PAIR);
        retained
    } else {
        true
    }
}

fn endpoint_in_carrier(points: &[Point], endpoint: u16, carrier: CellSet) -> bool {
    match points[usize::from(endpoint)] {
        Point::Chain => false,
        Point::Empty(cell) => carrier.contains(cell),
    }
}

const fn pair(left: u16, right: u16) -> (u16, u16) {
    if left < right {
        (left, right)
    } else {
        (right, left)
    }
}

fn build_points(
    position: Position,
    color: Color,
) -> (Vec<Point>, [Option<u16>; MAX_CELLS], u16, u16) {
    let mut union = DisjointSet::new();
    let first_edge = MAX_CELLS;
    let second_edge = MAX_CELLS + 1;
    let limit = position.size().get() - 1;
    for dense in 0..position.size().cell_count() {
        let cell = Cell::from_dense(dense, position.size());
        if position.color_at(cell) != Some(color) {
            continue;
        }
        for neighbor in neighbors(cell, position.size()).into_iter().flatten() {
            if position.color_at(neighbor) == Some(color) {
                union.join(usize::from(cell.index()), usize::from(neighbor.index()));
            }
        }
        match color {
            Color::Red => {
                if cell.rank() == 0 {
                    union.join(usize::from(cell.index()), first_edge);
                }
                if cell.rank() == limit {
                    union.join(usize::from(cell.index()), second_edge);
                }
            }
            Color::Blue => {
                if cell.file() == 0 {
                    union.join(usize::from(cell.index()), first_edge);
                }
                if cell.file() == limit {
                    union.join(usize::from(cell.index()), second_edge);
                }
            }
        }
    }

    let mut points = Vec::with_capacity(usize::from(position.size().cell_count()) + 2);
    let mut point_for_root = HashMap::new();
    let mut point_for_cell = [None; MAX_CELLS];
    for index in [first_edge, second_edge].into_iter().chain(
        (0..position.size().cell_count())
            .map(|dense| usize::from(Cell::from_dense(dense, position.size()).index())),
    ) {
        let cell = (index < MAX_CELLS).then(|| Cell::from_index(index as u16));
        if cell.is_some_and(|cell| position.color_at(cell) != Some(color)) {
            continue;
        }
        let root = union.find(index);
        let point = *point_for_root.entry(root).or_insert_with(|| {
            let point = points.len() as u16;
            points.push(Point::Chain);
            point
        });
        if let Some(cell) = cell {
            point_for_cell[usize::from(cell.index())] = Some(point);
        }
    }
    let edge_one = point_for_root[&union.find(first_edge)];
    let edge_two = point_for_root[&union.find(second_edge)];
    for cell in position.empty_cells() {
        let point = points.len() as u16;
        points.push(Point::Empty(cell));
        point_for_cell[usize::from(cell.index())] = Some(point);
    }
    (points, point_for_cell, edge_one, edge_two)
}

struct DisjointSet {
    parent: [u16; MAX_CELLS + 2],
}

impl DisjointSet {
    fn new() -> Self {
        Self {
            parent: std::array::from_fn(|index| index as u16),
        }
    }

    fn find(&mut self, index: usize) -> usize {
        let parent = usize::from(self.parent[index]);
        if parent == index {
            index
        } else {
            let root = self.find(parent);
            self.parent[index] = root as u16;
            root
        }
    }

    fn join(&mut self, left: usize, right: usize) {
        let left = self.find(left);
        let right = self.find(right);
        if left != right {
            self.parent[right] = left as u16;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BoardSize, GameResult, Move, Seat, SwapRule};
    use std::collections::{HashMap, HashSet};

    fn position(size: u8, moves: &[&str]) -> Position {
        let moves = moves
            .iter()
            .map(|mv| mv.parse::<Move>().unwrap())
            .collect::<Vec<_>>();
        Position::from_moves(BoardSize::for_test(size), SwapRule::Disabled, &moves).unwrap()
    }

    fn cell(name: &str) -> Cell {
        name.parse::<Move>().unwrap().cell().unwrap()
    }

    #[test]
    fn two_disjoint_join_points_make_a_virtual_connection() {
        let position = position(5, &["b2", "a1", "c3"]);
        let summary = analyze(position, Color::Red);

        assert!(summary.virtual_connections > 0);
        let mut bridge = CellSet::default();
        bridge.insert(cell("b3"));
        bridge.insert(cell("c2"));
        assert!(summary.winning_vc != Some(bridge));
    }

    #[test]
    fn a_single_join_point_makes_a_semi_connection() {
        let position = position(3, &["b1", "c1", "a3", "c2"]);
        let summary = analyze(position, Color::Red);

        assert!(summary.winning_sc_count > 0);
        assert!(summary.winning_keys().contains(cell("b2")));
    }

    #[test]
    fn linked_bridges_prove_a_side_to_side_connection() {
        let position = position(5, &["b1", "a1", "c2", "a2", "b4", "a3", "c5"]);
        let summary = analyze(position, Color::Red);

        assert!(summary.winning_vc.is_some(), "{summary:?}");
    }

    #[test]
    fn every_three_by_three_proof_agrees_with_perfect_play() {
        let position = Position::new(BoardSize::for_test(3), SwapRule::Disabled);
        let mut solved = HashMap::new();
        let mut audited = HashSet::new();

        audit_reachable_positions(position, &mut solved, &mut audited);

        assert_eq!(audited.len(), 4_520);
    }

    fn audit_reachable_positions(
        position: Position,
        solved: &mut HashMap<u32, bool>,
        audited: &mut HashSet<u32>,
    ) {
        if position.result() != GameResult::Ongoing || !audited.insert(position_key(position)) {
            return;
        }

        let mover = position.color_to_move();
        let own = analyze(position, mover);
        let opponent = analyze(position, mover.other());
        let winning = can_force_win(position, solved);

        if own.winning_vc.is_some() || own.winning_sc_count > 0 {
            assert!(winning, "false win proof at {}", position_key(position));
        }
        if opponent.winning_vc.is_some()
            || opponent
                .winning_sc_carrier_intersection()
                .is_some_and(CellSet::is_empty)
        {
            assert!(!winning, "false loss proof at {}", position_key(position));
        }
        if own.winning_sc_count > 0 {
            assert!(
                position.empty_cells().into_iter().any(|cell| {
                    own.winning_keys().contains(cell)
                        && move_preserves_win(position, Move::Place(cell), solved)
                }),
                "winning SC has no winning key at {}",
                position_key(position),
            );
        }
        if winning {
            if let Some(must_play) = opponent.winning_sc_carrier_intersection() {
                assert!(
                    position.empty_cells().into_iter().any(|cell| {
                        must_play.contains(cell)
                            && move_preserves_win(position, Move::Place(cell), solved)
                    }),
                    "opponent SCs exclude every winning defense at {}",
                    position_key(position),
                );
            }
        }

        for mv in position.legal_moves() {
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
