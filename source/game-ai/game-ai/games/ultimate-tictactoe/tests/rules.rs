use ai_ultimate_tictactoe::{
    GameResult, MiniResult, Move, MoveError, Player, Position, PositionStateError, perft,
};

const DRAW_X: u16 = 0b101_100_011;
const DRAW_O: u16 = 0b010_011_100;

#[test]
fn opening_allows_all_eighty_one_cells() {
    let position = Position::start();
    assert_eq!(position.legal_moves().len(), 81);
    for index in 0..81 {
        assert!(
            position
                .legal_moves()
                .contains(Move::from_global_index(index))
        );
    }
}

#[test]
fn local_cell_routes_the_next_player_to_the_matching_board() {
    let position = Position::start().play(Move::new(4, 0)).unwrap();
    assert_eq!(position.side_to_move(), Player::O);
    assert_eq!(position.active_board(), Some(0));
    assert_eq!(position.legal_moves().len(), 9);
    assert!(position.legal_moves().iter().all(|mv| mv.board() == 0));
}

#[test]
fn forced_board_is_enforced_even_when_other_cells_are_empty() {
    let position = Position::start().play(Move::new(4, 7)).unwrap();
    assert_eq!(
        position.play(Move::new(1, 0)),
        Err(MoveError::WrongBoard { expected: 7 })
    );
}

#[test]
fn occupied_cells_are_never_legal() {
    let first = Move::new(4, 4);
    let position = Position::start().play(first).unwrap();
    assert!(!position.legal_moves().contains(first));
    assert_eq!(position.play(first), Err(MoveError::Occupied));
}

#[test]
fn winning_a_mini_board_claims_and_closes_it_immediately() {
    let mut x = [0; 9];
    let mut o = [0; 9];
    x[0] = 0b000_000_011;
    o[0] = 0b000_011_000;
    let position = Position::from_cells(x, o, Some(0), Player::X).unwrap();

    let won = position.play(Move::new(0, 2)).unwrap();
    assert_eq!(won.mini_result(0), MiniResult::Win(Player::X));
    assert_eq!(won.active_board(), Some(2));
    assert!(!won.legal_moves().iter().any(|mv| mv.board() == 0));
}

#[test]
fn a_filled_unwon_board_is_a_closed_draw() {
    let mut x = [0; 9];
    let mut o = [0; 9];
    x[0] = DRAW_X & !(1 << 8);
    o[0] = DRAW_O;
    let position = Position::from_cells(x, o, Some(0), Player::X).unwrap();

    let drawn = position.play(Move::new(0, 8)).unwrap();
    assert_eq!(drawn.mini_result(0), MiniResult::Draw);
    assert_eq!(drawn.active_board(), Some(8));
    assert!(!drawn.legal_moves().iter().any(|mv| mv.board() == 0));
}

#[test]
fn routing_to_a_won_board_grants_the_wildcard_free_choice() {
    let mut x = [0; 9];
    let mut o = [0; 9];
    x[0] = 0b000_000_111;
    o[0] = 0b000_011_000;
    let position = Position::from_cells(x, o, Some(4), Player::O).unwrap();

    let wildcard = position.play(Move::new(4, 0)).unwrap();
    assert_eq!(wildcard.active_board(), None);
    assert!(wildcard.legal_moves().iter().any(|mv| mv.board() == 1));
    assert!(wildcard.legal_moves().iter().any(|mv| mv.board() == 8));
    assert!(!wildcard.legal_moves().iter().any(|mv| mv.board() == 0));
}

#[test]
fn routing_to_a_drawn_board_also_grants_the_wildcard() {
    let mut x = [0; 9];
    let mut o = [0; 9];
    x[0] = DRAW_X;
    o[0] = DRAW_O;
    let position = Position::from_cells(x, o, Some(4), Player::O).unwrap();

    let wildcard = position.play(Move::new(4, 0)).unwrap();
    assert_eq!(wildcard.active_board(), None);
    assert_eq!(wildcard.legal_moves().len(), 71);
    assert!(!wildcard.legal_moves().iter().any(|mv| mv.board() == 0));
}

#[test]
fn three_claimed_boards_in_any_macro_line_win_the_game() {
    for boards in [
        [0, 1, 2],
        [3, 4, 5],
        [6, 7, 8],
        [0, 3, 6],
        [1, 4, 7],
        [2, 5, 8],
        [0, 4, 8],
        [2, 4, 6],
    ] {
        let mut x = [0; 9];
        let mut o = [0; 9];
        for board in boards {
            x[board] = 0b000_000_111;
            o[board] = 0b000_011_000;
        }
        o[8] |= 1 << 8;
        o[7] |= 1 << 8;
        let position = Position::from_cells(x, o, None, Player::O).unwrap();
        assert_eq!(position.result(), GameResult::Win(Player::X));
        assert!(position.legal_moves().is_empty());
        assert_eq!(position.play(Move::new(4, 8)), Err(MoveError::GameOver));
    }
}

