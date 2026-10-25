use ai_crosshair::{
    ACTIONS, CELLS, Color, GameResult, INPUTS, MctsOptions, MctsSearcher, Move, PolicyValueNetwork,
    Position, Strategy, WinReason, canonical_index, encode,
};
use std::env;
use std::fmt::Write as _;
use std::fs::File;
use std::io::{BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::str::FromStr;
use std::time::Instant;

const DATA_MAGIC: &[u8; 8] = b"CXHRDAT1";
const DATA_VERSION: u32 = 1;
const DATA_HEADER_BYTES: usize = 16;
const SAMPLE_FLOATS: usize = INPUTS + ACTIONS + 1; // 344
const SAMPLE_BYTES: usize = SAMPLE_FLOATS * 4; // 1376

const USAGE: &str = "\
Usage:
  1. Self-play data generation:
     cargo run --release --bin selfplay -- --datagen --games <N> --simulations <SIMS> --out <PATH> \\
       [--weights <WEIGHTS_PATH>] [--seed <SEED>] [--dirichlet-alpha 0.3] [--dirichlet-eps 0.25] \\
       [--temp-plies 12] [--augment-symmetries] [--threads <T>]

  2. Arena match evaluation:
     cargo run --release --bin selfplay -- --arena --games <N> --simulations <SIMS> \\
       --candidate <STRATEGY_OR_WEIGHTS> --baseline <STRATEGY_OR_WEIGHTS> \\
       [--opening-plies 4] [--seed <SEED>] [--depth <D>] [--threads <T>]

  3. Forward-pass parity verification:
     cargo run --release --bin selfplay -- --verify-forward <SAMPLES_FILE> <WEIGHTS_FILE> [--count <K>]

  4. Seeded weight initialization:
     cargo run --release --bin selfplay -- --init-weights <OUT_PATH> [--seed <SEED>]";

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("error: {message}\n\n{USAGE}");
            ExitCode::FAILURE
        }
    }
}

enum CommandMode {
    Datagen(DatagenArgs),
    Arena(ArenaArgs),
    VerifyForward(VerifyArgs),
    InitWeights { out: PathBuf, seed: u64 },
}

#[derive(Clone)]
struct DatagenArgs {
    games: usize,
    simulations: u32,
    out: PathBuf,
    weights: Option<PathBuf>,
    seed: u64,
    dirichlet_alpha: f64,
    dirichlet_eps: f64,
    temp_plies: u8,
    augment_symmetries: bool,
    threads: usize,
}

#[derive(Clone)]
struct ArenaArgs {
    games: usize,
    simulations: u32,
    candidate: String,
    baseline: String,
    opening_plies: u8,
    seed: u64,
    depth: u8,
    threads: usize,
}

#[derive(Clone)]
struct VerifyArgs {
    samples_file: PathBuf,
    weights_file: PathBuf,
    count: usize,
}

#[derive(Clone)]
struct SampleRecord {
    input: [f32; INPUTS],
    policy: [f32; ACTIONS],
    value: f32,
}

fn run() -> Result<(), String> {
    let mode = parse_cli()?;
    match mode {
        CommandMode::Datagen(args) => run_datagen(&args),
        CommandMode::Arena(args) => run_arena(&args),
        CommandMode::VerifyForward(args) => run_verify_forward(&args),
        CommandMode::InitWeights { out, seed } => {
            let net = PolicyValueNetwork::seeded(seed);
            let bytes = net.to_bytes();
            if let Some(parent) = out.parent().filter(|p| !p.as_os_str().is_empty()) {
                std::fs::create_dir_all(parent)
                    .map_err(|e| format!("failed to create directory {}: {e}", parent.display()))?;
            }
            std::fs::write(&out, &bytes)
                .map_err(|e| format!("failed to write {}: {e}", out.display()))?;
            println!(
                "Initialized CXHRNET1 weights ({} bytes, seed {seed}) at {}",
                bytes.len(),
                out.display()
            );
            Ok(())
        }
    }
}

