use crate::{
    Dice, Game, GameKind, GameOutcome, GamePhase, Location, Player, Point, SearchReport, Searcher,
    Step, Turn, search_preset,
};
use std::fmt::Write;

/// A stateful text-protocol session for native clients and the WASM worker.
pub struct Engine {
    game: Game,
    turn: Option<Turn>,
    last_roll: Option<[u8; 2]>,
    last_step: Option<Step>,
    last_passed: Option<Player>,
    last_analysis: Option<SearchReport>,
    searcher: Searcher,
}

impl Default for Engine {
    fn default() -> Self {
        Self::new()
    }
}

impl Engine {
    pub fn new() -> Self {
        Self {
            game: Game::new(),
            turn: None,
            last_roll: None,
            last_step: None,
            last_passed: None,
            last_analysis: None,
            searcher: Searcher::new(),
        }
    }

    /// Runs one protocol command and returns zero or more response lines.
    ///
    /// The commands are `gai`, `isready`, `newgame`, `opening`, `roll`,
    /// `step`, `undo`, `search`, `state`, and `quit`.
    pub fn command(&mut self, line: &str) -> Vec<String> {
        let words = line.split_whitespace().collect::<Vec<_>>();
        let Some(command) = words.first().copied() else {
            return vec![error("empty command")];
        };
        match command {
            "gai" => vec![
                "id name ai-backgammon".to_owned(),
                "id game backgammon".to_owned(),
                "gaiok".to_owned(),
            ],
            "isready" => vec!["readyok".to_owned()],
            "newgame" if words.len() == 1 => {
                self.replace_game();
                Vec::new()
            }
            "opening" => self.opening_roll(&words[1..]),
            "roll" => self.roll(&words[1..]),
            "step" => self.step(&words[1..]),
            "undo" if words.len() == 1 => self.undo(),
            "search" => self.search(&words[1..]),
            "state" if words.len() == 1 => vec![format!("state {}", self.snapshot_json())],
            "quit" => Vec::new(),
            _ => vec![error("unknown command or invalid arguments")],
        }
    }

    /// Serializes the current game, partial turn, and last search report as JSON.
    pub fn snapshot_json(&self) -> String {
        let position = self
            .turn
            .as_ref()
            .map_or_else(|| self.game.position(), Turn::preview_position);
        let phase = self.game.phase();
        let mut json = String::with_capacity(2_048);
        json.push_str("{\"points\":[");
        for (index, count) in position.points().iter().enumerate() {
            if index > 0 {
                json.push(',');
            }
            write!(json, "{count}").expect("writing to a String cannot fail");
        }
        write!(
            json,
            "],\"bar\":{{\"white\":{},\"black\":{}}},\"off\":{{\"white\":{},\"black\":{}}},",
            position.bar(Player::White),
            position.bar(Player::Black),
            position.off(Player::White),
            position.off(Player::Black),
        )
        .expect("writing to a String cannot fail");
        write!(
            json,
            "\"sideToMove\":\"{}\",\"phase\":\"{}\",",
            position.side_to_move(),
            phase_name(phase),
        )
        .expect("writing to a String cannot fail");
        json.push_str("\"dice\":");
        write_dice(&mut json, self.last_roll);
        json.push_str(",\"remainingDice\":[");
        if let Some(turn) = &self.turn {
            write_numbers(&mut json, &turn.remaining_dice());
        }
        json.push_str("],\"legalSteps\":[");
        if let Some(turn) = &self.turn {
            write_steps(&mut json, &turn.legal_steps());
        }
        json.push_str("],\"turnSteps\":[");
        if let Some(turn) = &self.turn {
            write_steps(&mut json, turn.steps());
        }
        json.push_str("],\"lastStep\":");
        if let Some(step) = self.last_step {
            write_step(&mut json, step);
        } else {
            json.push_str("null");
        }
        write!(
            json,
            ",\"canUndo\":{},\"openingTied\":{},\"lastPassed\":{},\"result\":{},\"analysis\":{}",
            self.turn
                .as_ref()
                .is_some_and(|turn| !turn.steps().is_empty()),
            matches!(phase, GamePhase::OpeningRoll) && self.last_roll.is_some(),
            player_json(self.last_passed),
            result_json(phase),
            analysis_json(self.last_analysis.as_ref()),
        )
        .expect("writing to a String cannot fail");
        json.push('}');
        json
    }

