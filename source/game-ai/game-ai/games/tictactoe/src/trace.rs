use crate::mv::Move;
use crate::position::{Position, Side};
use crate::search::Outcome;
use crate::tablebase::Tablebase;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeEdge {
    pub mv: Move,
    pub outcome: Outcome,
    pub distance: u8,
    pub canonical_key: usize,
    pub children: Vec<Self>,
}

/// A bounded tree annotated with exact tablebase values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchTree {
    pub depth: u8,
    pub nodes: u64,
    pub side_to_move: Side,
    pub outcome: Outcome,
    pub distance: u8,
    pub children: Vec<TreeEdge>,
}

/// Builds an exact, tablebase-annotated tree up to `depth` plies.
pub fn build_tree(position: Position, depth: u8, tablebase: &Tablebase) -> SearchTree {
    let solved = tablebase.value(position);
    let mut nodes = 1;
    let children = expand(position, depth, tablebase, &mut nodes);
    SearchTree {
        depth,
        nodes,
        side_to_move: position.side_to_move(),
        outcome: solved.outcome,
        distance: solved.distance,
        children,
    }
}

fn expand(
    position: Position,
    remaining: u8,
    tablebase: &Tablebase,
    nodes: &mut u64,
) -> Vec<TreeEdge> {
    if remaining == 0 {
        return Vec::new();
    }

    position
        .legal_moves()
        .map(|mv| {
            let mut child = position;
            child.make_move(mv).expect("generated tree move is legal");
            *nodes += 1;
            let child_value = tablebase.value(child);
            TreeEdge {
                mv,
                outcome: child_value.outcome.negate(),
                distance: child_value.distance.saturating_add(1),
                canonical_key: child.canonical_key(),
                children: expand(child, remaining - 1, tablebase, nodes),
            }
        })
        .collect()
}
