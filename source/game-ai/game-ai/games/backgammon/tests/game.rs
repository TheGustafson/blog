use ai_backgammon::{Dice, Game, GameError, GamePhase, Play, Player};

fn finish_opening_turn(game: &mut Game, white: u8, black: u8) {
    assert!(game.opening_roll(white, black).unwrap());
    let play = game.legal_plays().remove(0);
    game.play(&play).unwrap();
    assert_eq!(game.phase(), GamePhase::PreRoll);
}

#[test]
fn tied_opening_rolls_are_repeated_without_changing_the_position() {
    let mut game = Game::new();
    let position = game.position();

    assert!(!game.opening_roll(4, 4).unwrap());
    assert_eq!(game.phase(), GamePhase::OpeningRoll);
    assert_eq!(game.position(), position);
}

#[test]
fn the_higher_opening_die_sets_the_first_player_and_first_turn() {
    let mut game = Game::new();

    assert!(game.opening_roll(2, 6).unwrap());
    assert_eq!(game.position().side_to_move(), Player::Black);
    assert_eq!(
        game.phase(),
        GamePhase::CheckerPlay(Dice::new(6, 2).unwrap())
    );
}

#[test]
fn a_complete_checker_play_advances_to_the_next_roll() {
    let mut game = Game::new();
    finish_opening_turn(&mut game, 6, 1);

    assert_eq!(game.position().side_to_move(), Player::Black);
    assert!(game.legal_plays().is_empty());
}

#[test]
fn actions_are_rejected_outside_their_phase() {
    let mut game = Game::new();
    let dice = Dice::new(4, 2).unwrap();

    assert_eq!(game.roll(dice), Err(GameError::WrongPhase));
    assert_eq!(game.play(&Play::pass()), Err(GameError::WrongPhase));

    finish_opening_turn(&mut game, 6, 1);
    assert_eq!(game.opening_roll(5, 2), Err(GameError::WrongPhase));
}

#[test]
fn restart_restores_the_opening_position_from_any_active_phase() {
    let mut game = Game::new();
    assert!(game.opening_roll(6, 1).unwrap());

    game.restart();

    assert_eq!(game, Game::new());
}