fn parse_cli() -> Result<CommandMode, String> {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.is_empty() {
        return Err(
            "missing mode flag (--datagen, --arena, --verify-forward, or --init-weights)"
                .to_owned(),
        );
    }

    let default_threads = std::thread::available_parallelism()
        .map(usize::from)
        .unwrap_or(1);

    if args.iter().any(|a| a == "--verify-forward") {
        let mut positional = Vec::new();
        let mut count = 8_usize;
        let mut i = 0;
        while i < args.len() {
            match args[i].as_str() {
                "--verify-forward" => i += 1,
                "--count" => {
                    let val = args.get(i + 1).ok_or("--count requires a value")?;
                    count = val.parse().map_err(|_| "invalid --count")?;
                    i += 2;
                }
                other if other.starts_with("--") => {
                    return Err(format!("unknown option {other} for --verify-forward"));
                }
                pos => {
                    positional.push(pos.to_owned());
                    i += 1;
                }
            }
        }
        if positional.len() != 2 {
            return Err(
                "expected: --verify-forward <SAMPLES_FILE> <WEIGHTS_FILE> [--count <K>]".to_owned(),
            );
        }
        return Ok(CommandMode::VerifyForward(VerifyArgs {
            samples_file: PathBuf::from(&positional[0]),
            weights_file: PathBuf::from(&positional[1]),
            count,
        }));
    }

    if args.iter().any(|a| a == "--init-weights") {
        let mut out = None;
        let mut seed = 0x4358_4852_3230_3236_u64;
        let mut i = 0;
        while i < args.len() {
            match args[i].as_str() {
                "--init-weights" => {
                    let val = args.get(i + 1).ok_or("--init-weights requires a path")?;
                    out = Some(PathBuf::from(val));
                    i += 2;
                }
                "--seed" => {
                    let val = args.get(i + 1).ok_or("--seed requires a value")?;
                    seed = val.parse().map_err(|_| "invalid --seed")?;
                    i += 2;
                }
                other => return Err(format!("unknown option {other} for --init-weights")),
            }
        }
        return Ok(CommandMode::InitWeights {
            out: out.ok_or("missing path for --init-weights")?,
            seed,
        });
    }

    let is_datagen = args.iter().any(|a| a == "--datagen");
    let is_arena = args.iter().any(|a| a == "--arena");
    if is_datagen == is_arena {
        return Err("specify exactly one of --datagen or --arena".to_owned());
    }

    if is_datagen {
        let mut datagen = DatagenArgs {
            games: 100,
            simulations: 400,
            out: PathBuf::from("selfplay.bin"),
            weights: None,
            seed: 1,
            dirichlet_alpha: 0.3,
            dirichlet_eps: 0.25,
            temp_plies: 12,
            augment_symmetries: false,
            threads: default_threads,
        };
        let mut has_out = false;
        let mut i = 0;
        while i < args.len() {
            match args[i].as_str() {
                "--datagen" => i += 1,
                "--augment-symmetries" => {
                    datagen.augment_symmetries = true;
                    i += 1;
                }
                flag => {
                    let val = args
                        .get(i + 1)
                        .ok_or_else(|| format!("{flag} requires a value"))?;
                    match flag {
                        "--games" => datagen.games = parse_val(val, "games")?,
                        "--simulations" => datagen.simulations = parse_val(val, "simulations")?,
                        "--out" => {
                            datagen.out = PathBuf::from(val);
                            has_out = true;
                        }
                        "--weights" => datagen.weights = Some(PathBuf::from(val)),
                        "--seed" => datagen.seed = parse_val(val, "seed")?,
                        "--dirichlet-alpha" => {
                            datagen.dirichlet_alpha = parse_val(val, "dirichlet-alpha")?
                        }
                        "--dirichlet-eps" => {
                            datagen.dirichlet_eps = parse_val(val, "dirichlet-eps")?
                        }
                        "--temp-plies" => datagen.temp_plies = parse_val(val, "temp-plies")?,
                        "--threads" => datagen.threads = parse_val(val, "threads")?,
                        _ => return Err(format!("unknown --datagen option {flag}")),
                    }
                    i += 2;
                }
            }
        }
        if !has_out {
            return Err("--datagen requires --out <PATH>".to_owned());
        }
        if datagen.games == 0 {
            return Err("--games must be at least 1".to_owned());
        }
        return Ok(CommandMode::Datagen(datagen));
    }

    let mut arena = ArenaArgs {
        games: 20,
        simulations: 400,
        candidate: "az-puct".to_owned(),
        baseline: "tactical-uct".to_owned(),
        opening_plies: 4,
        seed: 1,
        depth: 5,
        threads: default_threads,
    };
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--arena" => i += 1,
            flag => {
                let val = args
                    .get(i + 1)
                    .ok_or_else(|| format!("{flag} requires a value"))?;
                match flag {
                    "--games" => arena.games = parse_val(val, "games")?,
                    "--simulations" => arena.simulations = parse_val(val, "simulations")?,
                    "--candidate" => arena.candidate = val.clone(),
                    "--baseline" => arena.baseline = val.clone(),
                    "--opening-plies" => arena.opening_plies = parse_val(val, "opening-plies")?,
                    "--seed" => arena.seed = parse_val(val, "seed")?,
                    "--depth" => arena.depth = parse_val(val, "depth")?,
                    "--threads" => arena.threads = parse_val(val, "threads")?,
                    _ => return Err(format!("unknown --arena option {flag}")),
                }
                i += 2;
            }
        }
    }
    if arena.games == 0 {
        return Err("--games must be at least 1".to_owned());
    }
    Ok(CommandMode::Arena(arena))
}