    fn opening_roll(&mut self, words: &[&str]) -> Vec<String> {
        let Some([white, black]) = parse_dice(words) else {
            return vec![error("expected: opening WHITE_DIE BLACK_DIE")];
        };
        match self.game.opening_roll(white, black) {
            Ok(false) => {
                self.last_analysis = None;
                self.last_roll = Some([white, black]);
                self.last_step = None;
                self.last_passed = None;
                vec!["info string opening roll tied".to_owned()]
            }
            Ok(true) => {
                self.last_analysis = None;
                self.last_roll = Some([white, black]);
                self.last_step = None;
                self.last_passed = None;
                self.begin_turn()
            }
            Err(message) => vec![error(&message.to_string())],
        }
    }

    fn roll(&mut self, words: &[&str]) -> Vec<String> {
        let Some([first, second]) = parse_dice(words) else {
            return vec![error("expected: roll FIRST_DIE SECOND_DIE")];
        };
        let dice = Dice::new(first, second).expect("parsed dice are valid");
        match self.game.roll(dice) {
            Ok(()) => {
                self.last_analysis = None;
                self.last_roll = Some([first, second]);
                self.last_step = None;
                self.last_passed = None;
                self.begin_turn()
            }
            Err(message) => vec![error(&message.to_string())],
        }
    }

    fn begin_turn(&mut self) -> Vec<String> {
        let GamePhase::CheckerPlay(dice) = self.game.phase() else {
            return vec![error("checker play did not start")];
        };
        let turn = match Turn::new(self.game.position(), dice) {
            Ok(turn) => turn,
            Err(message) => return vec![error(&message.to_string())],
        };
        if turn.is_pass() {
            let passed = self.game.position().side_to_move();
            let play = turn.finish().expect("a pass is a complete play");
            if let Err(message) = self.game.play(&play) {
                return vec![error(&message.to_string())];
            }
            self.turn = None;
            self.last_passed = Some(passed);
            vec![format!("info string {passed} has no legal checker play")]
        } else {
            self.turn = Some(turn);
            Vec::new()
        }
    }

    fn step(&mut self, words: &[&str]) -> Vec<String> {
        let [from, to, die] = words else {
            return vec![error("expected: step FROM TO DIE")];
        };
        let Some(from) = parse_location(from) else {
            return vec![error("step source must be bar or p1 through p24")];
        };
        let Some(to) = parse_location(to) else {
            return vec![error("step destination must be off or p1 through p24")];
        };
        let Ok(die) = die.parse::<u8>() else {
            return vec![error("step die must be from 1 through 6")];
        };
        let Ok(step) = Step::new(from, to, die) else {
            return vec![error("step needs a valid source, destination, and die")];
        };
        let Some(turn) = self.turn.as_mut() else {
            return vec![error("there is no checker turn in progress")];
        };
        if let Err(message) = turn.select(step) {
            return vec![error(&message.to_string())];
        }
        self.last_analysis = None;
        self.last_step = Some(step);
        if !turn.is_complete() {
            return Vec::new();
        }
        let turn = self.turn.take().expect("the completed turn exists");
        let play = turn
            .finish()
            .expect("the selected checker turn is complete");
        match self.game.play(&play) {
            Ok(()) => Vec::new(),
            Err(message) => vec![error(&message.to_string())],
        }
    }

