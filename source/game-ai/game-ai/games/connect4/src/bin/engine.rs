use ai_connect4::Engine;
use std::io::{self, BufRead};

fn main() {
    let stdin = io::stdin();
    let mut engine = Engine::new();

    for line in stdin.lock().lines() {
        let Ok(line) = line else {
            eprintln!("failed to read stdin");
            break;
        };
        let quit = line.trim() == "quit";
        for response in engine.command(&line) {
            println!("{response}");
        }
        if quit {
            break;
        }
    }
}