fn parse_val<T: FromStr>(val: &str, name: &str) -> Result<T, String> {
    val.parse().map_err(|_| format!("invalid {name}: {val}"))
}

fn run_datagen(args: &DatagenArgs) -> Result<(), String> {
    let started = Instant::now();
    let network = match &args.weights {
        Some(path) => PolicyValueNetwork::from_file(path)?,
        None => PolicyValueNetwork::embedded().map_err(|e| e.to_owned())?,
    };

    let worker_count = args.threads.clamp(1, args.games);
    let mut game_results = Vec::with_capacity(args.games);

    std::thread::scope(|scope| {
        let mut handles = Vec::with_capacity(worker_count);
        for worker_id in 0..worker_count {
            let net = network.clone();
            let args_ref = args;
            handles.push(scope.spawn(move || {
                let mut searcher = MctsSearcher::with_network(net);
                let mut local_games = Vec::new();
                let mut g = worker_id;
                while g < args_ref.games {
                    let game_out = play_selfplay_game(&mut searcher, args_ref, g as u64);
                    local_games.push((g, game_out));
                    g += worker_count;
                }
                local_games
            }));
        }
        for handle in handles {
            let local = handle.join().expect("datagen worker panicked");
            game_results.extend(local);
        }
    });

    game_results.sort_by_key(|(g, _)| *g);

    let mut total_samples = 0_usize;
    let mut red_wins = 0_usize;
    let mut blue_wins = 0_usize;
    let mut connection_wins = 0_usize;
    let mut total_plies = 0_usize;

    for (_, outcome) in &game_results {
        total_samples += outcome.samples.len();
        total_plies += usize::from(outcome.plies);
        match outcome.winner {
            Color::Red => red_wins += 1,
            Color::Blue => blue_wins += 1,
        }
        match outcome.reason {
            WinReason::Connection => connection_wins += 1,
        }
    }

    if let Some(parent) = args.out.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("failed to create {}: {e}", parent.display()))?;
    }
    let file = File::create(&args.out)
        .map_err(|e| format!("failed to create {}: {e}", args.out.display()))?;
    let mut writer = BufWriter::new(file);

    writer
        .write_all(DATA_MAGIC)
        .map_err(|e| format!("write error: {e}"))?;
    writer
        .write_all(&DATA_VERSION.to_le_bytes())
        .map_err(|e| format!("write error: {e}"))?;
    writer
        .write_all(&(total_samples as u32).to_le_bytes())
        .map_err(|e| format!("write error: {e}"))?;

    for (_, outcome) in &game_results {
        for sample in &outcome.samples {
            for &x in &sample.input {
                writer
                    .write_all(&x.to_le_bytes())
                    .map_err(|e| format!("write error: {e}"))?;
            }
            for &p in &sample.policy {
                writer
                    .write_all(&p.to_le_bytes())
                    .map_err(|e| format!("write error: {e}"))?;
            }
            writer
                .write_all(&sample.value.to_le_bytes())
                .map_err(|e| format!("write error: {e}"))?;
        }
    }
    writer.flush().map_err(|e| format!("flush error: {e}"))?;

    let elapsed = started.elapsed().as_secs_f64();
    println!(
        "datagen complete: games={} samples={} avg_plies={:.1} red_wins={} blue_wins={} connection={} elapsed_s={:.2} out={}",
        args.games,
        total_samples,
        total_plies as f64 / args.games as f64,
        red_wins,
        blue_wins,
        connection_wins,
        elapsed,
        args.out.display()
    );
    Ok(())
}

