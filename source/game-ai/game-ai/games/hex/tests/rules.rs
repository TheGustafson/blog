use ai_hex::{BoardSize, Color, GameResult, Move, MoveError, Position, Seat, SwapRule};

fn size(value: u8) -> BoardSize {
    BoardSize::new(value).unwrap()
}

#[test]
fn accepts_every_supported_board_size() {
    assert_eq!(BoardSize::default().get(), 13);
    assert!(BoardSize::new(8).is_err());
    for value in 9..=24 {
        assert_eq!(BoardSize::new(value).unwrap().get(), value);
    }
    assert!(BoardSize::new(25).is_err());
}

#[test]
fn parses_acute_notation_and_swap() {
    let a1 = "a1".parse::<Move>().unwrap();
    let x24 = "X24".parse::<Move>().unwrap();
    assert_eq!(a1.to_string(), "a1");
    assert_eq!(x24.to_string(), "x24");
    assert_eq!("SWAP".parse::<Move>().unwrap(), Move::Swap);
    assert!("i0".parse::<Move>().is_err());
    assert!("y1".parse::<Move>().is_err());
}

#[test]
fn rejects_cells_outside_the_current_board() {
    let position = Position::new(size(9), SwapRule::Enabled);
    assert_eq!(
        position.play("j1".parse().unwrap()),
        Err(MoveError::OutsideBoard)
    );
}

#[test]
fn swap_exchanges_colors_without_moving_the_opening_stone() {
    let position = Position::new(size(15), SwapRule::Enabled)
        .play("h8".parse().unwrap())
        .unwrap();
    assert_eq!(position.seat_to_move(), Seat::Two);
    assert_eq!(position.color_to_move(), Color::Blue);
    assert!(position.swap_available());

    let swapped = position.play(Move::Swap).unwrap();
    assert_eq!(swapped.seat_to_move(), Seat::One);
    assert_eq!(swapped.color_to_move(), Color::Blue);
    assert_eq!(swapped.color_for_seat(Seat::One), Color::Blue);
    assert_eq!(swapped.color_for_seat(Seat::Two), Color::Red);
    assert_eq!(
        swapped.color_at("h8".parse::<Move>().unwrap().cell().unwrap()),
        Some(Color::Red)
    );
    assert!(!swapped.swap_available());
    assert_eq!(swapped.stones(), 1);
    assert_eq!(swapped.actions(), 2);
}

#[test]
fn declining_swap_closes_the_window() {
    let position = Position::new(size(15), SwapRule::Enabled)
        .play("h8".parse().unwrap())
        .unwrap()
        .play("g8".parse().unwrap())
        .unwrap();
    assert!(!position.swap_available());
    assert_eq!(position.play(Move::Swap), Err(MoveError::SwapUnavailable));
}

#[test]
fn disabled_swap_rule_never_offers_swap() {
    let position = Position::new(size(15), SwapRule::Disabled)
        .play("h8".parse().unwrap())
        .unwrap();
    assert!(!position.swap_available());
    assert!(!position.legal_moves().contains(&Move::Swap));
}

#[test]
fn red_connects_top_to_bottom() {
    let mut moves = Vec::new();
    for rank in 1..=9 {
        moves.push(format!("a{rank}").parse::<Move>().unwrap());
        if rank < 9 {
            moves.push(format!("h{rank}").parse::<Move>().unwrap());
        }
    }
    let position = Position::from_moves(size(9), SwapRule::Disabled, &moves).unwrap();
    assert_eq!(position.result(), GameResult::Win(Seat::One));
    assert_eq!(position.winning_path().len(), 9);
    assert_eq!(position.winning_path().first().unwrap().rank(), 0);
    assert_eq!(position.winning_path().last().unwrap().rank(), 8);
    assert!(position.legal_moves().is_empty());
}

#[test]
fn blue_connects_left_to_right() {
    let mut moves = Vec::new();
    for file in 0..9 {
        moves.push(Move::place(file, 0).unwrap());
        moves.push(Move::place(file, 8).unwrap());
    }
    let position = Position::from_moves(size(9), SwapRule::Disabled, &moves).unwrap();
    assert_eq!(position.result(), GameResult::Win(Seat::Two));
    assert_eq!(position.winning_path().len(), 9);
    assert_eq!(position.winning_path().first().unwrap().file(), 0);
    assert_eq!(position.winning_path().last().unwrap().file(), 8);
}

#[test]
fn diagonal_hex_neighbors_form_a_connection() {
    let red = ["e1", "d2", "c3", "b4", "a5", "a6", "a7", "a8", "a9"];
    let blue = ["h1", "h2", "h3", "h4", "h5", "h6", "h7", "h8"];
    let moves = red
        .iter()
        .zip(blue)
        .flat_map(|(red, blue)| [red.parse().unwrap(), blue.parse().unwrap()])
        .chain(std::iter::once(red[8].parse().unwrap()))
        .collect::<Vec<_>>();
    let position = Position::from_moves(size(9), SwapRule::Disabled, &moves).unwrap();
    assert_eq!(position.result(), GameResult::Win(Seat::One));
}

#[test]
fn random_games_end_with_a_connected_winning_path() {
    let mut random = SplitMix64(0xdecafbad);
    for board_size in [9, 15, 24] {
        for _ in 0..24 {
            let mut position = Position::new(size(board_size), SwapRule::Enabled);
            while position.result() == GameResult::Ongoing {
                let moves = position.legal_moves();
                let mv = moves[random.index(moves.len())];
                position = position.play(mv).unwrap();
            }

            let GameResult::Win(winner) = position.result() else {
                unreachable!();
            };
            let color = position.color_for_seat(winner);
            let path = position.winning_path();
            assert!(!path.is_empty());
            assert!(
                path.iter()
                    .all(|&cell| position.color_at(cell) == Some(color))
            );
            assert!(path.windows(2).all(|pair| {
                let file = i16::from(pair[1].file()) - i16::from(pair[0].file());
                let rank = i16::from(pair[1].rank()) - i16::from(pair[0].rank());
                matches!(
                    (file, rank),
                    (1, 0) | (-1, 0) | (0, 1) | (0, -1) | (1, -1) | (-1, 1)
                )
            }));
            match color {
                Color::Red => {
                    assert_eq!(path.first().unwrap().rank(), 0);
                    assert_eq!(path.last().unwrap().rank(), board_size - 1);
                }
                Color::Blue => {
                    assert_eq!(path.first().unwrap().file(), 0);
                    assert_eq!(path.last().unwrap().file(), board_size - 1);
                }
            }
            assert!(position.actions() <= u16::from(board_size) * u16::from(board_size) + 1);
        }
    }
}

struct SplitMix64(u64);

impl SplitMix64 {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut value = self.0;
        value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        value ^ (value >> 31)
    }

    fn index(&mut self, length: usize) -> usize {
        (self.next() as usize) % length
    }
}
