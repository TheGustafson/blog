"""AlphaZero self-play, GPU training, forward-parity verification, and arena gating loop."""

from __future__ import annotations

import argparse
import json
import os
import re
import shutil
import subprocess
import time
from dataclasses import asdict, dataclass
from pathlib import Path
from typing import Any

# Disable XLA pool preallocation and cap device memory fraction before importing JAX.
os.environ.setdefault("XLA_PYTHON_CLIENT_PREALLOCATE", "false")
os.environ.setdefault("XLA_PYTHON_CLIENT_MEM_FRACTION", "0.50")

import jax.numpy as jnp
import numpy as np

from dataset import ReplayBuffer, load_dataset_file
from model import Params, init_params, load_weights, predict, save_weights
from train import TrainConfig, train_model

DEFAULT_ENGINE_MANIFEST = Path(__file__).resolve().parent.parent / "Cargo.toml"


@dataclass
class LoopConfig:
    """Configuration for multi-generation AlphaZero self-play and training."""

    work_dir: Path
    engine_manifest: Path = DEFAULT_ENGINE_MANIFEST
    selfplay_bin: Path | None = None
    generations: int = 5
    games_per_gen: int = 100
    datagen_sims: int = 200
    dirichlet_alpha: float = 0.3
    dirichlet_eps: float = 0.25
    temp_plies: int = 12
    augment_symmetries: bool = True
    max_replay_samples: int = 250_000
    val_fraction: float = 0.1
    epochs_per_gen: int = 3
    batch_size: int = 512
    lr: float = 3e-4
    weight_decay: float = 1e-3
    value_weight: float = 0.20
    arena_games: int = 60
    arena_sims: int = 400
    opening_plies: int = 4
    promotion_threshold: float = 0.52
    verify_tolerance: float = 1e-5
    seed: int = 42
    export_default_weights: Path | None = None


def resolve_selfplay_binary(
    engine_manifest: Path = DEFAULT_ENGINE_MANIFEST,
    explicit_bin: Path | None = None,
) -> Path:
    """Locate or build the release `selfplay` binary for `ai-crosshair`."""
    if explicit_bin is not None and explicit_bin.exists():
        return explicit_bin.resolve()

    manifest_path = engine_manifest.resolve()
    if manifest_path.is_dir():
        manifest_path = manifest_path / "Cargo.toml"

    repo_root = Path(__file__).resolve().parent.parent
    manifest_candidates = [
        manifest_path,
        repo_root / "Cargo.toml",
        repo_root / "ai-crosshair" / "Cargo.toml",
    ]
    binary_candidates = [m.parent / "target" / "release" / "selfplay" for m in manifest_candidates]

    for cand in binary_candidates:
        if cand.exists() and os.access(cand, os.X_OK):
            return cand.resolve()

    for manifest in manifest_candidates:
        if manifest.exists():
            env = os.environ.copy()
            env["PATH"] = f"{Path.home() / '.cargo/bin'}:{Path.home() / '.local/bin'}:{env.get('PATH', '')}"
            subprocess.run(
                [
                    "cargo",
                    "build",
                    "--release",
                    "--manifest-path",
                    str(manifest),
                    "--bin",
                    "selfplay",
                ],
                check=True,
                env=env,
            )
            built = manifest.parent / "target" / "release" / "selfplay"
            if built.exists() and os.access(built, os.X_OK):
                return built.resolve()

    raise FileNotFoundError(
        f"Could not find or build `selfplay` binary from manifest {engine_manifest}."
    )


def run_datagen(
    selfplay_bin: Path,
    out_path: Path,
    weights_path: Path,
    games: int,
    simulations: int,
    seed: int,
    dirichlet_alpha: float = 0.3,
    dirichlet_eps: float = 0.25,
    temp_plies: int = 12,
    augment_symmetries: bool = True,
) -> dict[str, Any]:
    """Run Rust `selfplay --datagen` to generate self-play trajectories."""
    out_path.parent.mkdir(parents=True, exist_ok=True)
    cmd = [
        str(selfplay_bin),
        "--datagen",
        "--games",
        str(games),
        "--simulations",
        str(simulations),
        "--out",
        str(out_path),
        "--weights",
        str(weights_path),
        "--seed",
        str(seed),
        "--dirichlet-alpha",
        str(dirichlet_alpha),
        "--dirichlet-eps",
        str(dirichlet_eps),
        "--temp-plies",
        str(temp_plies),
    ]
    if augment_symmetries:
        cmd.append("--augment-symmetries")

    t0 = time.perf_counter()
    proc = subprocess.run(cmd, check=True, capture_output=True, text=True)
    elapsed = time.perf_counter() - t0
    split = load_dataset_file(out_path)
    return {
        "games": games,
        "simulations": simulations,
        "num_samples": split.num_samples,
        "elapsed_sec": elapsed,
        "samples_per_sec": split.num_samples / max(elapsed, 1e-9),
        "stdout": proc.stdout.strip(),
    }