#[test]
fn nine_drawn_mini_boards_draw_the_macro_game() {
    let mut x = [0; 9];
    let mut o = [0; 9];
    for board in 0..9 {
        if board < 5 {
            x[board] = DRAW_X;
            o[board] = DRAW_O;
        } else {
            x[board] = DRAW_O;
            o[board] = DRAW_X;
        }
    }
    let position = Position::from_cells(x, o, None, Player::O).unwrap();
    assert_eq!(position.result(), GameResult::Draw);
    assert!(position.legal_moves().is_empty());
}

#[test]
fn drawn_boards_block_macro_lines() {
    let mut x = [0; 9];
    let mut o = [0; 9];
    x[0] = 0b000_000_111;
    o[0] = 0b000_011_000;
    x[1] = DRAW_X;
    o[1] = DRAW_O;
    x[2] = 0b000_000_111;
    o[2] = 0b000_011_000;
    o[3] = 0b000_000_111;
    x[3] = 0b000_011_000;
    x[4] = 1 << 4;
    o[4] = 1 << 4;
    o[5] = 0b000_000_011;
    assert_eq!(x.iter().map(|mask| mask.count_ones()).sum::<u32>(), 14);
    assert_eq!(o.iter().map(|mask| mask.count_ones()).sum::<u32>(), 14);
    let error = Position::from_cells(x, o, None, Player::X).unwrap_err();
    assert_eq!(error, PositionStateError::OverlappingCells { board: 4 });

    x[4] = 1 << 4;
    o[4] = 1 << 5;
    let position = Position::from_cells(x, o, None, Player::X).unwrap();
    assert_eq!(position.result(), GameResult::Ongoing);
}

#[test]
fn imported_positions_reject_impossible_state() {
    let mut x = [0; 9];
    let mut o = [0; 9];
    x[0] = 1;
    o[0] = 1;
    assert_eq!(
        Position::from_cells(x, o, None, Player::X),
        Err(PositionStateError::OverlappingCells { board: 0 })
    );

    let x = [1; 9];
    let o = [0; 9];
    assert_eq!(
        Position::from_cells(x, o, None, Player::X),
        Err(PositionStateError::ImpossibleTurnCounts)
    );
}

#[test]
fn move_generation_has_stable_reference_counts() {
    assert_eq!(perft(Position::start(), 0), 1);
    assert_eq!(perft(Position::start(), 1), 81);
    assert_eq!(perft(Position::start(), 2), 720);
}

#[test]
fn every_generated_move_applies_and_preserves_hash_identity() {
    let positions = [
        Position::start(),
        Position::start().play(Move::new(4, 0)).unwrap(),
        Position::from_moves(&[Move::new(4, 0), Move::new(0, 8)]).unwrap(),
    ];
    for position in positions {
        assert_eq!(position.hash(), position.hash());
        for mv in position.legal_moves().iter() {
            let child = position.play(mv).unwrap();
            assert_ne!(child.hash(), position.hash());
            assert_eq!(child.ply(), position.ply() + 1);
            assert_eq!(child.side_to_move(), position.side_to_move().other());
        }
    }
}

#[test]
fn a_complete_reference_game_ends_on_a_macro_win() {
    let moves = "e5 d6 a9 c9 i9 g7 a1 c1 i1 h3 e9 d8 a5 c5 i5 h4 e1 d2 a6 c8 g5 a4 b3 f9 h9 e7 e3 f7 g1 a3 c7 g2 c6 h8 d4 b2 f6 h7 e2 f8 h5 g3 a8 b6 g9 b8 h1"
        .split_whitespace()
        .map(|word| word.parse().unwrap())
        .collect::<Vec<_>>();
    let position = Position::from_moves(&moves).unwrap();
    let mut current = Position::start();
    let mut wildcard_plies = Vec::new();
    for (index, mv) in moves.iter().copied().enumerate() {
        current = current.play(mv).unwrap();
        if index > 0 && current.result() == GameResult::Ongoing && current.active_board().is_none()
        {
            wildcard_plies.push(index + 1);
        }
    }
    assert_eq!(wildcard_plies, [39, 41, 44, 46]);
    assert_eq!(position.result(), GameResult::Win(Player::X));
    assert_eq!(position.ply(), 47);
    assert!(position.legal_moves().is_empty());
}