    fn undo(&mut self) -> Vec<String> {
        let Some(turn) = self.turn.as_mut() else {
            return vec![error("there is no checker step to undo")];
        };
        if !turn.undo() {
            return vec![error("there is no checker step to undo")];
        }
        self.last_analysis = None;
        self.last_step = turn.steps().last().copied();
        Vec::new()
    }

    fn replace_game(&mut self) {
        self.game.restart();
        self.turn = None;
        self.last_roll = None;
        self.last_step = None;
        self.last_passed = None;
        self.last_analysis = None;
    }

    fn search(&mut self, words: &[&str]) -> Vec<String> {
        let [name] = words else {
            return vec![error("expected: search PRESET")];
        };
        let Some(preset) = search_preset(name) else {
            return vec![error("unknown search preset")];
        };
        let GamePhase::CheckerPlay(dice) = self.game.phase() else {
            return vec![error("search requires a checker turn")];
        };
        if self
            .turn
            .as_ref()
            .is_none_or(|turn| !turn.steps().is_empty())
        {
            return vec![error("search requires the start of a checker turn")];
        }

        let report = self
            .searcher
            .search(self.game.position(), dice, preset.options);
        let Some(play) = report.best_play.as_ref() else {
            return vec![error("search found no legal checker play")];
        };
        let output = vec![
            format!(
                "info depth {} nodes {} chance {} tthits {} equity {:.4}",
                report.depth,
                report.nodes,
                report.chance_nodes,
                report.tt_hits,
                report.equity.expected_points(),
            ),
            format!("bestplay {play}"),
        ];
        self.last_analysis = Some(report);
        output
    }
}

fn parse_dice(words: &[&str]) -> Option<[u8; 2]> {
    let [first, second] = words else {
        return None;
    };
    let first = first.parse::<u8>().ok()?;
    let second = second.parse::<u8>().ok()?;
    Dice::new(first, second).ok()?;
    Some([first, second])
}

fn parse_location(word: &str) -> Option<Location> {
    match word {
        "bar" => Some(Location::Bar),
        "off" => Some(Location::Off),
        _ => word
            .strip_prefix('p')?
            .parse::<u8>()
            .ok()
            .and_then(Point::new)
            .map(Location::Point),
    }
}

fn phase_name(phase: GamePhase) -> &'static str {
    match phase {
        GamePhase::OpeningRoll => "opening-roll",
        GamePhase::PreRoll => "pre-roll",
        GamePhase::CheckerPlay(_) => "checker-play",
        GamePhase::GameOver(_) => "game-over",
    }
}

fn player_json(player: Option<Player>) -> &'static str {
    match player {
        Some(Player::White) => "\"white\"",
        Some(Player::Black) => "\"black\"",
        None => "null",
    }
}

fn result_json(phase: GamePhase) -> String {
    let outcome = match phase {
        GamePhase::GameOver(outcome) => outcome,
        _ => return "null".to_owned(),
    };
    let mut json = String::new();
    write!(
        json,
        "{{\"winner\":\"{}\",\"kind\":\"{}\"}}",
        outcome.winner,
        game_kind_name(outcome),
    )
    .expect("writing to a String cannot fail");
    json
}

fn analysis_json(analysis: Option<&SearchReport>) -> String {
    let Some(analysis) = analysis else {
        return "null".to_owned();
    };
    let mut json = String::new();
    json.push_str("{\"play\":[");
    if let Some(play) = &analysis.best_play {
        write_steps(&mut json, play.steps());
    }
    let outcomes = analysis.equity.outcomes();
    write!(
        json,
        "],\"outcomes\":[{:.6},{:.6},{:.6},{:.6},{:.6},{:.6}],\"expectedPoints\":{:.6},\"depth\":{},\"nodes\":{},\"chanceNodes\":{},\"ttHits\":{},\"stopped\":{}}}",
        outcomes[0],
        outcomes[1],
        outcomes[2],
        outcomes[3],
        outcomes[4],
        outcomes[5],
        analysis.equity.expected_points(),
        analysis.depth,
        analysis.nodes,
        analysis.chance_nodes,
        analysis.tt_hits,
        analysis.stopped,
    )
    .expect("writing to a String cannot fail");
    json
}