def reference_numpy_predict(params: Params, x: np.ndarray) -> tuple[np.ndarray, np.ndarray]:
    """Exact CPU float32 reference implementation matching the Rust forward pass."""
    x_f32 = np.asarray(x, dtype=np.float32)
    single = x_f32.ndim == 1
    if single:
        x_f32 = x_f32[None, :]

    w_stem = np.asarray(params["W_stem"], dtype=np.float32)
    b_stem = np.asarray(params["b_stem"], dtype=np.float32)
    w_res1a = np.asarray(params["W_res1a"], dtype=np.float32)
    b_res1a = np.asarray(params["b_res1a"], dtype=np.float32)
    w_res1b = np.asarray(params["W_res1b"], dtype=np.float32)
    b_res1b = np.asarray(params["b_res1b"], dtype=np.float32)
    w_res2a = np.asarray(params["W_res2a"], dtype=np.float32)
    b_res2a = np.asarray(params["b_res2a"], dtype=np.float32)
    w_res2b = np.asarray(params["W_res2b"], dtype=np.float32)
    b_res2b = np.asarray(params["b_res2b"], dtype=np.float32)
    w_pol = np.asarray(params["W_pol"], dtype=np.float32)
    b_pol = np.asarray(params["b_pol"], dtype=np.float32)
    w_val1 = np.asarray(params["W_val1"], dtype=np.float32)
    b_val1 = np.asarray(params["b_val1"], dtype=np.float32)
    w_val2 = np.asarray(params["W_val2"], dtype=np.float32)
    b_val2 = np.asarray(params["b_val2"], dtype=np.float32)

    h0 = np.maximum(0.0, x_f32 @ w_stem.T + b_stem)
    u1 = np.maximum(0.0, h0 @ w_res1a.T + b_res1a)
    h1 = np.maximum(0.0, h0 + (u1 @ w_res1b.T + b_res1b))
    u2 = np.maximum(0.0, h1 @ w_res2a.T + b_res2a)
    h2 = np.maximum(0.0, h1 + (u2 @ w_res2b.T + b_res2b))

    logits = h2 @ w_pol.T + b_pol
    legal = x_f32[:, 98:147] > 0.5
    masked_logits = np.where(legal, logits, -np.inf)
    max_logits = np.max(masked_logits, axis=-1, keepdims=True)
    max_logits = np.where(np.isfinite(max_logits), max_logits, 0.0)
    exps = np.where(legal, np.exp(masked_logits - max_logits), 0.0)
    sums = np.sum(exps, axis=-1, keepdims=True)
    probs = np.where(sums > 0.0, exps / np.maximum(sums, 1e-12), 0.0).astype(np.float32)

    v_hid = np.maximum(0.0, h2 @ w_val1.T + b_val1)
    value = np.tanh(v_hid @ w_val2.T + b_val2).squeeze(-1).astype(np.float32)

    if single:
        return probs[0], value[0]
    return probs, value


