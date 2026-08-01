use ai_chess::Engine;
use std::io::{self, BufRead, Write};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, mpsc};
use std::thread;

struct Input {
    sequence: u64,
    line: String,
}

fn main() {
    let (sender, receiver) = mpsc::channel::<Input>();
    let stopped_search = Arc::new(AtomicU64::new(0));
    let input_stop = Arc::clone(&stopped_search);
    thread::spawn(move || {
        let stdin = io::stdin();
        let mut active_go = 0;
        for (offset, line) in stdin.lock().lines().enumerate() {
            let Ok(line) = line else {
                eprintln!("failed to read stdin");
                break;
            };
            let sequence = offset as u64 + 1;
            match line.split_whitespace().next() {
                Some("go") => active_go = sequence,
                Some("stop" | "quit") => input_stop.store(active_go, Ordering::Release),
                _ => {}
            }
            if sender.send(Input { sequence, line }).is_err() {
                break;
            }
        }
    });

    let mut stdout = io::stdout().lock();
    let mut engine = Engine::new();

    for input in receiver {
        let quit = input.line.trim() == "quit";
        for response in engine.command_until(&input.line, || {
            stopped_search.load(Ordering::Acquire) == input.sequence
        }) {
            if writeln!(stdout, "{response}").is_err() {
                return;
            }
        }
        if stdout.flush().is_err() || quit {
            break;
        }
    }
}
