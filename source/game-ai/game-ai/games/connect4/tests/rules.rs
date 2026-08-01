use ai_connect4::verification::{ReferencePosition, reference_perft};
use ai_connect4::{Cell, Column, GameResult, Move, MoveError, Position, Side, perft};
use std::collections::HashMap;

fn moves(notation: &[&str]) -> Vec<Move> {
    notation
        .iter()
        .map(|value| value.parse().expect("valid test move"))
        .collect()
}

#[test]
fn notation_and_center_first_order_are_stable() {
    for (index, notation) in ["a", "b", "c", "d", "e", "f", "g"].iter().enumerate() {
        let column: Column = notation.parse().unwrap();
        assert_eq!(column.index(), index);
        assert_eq!(column.to_string(), *notation);
        assert_eq!((index + 1).to_string().parse(), Ok(column));
    }
    let order: Vec<String> = Position::start()
        .legal_moves()
        .into_iter()
        .map(|mv| mv.to_string())
        .collect();
    assert_eq!(order, ["d", "c", "e", "b", "f", "a", "g"]);
}

#[test]
fn the_addition_trick_finds_one_playable_cell_per_column() {
    let position = Position::start();
    assert_eq!(position.playable_bits().count_ones(), 7);
    for column in Column::all() {
        let cell = Cell::new(column, 0);
        assert_ne!(position.playable_bits() & (1u64 << cell.bit_index()), 0);
    }
}

#[test]
fn full_columns_are_rejected_and_every_move_unmakes_exactly() {
    let mut position = Position::start();
    let start = position;
    let mut undos = Vec::new();
    for _ in 0..6 {
        undos.push(position.make_move("a".parse().unwrap()).unwrap());
    }
    assert!(!position.can_play(Column::new(0)));
    assert_eq!(
        position.make_move("a".parse().unwrap()),
        Err(MoveError::Full(Column::new(0)))
    );
    while let Some(undo) = undos.pop() {
        position.unmake_move(undo);
    }
    assert_eq!(position, start);
}

#[test]
fn vertical_horizontal_and_both_diagonal_wins_are_detected() {
    let vertical = Position::from_moves(&moves(&["a", "b", "a", "b", "a", "b", "a"])).unwrap();
    assert_eq!(vertical.result(), GameResult::Win(Side::Red));
    assert_eq!(
        vertical.winning_cells(),
        moves(&["a", "a", "a", "a"])
            .iter()
            .enumerate()
            .map(|(row, mv)| Cell::new(mv.column(), row as u8))
            .collect::<Vec<_>>()
    );

    let horizontal = Position::from_moves(&moves(&["a", "a", "b", "b", "c", "c", "d"])).unwrap();
    assert_eq!(horizontal.result(), GameResult::Win(Side::Red));

    let rising = Position::from_moves(&moves(&[
        "a", "b", "b", "c", "c", "d", "c", "d", "d", "g", "d",
    ]))
    .unwrap();
    assert_eq!(rising.result(), GameResult::Win(Side::Red));
    let falling = Position::from_moves(
        &moves(&["a", "b", "b", "c", "c", "d", "c", "d", "d", "g", "d"])
            .into_iter()
            .map(Move::mirrored)
            .collect::<Vec<_>>(),
    )
    .unwrap();
    assert_eq!(falling.result(), GameResult::Win(Side::Red));
}

#[test]
fn mirror_is_an_involution_and_preserves_the_result() {
    let position = Position::from_moves(&moves(&["d", "c", "d", "e", "b", "f", "a"])).unwrap();
    let mirror = position.mirrored();
    assert_eq!(mirror.mirrored(), position);
    assert_eq!(mirror.result(), position.result());
    for column in Column::all() {
        for row in 0..Cell::ROWS as u8 {
            let cell = Cell::new(column, row);
            assert_eq!(position.side_at(cell), mirror.side_at(cell.mirrored()));
        }
    }
}

#[test]
fn perft_signatures_lock_move_generation_and_terminal_stopping() {
    let expected = [1u64, 7, 49, 343, 2_401, 16_807, 117_649, 823_536, 5_673_234];
    let mut position = Position::start();
    let mut reference = ReferencePosition::start();
    for (depth, nodes) in expected.into_iter().enumerate() {
        assert_eq!(perft(&mut position, depth as u8), nodes, "depth {depth}");
        assert_eq!(
            reference_perft(&mut reference, depth as u8),
            nodes,
            "reference depth {depth}"
        );
        assert_eq!(position, Position::start());
        assert_eq!(reference, ReferencePosition::start());
    }
}

#[test]
fn bitboard_matches_the_reference_tree() {
    fn compare(fast: &mut Position, reference: &mut ReferencePosition, depth: u8) {
        fn compare_with_keys(
            fast: &mut Position,
            reference: &mut ReferencePosition,
            depth: u8,
            keys: &mut HashMap<u64, Position>,
        ) {
            if let Some(existing) = keys.insert(fast.key(), *fast) {
                assert_eq!(existing, *fast, "compact key collision");
            }
            assert_eq!(fast.side_to_move(), reference.side_to_move());
            assert_eq!(fast.result(), reference.result());
            assert_eq!(
                fast.legal_moves().as_slice(),
                reference.legal_moves().as_slice()
            );
            for column in Column::all() {
                for row in 0..Cell::ROWS as u8 {
                    let cell = Cell::new(column, row);
                    assert_eq!(fast.side_at(cell), reference.side_at(cell));
                }
            }
            if depth == 0 {
                return;
            }
            let fast_before = *fast;
            let reference_before = *reference;
            for mv in fast.legal_moves() {
                let fast_undo = fast.make_move(mv).unwrap();
                let reference_undo = reference.make_move(mv).unwrap();
                compare_with_keys(fast, reference, depth - 1, keys);
                fast.unmake_move(fast_undo);
                reference.unmake_move(reference_undo);
                assert_eq!(*fast, fast_before);
                assert_eq!(*reference, reference_before);
                assert_eq!(fast.key(), fast_before.key());
            }
        }

        let mut keys = HashMap::new();
        compare_with_keys(fast, reference, depth, &mut keys);
    }

    let mut fast = Position::start();
    let mut reference = ReferencePosition::start();
    compare(&mut fast, &mut reference, 6);
    assert_eq!(perft(&mut fast, 6), reference_perft(&mut reference, 6));
}