fn game_kind_name(outcome: GameOutcome) -> &'static str {
    match outcome.kind {
        GameKind::Single => "single",
        GameKind::Gammon => "gammon",
        GameKind::Backgammon => "backgammon",
    }
}

fn write_dice(json: &mut String, dice: Option<[u8; 2]>) {
    if let Some([first, second]) = dice {
        write!(json, "[{first},{second}]").expect("writing to a String cannot fail");
    } else {
        json.push_str("null");
    }
}

fn write_numbers(json: &mut String, numbers: &[u8]) {
    for (index, number) in numbers.iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
        write!(json, "{number}").expect("writing to a String cannot fail");
    }
}

fn write_steps(json: &mut String, steps: &[Step]) {
    for (index, step) in steps.iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
        write_step(json, *step);
    }
}

fn write_step(json: &mut String, step: Step) {
    write!(
        json,
        "{{\"from\":\"{}\",\"to\":\"{}\",\"die\":{}}}",
        location_name(step.from()),
        location_name(step.to()),
        step.die(),
    )
    .expect("writing to a String cannot fail");
}

fn location_name(location: Location) -> String {
    match location {
        Location::Bar => "bar".to_owned(),
        Location::Point(point) => format!("p{}", point.number()),
        Location::Off => "off".to_owned(),
    }
}