def _parse_verify_forward_json(stdout: str) -> tuple[np.ndarray, np.ndarray]:
    """Parse policy and value arrays from `selfplay --verify-forward` JSON output."""
    text = stdout.strip()
    try:
        payload = json.loads(text)
    except json.JSONDecodeError:
        # Try finding the last or multi-line JSON block
        lines = [ln.strip() for ln in text.splitlines() if ln.strip()]
        objs = []
        for ln in lines:
            if ln.startswith("{") or ln.startswith("["):
                try:
                    objs.append(json.loads(ln))
                except json.JSONDecodeError:
                    pass
        if not objs:
            raise ValueError(f"Could not parse JSON from --verify-forward output:\n{stdout}")
        payload = objs[0] if len(objs) == 1 else objs

    if isinstance(payload, dict):
        if "samples" in payload and isinstance(payload["samples"], list):
            items = payload["samples"]
        elif "results" in payload and isinstance(payload["results"], list):
            items = payload["results"]
        elif "outputs" in payload and isinstance(payload["outputs"], list):
            items = payload["outputs"]
        elif "policy" in payload and "value" in payload:
            pol = np.asarray(payload["policy"], dtype=np.float32)
            val = np.asarray(payload["value"], dtype=np.float32)
            if pol.ndim == 1:
                pol = pol[None, :]
                val = val.reshape(1)
            return pol, val.reshape(-1)
        else:
            raise ValueError(f"Unrecognized --verify-forward dict structure: {list(payload.keys())}")
    elif isinstance(payload, list):
        items = payload
    else:
        raise ValueError(f"Unexpected --verify-forward JSON type: {type(payload)}")

    policies = []
    values = []
    for item in items:
        p = item.get("policy", item.get("priors", item.get("probs")))
        v = item.get("value", item.get("v"))
        if p is None or v is None:
            raise ValueError(f"Missing policy/value keys in item: {item}")
        policies.append(p)
        values.append(v)

    return np.asarray(policies, dtype=np.float32), np.asarray(values, dtype=np.float32).reshape(-1)


def verify_forward_parity(
    selfplay_bin: Path,
    samples_path: Path,
    weights_path: Path,
    tolerance: float = 1e-5,
    count: int = 32,
) -> dict[str, Any]:
    """Run `selfplay --verify-forward` and assert max_abs_diff < tolerance against JAX on GPU."""
    cmd_variants = [
        [
            str(selfplay_bin),
            "--verify-forward",
            str(samples_path),
            str(weights_path),
            "--count",
            str(count),
        ],
        [str(selfplay_bin), "--verify-forward", str(samples_path), str(weights_path)],
    ]

    last_err = None
    stdout = ""
    for cmd in cmd_variants:
        proc = subprocess.run(cmd, capture_output=True, text=True)
        if proc.returncode == 0 and proc.stdout.strip():
            stdout = proc.stdout
            break
        last_err = proc.stderr or proc.stdout
    else:
        raise RuntimeError(f"`selfplay --verify-forward` failed: {last_err}")

    rust_probs, rust_values = _parse_verify_forward_json(stdout)
    k = rust_probs.shape[0]
    if k == 0:
        raise ValueError("`selfplay --verify-forward` returned 0 samples")

    split = load_dataset_file(samples_path)
    params = load_weights(weights_path)
    x_subset = jnp.asarray(split.inputs[:k], dtype=jnp.float32)
    jax_probs, jax_values = predict(params, x_subset)
    jax_probs_np = np.asarray(jax_probs, dtype=np.float32)
    jax_values_np = np.asarray(jax_values, dtype=np.float32).reshape(-1)

    max_policy_diff = float(np.max(np.abs(jax_probs_np - rust_probs)))
    max_value_diff = float(np.max(np.abs(jax_values_np - rust_values)))
    max_abs_diff = max(max_policy_diff, max_value_diff)

    if max_abs_diff >= tolerance:
        raise AssertionError(
            f"Forward parity check failed: max_abs_diff={max_abs_diff:.3e} >= {tolerance:.3e} "
            f"(policy={max_policy_diff:.3e}, value={max_value_diff:.3e})"
        )

    return {
        "num_verified": int(k),
        "max_policy_abs_diff": max_policy_diff,
        "max_value_abs_diff": max_value_diff,
        "max_abs_diff": max_abs_diff,
        "tolerance": tolerance,
        "passed": True,
    }