struct SelfplayGameOutcome {
    winner: Color,
    reason: WinReason,
    plies: u8,
    samples: Vec<SampleRecord>,
}

fn play_selfplay_game(
    searcher: &mut MctsSearcher,
    args: &DatagenArgs,
    game_index: u64,
) -> SelfplayGameOutcome {
    let mut position = Position::start();
    let mut rng = SplitMix64(
        args.seed ^ game_index.wrapping_mul(0x9e37_79b9_7f4a_7c15) ^ 0x6a09_e667_f3bc_c908,
    );
    let mut trajectory: Vec<(Position, [f32; ACTIONS])> = Vec::with_capacity(CELLS);

    while position.result() == GameResult::Ongoing {
        let step_seed = rng.next();
        let report = searcher.search(
            position,
            MctsOptions {
                max_simulations: args.simulations,
                soft_time_ms: 0,
                strategy: Strategy::AzPuct,
                seed: step_seed,
                dirichlet_alpha: args.dirichlet_alpha,
                dirichlet_eps: args.dirichlet_eps,
                ..MctsOptions::default()
            },
        );

        let mut policy_49 = [0.0_f32; ACTIONS];
        let total_visits: u32 = report.root_moves.iter().map(|m| m.visits).sum();
        if total_visits > 0 {
            for m in &report.root_moves {
                policy_49[m.mv.index() as usize] = (m.visits as f32) / (total_visits as f32);
            }
        } else {
            for m in &report.root_moves {
                policy_49[m.mv.index() as usize] = m.prior as f32;
            }
        }
        trajectory.push((position, policy_49));

        let chosen = if position.ply() < args.temp_plies && total_visits > 0 {
            let threshold = rng.index(total_visits as usize) as u32;
            let mut acc = 0_u32;
            let mut picked = report.root_moves[0].mv;
            for m in &report.root_moves {
                acc += m.visits;
                if threshold < acc {
                    picked = m.mv;
                    break;
                }
            }
            picked
        } else {
            report
                .best_move
                .expect("ongoing position must return a best move")
        };

        position = position.play(chosen).expect("chosen move must be legal");
    }

    let GameResult::Win { winner, reason } = position.result() else {
        unreachable!()
    };

    let sym_count: u8 = if args.augment_symmetries { 4 } else { 1 };
    let mut samples = Vec::with_capacity(trajectory.len() * usize::from(sym_count));
    for (pos_t, policy_49) in trajectory {
        let side = pos_t.side_to_move();
        let value = if side == winner { 1.0_f32 } else { -1.0_f32 };
        for sym in 0..sym_count {
            let input = encode(pos_t, sym);
            let mut perm_policy = [0.0_f32; ACTIONS];
            for a in 0..ACTIONS as u8 {
                let target_idx = canonical_index(a, side, sym) as usize;
                perm_policy[target_idx] = policy_49[a as usize];
            }
            samples.push(SampleRecord {
                input,
                policy: perm_policy,
                value,
            });
        }
    }

    SelfplayGameOutcome {
        winner,
        reason,
        plies: position.ply(),
        samples,
    }
}

#[derive(Clone)]
struct AgentSpec {
    label: String,
    strategy: Strategy,
    network: PolicyValueNetwork,
}

impl AgentSpec {
    fn parse(spec: &str) -> Result<Self, String> {
        if let Ok(strategy) = Strategy::from_str(spec) {
            let network = PolicyValueNetwork::embedded().map_err(|e| e.to_owned())?;
            Ok(Self {
                label: strategy.to_string(),
                strategy,
                network,
            })
        } else {
            let network = PolicyValueNetwork::from_file(Path::new(spec))?;
            Ok(Self {
                label: spec.to_owned(),
                strategy: Strategy::AzPuct,
                network,
            })
        }
    }
}