fn error(message: &str) -> String {
    format!("info string error {message}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opening_roll_and_checker_steps_follow_the_game() {
        let mut engine = Engine::new();
        assert!(engine.command("opening 4 4")[0].contains("tied"));
        assert!(engine.snapshot_json().contains("\"openingTied\":true"));

        assert!(engine.command("opening 6 1").is_empty());
        assert!(
            engine
                .snapshot_json()
                .contains("\"phase\":\"checker-play\"")
        );
        let play = engine.game.legal_plays()[0].clone();
        for step in play.steps() {
            let output = engine.command(&format!(
                "step {} {} {}",
                location_name(step.from()),
                location_name(step.to()),
                step.die(),
            ));
            assert!(output.is_empty());
        }
        assert_eq!(engine.game.phase(), GamePhase::PreRoll);
        assert!(engine.turn.is_none());
    }

    #[test]
    fn undo_only_changes_the_pending_checker_turn() {
        let mut engine = Engine::new();
        engine.command("opening 6 1");
        let initial = engine.game.position();
        let step = engine.turn.as_ref().unwrap().legal_steps()[0];
        engine.command(&format!(
            "step {} {} {}",
            location_name(step.from()),
            location_name(step.to()),
            step.die(),
        ));
        assert_ne!(engine.turn.as_ref().unwrap().preview_position(), initial);
        assert!(engine.command("undo").is_empty());
        assert_eq!(engine.turn.as_ref().unwrap().preview_position(), initial);
        assert!(!engine.command("undo")[0].is_empty());
    }

    #[test]
    fn new_game_clears_an_active_turn_and_transient_state() {
        let mut engine = Engine::new();
        engine.command("opening 6 1");
        engine.command("search beginner");
        let step = engine.turn.as_ref().unwrap().legal_steps()[0];
        engine.command(&format!(
            "step {} {} {}",
            location_name(step.from()),
            location_name(step.to()),
            step.die(),
        ));

        assert!(engine.command("newgame").is_empty());
        assert_eq!(engine.game, Game::new());
        assert!(engine.turn.is_none());
        assert!(engine.last_roll.is_none());
        assert!(engine.last_step.is_none());
        assert!(engine.last_analysis.is_none());
        assert!(
            engine
                .snapshot_json()
                .contains("\"phase\":\"opening-roll\"")
        );
    }

    #[test]
    fn malformed_commands_do_not_change_state() {
        let mut engine = Engine::new();
        let before = engine.snapshot_json();
        assert!(engine.command("opening 7 1")[0].starts_with("info string error"));
        assert!(engine.command("roll 6 6")[0].starts_with("info string error"));
        assert!(engine.command("step p24 p23 1")[0].starts_with("info string error"));
        assert!(engine.command("newgame extra")[0].starts_with("info string error"));
        assert_eq!(engine.snapshot_json(), before);
    }

    #[test]
    fn search_reports_a_complete_legal_play_without_moving() {
        let mut engine = Engine::new();
        engine.command("opening 6 1");
        let before = engine.game.position();
        let output = engine.command("search medium");
        let analysis = engine.last_analysis.as_ref().unwrap();
        let play = analysis.best_play.as_ref().unwrap();

        assert!(output[0].starts_with("info depth "));
        assert!(output[1].starts_with("bestplay "));
        assert!(before.legal_plays(Dice::new(6, 1).unwrap()).contains(play));
        assert_eq!(engine.game.position(), before);
        assert!(engine.snapshot_json().contains("\"analysis\":{\"play\":["));
    }

    #[test]
    fn search_rejects_wrong_phases_presets_and_partial_turns_without_mutation() {
        let mut engine = Engine::new();
        let opening = engine.snapshot_json();
        assert!(engine.command("search medium")[0].contains("requires a checker turn"));
        assert!(engine.command("search bogus")[0].contains("unknown search preset"));
        assert_eq!(engine.snapshot_json(), opening);

        engine.command("opening 6 1");
        let step = engine.turn.as_ref().unwrap().legal_steps()[0];
        engine.command(&format!(
            "step {} {} {}",
            location_name(step.from()),
            location_name(step.to()),
            step.die(),
        ));
        let partial = engine.snapshot_json();
        assert!(engine.command("search medium")[0].contains("start of a checker turn"));
        assert_eq!(engine.snapshot_json(), partial);
    }

    #[test]
    fn successful_state_changes_clear_stale_analysis() {
        let mut engine = Engine::new();
        engine.command("opening 6 1");
        engine.command("search beginner");
        assert!(engine.last_analysis.is_some());
        let step = engine.turn.as_ref().unwrap().legal_steps()[0];
        assert!(
            engine
                .command(&format!(
                    "step {} {} {}",
                    location_name(step.from()),
                    location_name(step.to()),
                    step.die(),
                ))
                .is_empty()
        );
        assert!(engine.last_analysis.is_none());
        assert!(engine.snapshot_json().contains("\"analysis\":null"));
    }

    #[test]
    fn analysis_json_contains_normalized_outcomes_and_search_counts() {
        let mut engine = Engine::new();
        engine.command("opening 3 1");
        engine.command("search easy");
        let snapshot = engine.snapshot_json();
        assert!(snapshot.contains("\"outcomes\":["));
        assert!(snapshot.contains("\"expectedPoints\":"));
        assert!(snapshot.contains("\"depth\":1"));
        assert!(snapshot.contains("\"nodes\":"));
        assert!(snapshot.contains("\"chanceNodes\":"));
        assert!(snapshot.contains("\"ttHits\":"));
    }

    #[test]
    fn snapshot_contains_only_single_game_state() {
        let snapshot = Engine::new().snapshot_json();

        assert!(snapshot.contains("\"phase\":\"opening-roll\""));
        assert!(snapshot.contains("\"sideToMove\":\"white\""));
        assert!(snapshot.contains("\"result\":null"));
    }

    #[test]
    fn completed_game_result_has_only_the_natural_outcome() {
        let phase = GamePhase::GameOver(GameOutcome {
            winner: Player::Black,
            kind: GameKind::Backgammon,
        });

        assert_eq!(
            result_json(phase),
            "{\"winner\":\"black\",\"kind\":\"backgammon\"}",
        );
    }
}