def _parse_arena_output(stdout: str) -> dict[str, Any]:
    """Parse candidate_wins, baseline_wins, and win_rate from `selfplay --arena` output."""
    text = stdout.strip()
    # Try JSON first (full stdout or last JSON line)
    for candidate_str in [text, *[ln.strip() for ln in reversed(text.splitlines()) if ln.strip()]]:
        if candidate_str.startswith("{") and candidate_str.endswith("}"):
            try:
                obj = json.loads(candidate_str)
                cand_wins = int(obj.get("candidate_wins", obj.get("candidateWins", 0)))
                base_wins = int(obj.get("baseline_wins", obj.get("baselineWins", 0)))
                draws = int(obj.get("draws", 0))
                total = cand_wins + base_wins + draws
                win_rate = float(
                    obj.get(
                        "win_rate",
                        obj.get("winRate", (cand_wins + 0.5 * draws) / max(total, 1)),
                    )
                )
                return {
                    "candidate_wins": cand_wins,
                    "baseline_wins": base_wins,
                    "draws": draws,
                    "win_rate": win_rate,
                    "raw": obj,
                }
            except json.JSONDecodeError:
                pass

    # Fallback regex extraction for key=value or key: value output
    cand_match = re.search(r"candidate_wins\s*[:=]\s*(\d+)", text, re.IGNORECASE)
    base_match = re.search(r"baseline_wins\s*[:=]\s*(\d+)", text, re.IGNORECASE)
    draw_match = re.search(r"draws\s*[:=]\s*(\d+)", text, re.IGNORECASE)
    rate_match = re.search(r"win_rate\s*[:=]\s*([0-9.]+)(%?)", text, re.IGNORECASE)

    if cand_match and base_match:
        cand_wins = int(cand_match.group(1))
        base_wins = int(base_match.group(1))
        draws = int(draw_match.group(1)) if draw_match else 0
        total = max(1, cand_wins + base_wins + draws)
        if rate_match:
            win_rate = float(rate_match.group(1))
            if rate_match.group(2) == "%" or win_rate > 1.0:
                win_rate /= 100.0
        else:
            win_rate = (cand_wins + 0.5 * draws) / total
        return {
            "candidate_wins": cand_wins,
            "baseline_wins": base_wins,
            "draws": draws,
            "win_rate": win_rate,
            "raw": text,
        }

    raise ValueError(f"Could not parse arena results from output:\n{stdout}")


def run_arena(
    selfplay_bin: Path,
    candidate: str | Path,
    baseline: str | Path,
    games: int,
    simulations: int,
    opening_plies: int = 4,
    seed: int = 42,
) -> dict[str, Any]:
    """Run paired-opening arena matches between candidate and baseline via `selfplay --arena`."""
    cmd = [
        str(selfplay_bin),
        "--arena",
        "--games",
        str(games),
        "--simulations",
        str(simulations),
        "--candidate",
        str(candidate),
        "--baseline",
        str(baseline),
        "--opening-plies",
        str(opening_plies),
        "--seed",
        str(seed),
    ]
    t0 = time.perf_counter()
    proc = subprocess.run(cmd, check=True, capture_output=True, text=True)
    elapsed = time.perf_counter() - t0
    parsed = _parse_arena_output(proc.stdout)
    parsed["elapsed_sec"] = elapsed
    parsed["candidate"] = str(candidate)
    parsed["baseline"] = str(baseline)
    parsed["games"] = games
    parsed["simulations"] = simulations
    return parsed


