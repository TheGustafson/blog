use ai_hex::Engine;
use std::io::{self, BufRead};

fn main() {
    let mut engine = Engine::new();
    for line in io::stdin().lock().lines() {
        let line = match line {
            Ok(line) => line,
            Err(error) => {
                eprintln!("error: {error}");
                break;
            }
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