fn run_arena(args: &ArenaArgs) -> Result<(), String> {
    let started = Instant::now();
    let candidate = AgentSpec::parse(&args.candidate)?;
    let baseline = AgentSpec::parse(&args.baseline)?;

    let pairs = args.games.div_ceil(2);
    let mut openings = Vec::with_capacity(pairs);
    for p in 0..pairs {
        openings.push(generate_opening(args.opening_plies, args.seed, p as u64));
    }

    let worker_count = args.threads.clamp(1, args.games);
    let mut outcomes = Vec::with_capacity(args.games);

    std::thread::scope(|scope| {
        let mut handles = Vec::with_capacity(worker_count);
        for worker_id in 0..worker_count {
            let cand = candidate.clone();
            let base = baseline.clone();
            let openings_ref = &openings;
            let args_ref = args;
            handles.push(scope.spawn(move || {
                let mut cand_searcher = MctsSearcher::with_network(cand.network.clone());
                let mut base_searcher = MctsSearcher::with_network(base.network.clone());
                let mut local = Vec::new();
                let mut g = worker_id;
                while g < args_ref.games {
                    let pair_idx = g / 2;
                    let candidate_is_red = g % 2 == 0;
                    let opening = &openings_ref[pair_idx];
                    let (cand_won, reason, plies) = play_arena_game(
                        &mut cand_searcher,
                        cand.strategy,
                        &mut base_searcher,
                        base.strategy,
                        opening,
                        candidate_is_red,
                        args_ref,
                        g as u64,
                    );
                    local.push((g, cand_won, candidate_is_red, reason, plies));
                    g += worker_count;
                }
                local
            }));
        }
        for handle in handles {
            let local = handle.join().expect("arena worker panicked");
            outcomes.extend(local);
        }
    });

    outcomes.sort_by_key(|(g, ..)| *g);

    let mut candidate_wins = 0_usize;
    let mut baseline_wins = 0_usize;
    let mut candidate_red_wins = 0_usize;
    let mut candidate_blue_wins = 0_usize;
    let mut connection_wins = 0_usize;
    let mut total_plies = 0_usize;

    for &(_, cand_won, cand_is_red, reason, plies) in &outcomes {
        total_plies += usize::from(plies);
        if cand_won {
            candidate_wins += 1;
            if cand_is_red {
                candidate_red_wins += 1;
            } else {
                candidate_blue_wins += 1;
            }
        } else {
            baseline_wins += 1;
        }
        match reason {
            WinReason::Connection => connection_wins += 1,
        }
    }

    let win_rate = candidate_wins as f64 / args.games as f64;
    let elapsed_s = started.elapsed().as_secs_f64();

    println!(
        "arena: candidate={} vs baseline={} games={} sims={} candidate_wins={} baseline_wins={} win_rate={:.4} (as_red={} as_blue={}) connection={} avg_plies={:.1} elapsed_s={:.2}",
        candidate.label,
        baseline.label,
        args.games,
        args.simulations,
        candidate_wins,
        baseline_wins,
        win_rate,
        candidate_red_wins,
        candidate_blue_wins,
        connection_wins,
        total_plies as f64 / args.games as f64,
        elapsed_s,
    );
    println!(
        "{{\"candidate\":\"{}\",\"baseline\":\"{}\",\"games\":{},\"simulations\":{},\"candidate_wins\":{},\"baseline_wins\":{},\"win_rate\":{:.4},\"connection_wins\":{},\"avg_plies\":{:.2},\"elapsed_s\":{:.2}}}",
        candidate.label,
        baseline.label,
        args.games,
        args.simulations,
        candidate_wins,
        baseline_wins,
        win_rate,
        connection_wins,
        total_plies as f64 / args.games as f64,
        elapsed_s,
    );
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn play_arena_game(
    cand_searcher: &mut MctsSearcher,
    cand_strategy: Strategy,
    base_searcher: &mut MctsSearcher,
    base_strategy: Strategy,
    opening: &[Move],
    candidate_is_red: bool,
    args: &ArenaArgs,
    game_index: u64,
) -> (bool, WinReason, u8) {
    let mut position = Position::from_moves(opening).expect("opening moves are legal");
    while position.result() == GameResult::Ongoing {
        let is_red = position.side_to_move() == Color::Red;
        let candidate_turn = is_red == candidate_is_red;
        let step_seed = args.seed
            ^ game_index.wrapping_mul(0x9e37_79b9_7f4a_7c15)
            ^ u64::from(position.ply()).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        let report = if candidate_turn {
            cand_searcher.search(
                position,
                MctsOptions {
                    max_simulations: args.simulations,
                    strategy: cand_strategy,
                    seed: step_seed,
                    alpha_beta_depth: args.depth,
                    ..MctsOptions::default()
                },
            )
        } else {
            base_searcher.search(
                position,
                MctsOptions {
                    max_simulations: args.simulations,
                    strategy: base_strategy,
                    seed: step_seed,
                    alpha_beta_depth: args.depth,
                    ..MctsOptions::default()
                },
            )
        };
        let mv = report
            .best_move
            .expect("ongoing position must yield a move");
        position = position.play(mv).expect("search move must be legal");
    }
    let GameResult::Win { winner, reason } = position.result() else {
        unreachable!()
    };
    let cand_color = if candidate_is_red {
        Color::Red
    } else {
        Color::Blue
    };
    (winner == cand_color, reason, position.ply())
}

fn generate_opening(opening_plies: u8, seed: u64, pair_index: u64) -> Vec<Move> {
    let mut rng =
        SplitMix64(seed ^ pair_index.wrapping_mul(0xd1b5_4a32_d192_ed03) ^ 0x94d0_49bb_1331_11eb);
    for _attempt in 0..64 {
        let mut pos = Position::start();
        let mut moves = Vec::with_capacity(usize::from(opening_plies));
        let mut ok = true;
        for _ in 0..opening_plies {
            let legal: Vec<Move> = pos.legal_moves().iter().collect();
            if legal.is_empty() {
                ok = false;
                break;
            }
            let mv = legal[rng.index(legal.len())];
            pos = pos.play(mv).expect("legal move");
            if pos.result() != GameResult::Ongoing {
                ok = false;
                break;
            }
            moves.push(mv);
        }
        if ok {
            return moves;
        }
    }
    Vec::new()
}

fn run_verify_forward(args: &VerifyArgs) -> Result<(), String> {
    let network = if args.weights_file.as_os_str() == "embedded" {
        PolicyValueNetwork::embedded().map_err(|e| e.to_owned())?
    } else {
        PolicyValueNetwork::from_file(&args.weights_file)?
    };

    let mut file = File::open(&args.samples_file)
        .map_err(|e| format!("failed to open {}: {e}", args.samples_file.display()))?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)
        .map_err(|e| format!("failed to read {}: {e}", args.samples_file.display()))?;

    if bytes.len() < DATA_HEADER_BYTES {
        return Err("sample file too short for 16-byte CXHRDAT1 header".to_owned());
    }
    if &bytes[..8] != DATA_MAGIC {
        return Err("wrong sample file magic (expected CXHRDAT1)".to_owned());
    }
    let version = u32::from_le_bytes(bytes[8..12].try_into().unwrap());
    if version != DATA_VERSION {
        return Err(format!("unsupported sample file version {version}"));
    }
    let num_samples = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
    let expected_len = DATA_HEADER_BYTES + num_samples * SAMPLE_BYTES;
    if bytes.len() != expected_len {
        return Err(format!(
            "sample file size mismatch: header claims {num_samples} samples ({expected_len} bytes), got {} bytes",
            bytes.len()
        ));
    }

    let k = args.count.min(num_samples);
    let mut json = String::with_capacity(4096);
    write!(
        json,
        "{{\"numSamples\":{num_samples},\"verified\":{k},\"samples\":["
    )
    .unwrap();

    for idx in 0..k {
        if idx > 0 {
            json.push(',');
        }
        let offset = DATA_HEADER_BYTES + idx * SAMPLE_BYTES;
        let record_bytes = &bytes[offset..offset + SAMPLE_BYTES];
        let mut input = [0.0_f32; INPUTS];
        for (i, slot) in input.iter_mut().enumerate() {
            let b = &record_bytes[i * 4..(i + 1) * 4];
            *slot = f32::from_le_bytes(b.try_into().unwrap());
        }
        let mut legal = [false; ACTIONS];
        for a in 0..ACTIONS {
            legal[a] = input[2 * CELLS + a] > 0.5;
        }
        let (policy, value) = network.evaluate_encoded(&input, &legal);
        write!(json, "{{\"index\":{idx},\"value\":{value:.8},\"policy\":[").unwrap();
        for (a, &p) in policy.iter().enumerate() {
            if a > 0 {
                json.push(',');
            }
            write!(json, "{p:.8}").unwrap();
        }
        json.push_str("]}");
    }
    json.push_str("]}");
    println!("{json}");
    Ok(())
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

    fn index(&mut self, len: usize) -> usize {
        (self.next() as usize) % len
    }
}