def run_training_loop(cfg: LoopConfig) -> dict[str, Any]:
    """Execute the full AlphaZero self-play, GPU training, verification, and arena loop."""
    work_dir = cfg.work_dir.resolve()
    data_dir = work_dir / "data"
    ckpt_dir = work_dir / "checkpoints"
    data_dir.mkdir(parents=True, exist_ok=True)
    ckpt_dir.mkdir(parents=True, exist_ok=True)

    selfplay_bin = resolve_selfplay_binary(cfg.engine_manifest, cfg.selfplay_bin)
    print(f"[LOOP] Using selfplay binary: {selfplay_bin}")

    best_weights_path = ckpt_dir / "best.bin"
    gen0_path = ckpt_dir / "gen_000.bin"
    if not best_weights_path.exists():
        init_p = init_params(seed=cfg.seed)
        save_weights(init_p, gen0_path)
        shutil.copy2(gen0_path, best_weights_path)
        print(
            f"[LOOP] Initialized fresh random gen_000 weights (seed={cfg.seed}) at {gen0_path} "
            f"({gen0_path.stat().st_size:,} bytes)"
        )

    buffer = ReplayBuffer(max_samples=cfg.max_replay_samples)
    existing_data = sorted(data_dir.glob("gen_*.bin"))
    if existing_data:
        buffer.load_files(existing_data)
        print(f"[LOOP] Preloaded {len(buffer):,} samples from {len(existing_data)} existing files")

    generation_logs: list[dict[str, Any]] = []

    for gen in range(1, cfg.generations + 1):
        gen_seed = cfg.seed + gen * 1009
        print(f"\n==================== GENERATION {gen}/{cfg.generations} ====================")

        # 1. Self-play data generation
        data_file = data_dir / f"gen_{gen:03d}.bin"
        print(
            f"[GEN {gen}] Generating {cfg.games_per_gen} self-play games "
            f"({cfg.datagen_sims} sims/move) -> {data_file.name}"
        )
        datagen_stats = run_datagen(
            selfplay_bin=selfplay_bin,
            out_path=data_file,
            weights_path=best_weights_path,
            games=cfg.games_per_gen,
            simulations=cfg.datagen_sims,
            seed=gen_seed,
            dirichlet_alpha=cfg.dirichlet_alpha,
            dirichlet_eps=cfg.dirichlet_eps,
            temp_plies=cfg.temp_plies,
            augment_symmetries=cfg.augment_symmetries,
        )
        buffer.add_file(data_file)
        print(
            f"[GEN {gen}] Collected {datagen_stats['num_samples']:,} samples in "
            f"{datagen_stats['elapsed_sec']:.2f}s ({datagen_stats['samples_per_sec']:,.0f} samples/s). "
            f"Replay buffer: {len(buffer):,}/{cfg.max_replay_samples:,} samples "
            f"({buffer.memory_bytes / (1024 * 1024):.1f} MiB)"
        )

        # 2. Train on GPU with JAX over the sliding-window replay buffer
        train_split, val_split = buffer.split_train_val(
            val_fraction=cfg.val_fraction,
            seed=cfg.seed,
        )
        current_params = load_weights(best_weights_path)
        train_cfg = TrainConfig(
            epochs=cfg.epochs_per_gen,
            batch_size=cfg.batch_size,
            lr=cfg.lr,
            weight_decay=cfg.weight_decay,
            value_weight=cfg.value_weight,
            seed=gen_seed,
            verbose=True,
        )
        trained_params, train_summary = train_model(
            current_params,
            train_split,
            val_split,
            config=train_cfg,
        )
        cand_weights_path = ckpt_dir / f"gen_{gen:03d}.bin"
        save_weights(trained_params, cand_weights_path)

        # 3. Verify JAX vs Rust forward pass numerical parity (< 1e-5)
        parity_stats = verify_forward_parity(
            selfplay_bin=selfplay_bin,
            samples_path=data_file,
            weights_path=cand_weights_path,
            tolerance=cfg.verify_tolerance,
        )
        print(
            f"[GEN {gen}] Forward parity verified on {parity_stats['num_verified']} states: "
            f"max_abs_diff={parity_stats['max_abs_diff']:.3e} "
            f"(policy={parity_stats['max_policy_abs_diff']:.3e}, "
            f"value={parity_stats['max_value_abs_diff']:.3e})"
        )

        # 4. Arena evaluation: candidate vs previous best, plus reference baselines
        arena_vs_prev = run_arena(
            selfplay_bin=selfplay_bin,
            candidate=cand_weights_path,
            baseline=best_weights_path,
            games=cfg.arena_games,
            simulations=cfg.arena_sims,
            opening_plies=cfg.opening_plies,
            seed=gen_seed + 1,
        )
        promoted = arena_vs_prev["win_rate"] >= cfg.promotion_threshold
        if promoted:
            shutil.copy2(cand_weights_path, best_weights_path)
            if cfg.export_default_weights is not None:
                cfg.export_default_weights.parent.mkdir(parents=True, exist_ok=True)
                shutil.copy2(cand_weights_path, cfg.export_default_weights)

        print(
            f"[GEN {gen}] Arena vs previous best: "
            f"{arena_vs_prev['candidate_wins']}-{arena_vs_prev['baseline_wins']} "
            f"(win_rate={arena_vs_prev['win_rate'] * 100:.1f}%) -> "
            f"{'PROMOTED' if promoted else 'REJECTED'}"
        )

        if gen == 1 and promoted:
            arena_vs_gen0 = arena_vs_prev
        else:
            arena_vs_gen0 = run_arena(
                selfplay_bin=selfplay_bin,
                candidate=best_weights_path,
                baseline=gen0_path,
                games=cfg.arena_games,
                simulations=cfg.arena_sims,
                opening_plies=cfg.opening_plies,
                seed=gen_seed + 4,
            )
            print(
                f"[GEN {gen}] Best vs initial gen_000: "
                f"{arena_vs_gen0['candidate_wins']}-{arena_vs_gen0['baseline_wins']} "
                f"(win_rate={arena_vs_gen0['win_rate'] * 100:.1f}%)"
            )

        arena_vs_random = run_arena(
            selfplay_bin=selfplay_bin,
            candidate=best_weights_path,
            baseline="random-uct",
            games=cfg.arena_games,
            simulations=cfg.arena_sims,
            opening_plies=cfg.opening_plies,
            seed=gen_seed + 2,
        )
        print(
            f"[GEN {gen}] Best vs random-uct:   "
            f"{arena_vs_random['candidate_wins']}-{arena_vs_random['baseline_wins']} "
            f"(win_rate={arena_vs_random['win_rate'] * 100:.1f}%)"
        )

        arena_vs_tactical = run_arena(
            selfplay_bin=selfplay_bin,
            candidate=best_weights_path,
            baseline="tactical-uct",
            games=cfg.arena_games,
            simulations=cfg.arena_sims,
            opening_plies=cfg.opening_plies,
            seed=gen_seed + 3,
        )
        print(
            f"[GEN {gen}] Best vs tactical-uct: "
            f"{arena_vs_tactical['candidate_wins']}-{arena_vs_tactical['baseline_wins']} "
            f"(win_rate={arena_vs_tactical['win_rate'] * 100:.1f}%)"
        )

        gen_record = {
            "generation": gen,
            "datagen": datagen_stats,
            "replay_buffer_samples": len(buffer),
            "train": train_summary,
            "verify_forward": parity_stats,
            "arena_vs_prev": arena_vs_prev,
            "arena_vs_gen0": arena_vs_gen0,
            "arena_vs_random_uct": arena_vs_random,
            "arena_vs_tactical_uct": arena_vs_tactical,
            "promoted": promoted,
            "candidate_weights": str(cand_weights_path),
            "best_weights": str(best_weights_path),
        }
        generation_logs.append(gen_record)

        log_path = work_dir / "training_log.json"
        log_path.write_text(json.dumps({"generations": generation_logs}, indent=2))

    return {
        "best_weights": str(best_weights_path),
        "generations": generation_logs,
    }


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Orchestrate Crosshair AlphaZero self-play and JAX GPU training.")
    parser.add_argument("--work-dir", type=Path, default=Path("runs/default"), help="Run output directory.")
    parser.add_argument(
        "--engine-manifest",
        type=Path,
        default=DEFAULT_ENGINE_MANIFEST,
        help="Path to the ai-crosshair Cargo.toml manifest.",
    )
    parser.add_argument("--selfplay-bin", type=Path, default=None, help="Explicit path to selfplay binary.")
    parser.add_argument("--generations", type=int, default=3, help="Number of self-play generations.")
    parser.add_argument("--games-per-gen", type=int, default=180, help="Self-play games per generation.")
    parser.add_argument("--datagen-sims", type=int, default=500, help="MCTS simulations per move during datagen.")
    parser.add_argument("--epochs", type=int, default=3, help="Training epochs per generation.")
    parser.add_argument("--batch-size", type=int, default=512, help="Training mini-batch size.")
    parser.add_argument("--lr", type=float, default=3e-4, help="Peak learning rate.")
    parser.add_argument("--weight-decay", type=float, default=1e-3, help="L2 weight decay.")
    parser.add_argument("--value-weight", type=float, default=0.20, help="Value MSE weight.")
    parser.add_argument("--arena-games", type=int, default=60, help="Paired games per arena match.")
    parser.add_argument("--arena-sims", type=int, default=400, help="Simulations per move in arena matches.")
    parser.add_argument("--promotion-threshold", type=float, default=0.52, help="Win rate threshold for promotion.")
    parser.add_argument("--export-default-weights", type=Path, default=None, help="Copy promoted weights here.")
    parser.add_argument("--seed", type=int, default=42, help="Base random seed.")
    args = parser.parse_args(argv)

    cfg = LoopConfig(
        work_dir=args.work_dir,
        engine_manifest=args.engine_manifest,
        selfplay_bin=args.selfplay_bin,
        generations=args.generations,
        games_per_gen=args.games_per_gen,
        datagen_sims=args.datagen_sims,
        epochs_per_gen=args.epochs,
        batch_size=args.batch_size,
        lr=args.lr,
        weight_decay=args.weight_decay,
        value_weight=args.value_weight,
        arena_games=args.arena_games,
        arena_sims=args.arena_sims,
        promotion_threshold=args.promotion_threshold,
        export_default_weights=args.export_default_weights,
        seed=args.seed,
    )
    run_training_loop(cfg)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
