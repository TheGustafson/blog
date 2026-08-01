#[cfg(target_arch = "wasm32")]
fn main() {}

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    native::run();
}

#[cfg(not(target_arch = "wasm32"))]
mod native {
    use ai_othello::{CassioEngine, Engine};
    use std::io::{self, BufRead, Write};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::{Arc, mpsc};
    use std::thread;

    pub fn run() {
        if std::env::args().any(|argument| argument == "--cassio" || argument == "-cassio") {
            run_cassio();
        } else {
            run_teaching_protocol();
        }
    }

    fn run_teaching_protocol() {
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

    struct Input {
        sequence: u64,
        line: String,
    }

    fn run_cassio() {
        let (sender, receiver) = mpsc::channel::<Input>();
        let stopped_search = Arc::new(AtomicU64::new(0));
        let input_stop = Arc::clone(&stopped_search);
        thread::spawn(move || {
            let stdin = io::stdin();
            let mut active_search = 0;
            for (offset, line) in stdin.lock().lines().enumerate() {
                let Ok(line) = line else {
                    eprintln!("failed to read stdin");
                    break;
                };
                let sequence = offset as u64 + 1;
                match line.split_whitespace().nth(1) {
                    Some("midgame-search" | "endgame-search") => active_search = sequence,
                    Some("stop" | "quit" | "eof") => {
                        input_stop.store(active_search, Ordering::Release);
                    }
                    _ => {}
                }
                if sender.send(Input { sequence, line }).is_err() {
                    break;
                }
            }
        });

        let mut stdout = io::stdout().lock();
        let mut engine = CassioEngine::new();
        for input in receiver {
            let command = input.line.split_whitespace().nth(1);
            let quit = matches!(command, Some("quit" | "eof"));
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
}
