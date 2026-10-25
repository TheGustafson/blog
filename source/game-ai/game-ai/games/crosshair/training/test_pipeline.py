"""Unit and GPU integration tests for the Crosshair JAX training pipeline."""

from __future__ import annotations

import json
import os
import struct
import tempfile
import time
import unittest
from pathlib import Path

# Disable XLA pool preallocation and cap device memory fraction before importing JAX.
os.environ.setdefault("XLA_PYTHON_CLIENT_PREALLOCATE", "false")
os.environ.setdefault("XLA_PYTHON_CLIENT_MEM_FRACTION", "0.50")

import jax
import jax.numpy as jnp
import numpy as np

from dataset import (
    DATA_HEADER_BYTES,
    DATA_HEADER_STRUCT,
    DATA_MAGIC,
    DATA_VERSION,
    SAMPLE_BYTES,
    ReplayBuffer,
    load_dataset_file,
    save_dataset_file,
)
from loop import (
    _parse_arena_output,
    _parse_verify_forward_json,
    reference_numpy_predict,
)
from model import (
    ACTIONS,
    FNV_MASK,
    FNV_OFFSET_BASIS,
    FNV_PRIME,
    HEADER_BYTES,
    HEADER_STRUCT,
    HIDDEN,
    INPUTS,
    MAGIC,
    PARAM_COUNT,
    PARAM_SPECS,
    PAYLOAD_BYTES,
    TOTAL_BYTES,
    VALUE_HIDDEN,
    VERSION,
    deserialize_weights,
    fnv1a_checksum,
    forward,
    init_params,
    load_weights,
    predict,
    save_weights,
    serialize_weights,
)
from train import TrainConfig, train_model


def _make_synthetic_games(
    num_games: int = 32,
    plies_per_game: int = 16,
    seed: int = 123,
) -> tuple[np.ndarray, np.ndarray, np.ndarray]:
    """Generate realistic synthetic Crosshair trajectories for testing."""
    rng = np.random.default_rng(seed)
    teacher_params = init_params(seed=999)

    total_samples = num_games * plies_per_game
    inputs = np.zeros((total_samples, INPUTS), dtype=np.float32)

    idx = 0
    for _ in range(num_games):
        for ply in range(plies_per_game):
            # Channel 0 & 1: sparse stones
            occ = rng.choice(49, size=min(ply, 20), replace=False) if ply > 0 else np.array([], dtype=int)
            my_stones = occ[::2]
            opp_stones = occ[1::2]
            inputs[idx, my_stones] = 1.0
            inputs[idx, 49 + opp_stones] = 1.0

            # Channel 2 (98..147): legal move mask (at least 5 legal moves)
            active_row = int(rng.integers(0, 7))
            active_col = int(rng.integers(0, 7))
            for r in range(7):
                for c in range(7):
                    cell = r * 7 + c
                    if cell not in occ and (r == active_row or c == active_col):
                        inputs[idx, 98 + cell] = 1.0
            if np.sum(inputs[idx, 98:147]) == 0:
                # Wildcard fallback
                for cell in range(49):
                    if cell not in occ:
                        inputs[idx, 98 + cell] = 1.0

            # Channel 3 (147..196): flip count / 4.0 on legal moves
            legal_cells = np.where(inputs[idx, 98:147] > 0.5)[0]
            inputs[idx, 147 + legal_cells] = rng.choice(
                [0.0, 0.25, 0.5], size=len(legal_cells), p=[0.7, 0.2, 0.1]
            ).astype(np.float32)

            # Channel 4 (196..245): last move
            if ply > 0:
                last_mv = int(occ[-1])
                inputs[idx, 196 + last_mv] = 1.0

            # Channel 5 (245..294): ply / 49.0
            inputs[idx, 245:294] = np.float32(ply / 49.0)
            idx += 1

    # Compute deterministic teacher policy & value targets so a student network can learn them cleanly
    legal_mask = inputs[:, 98:147] > 0.5
    logits_jax, val_jax = forward(
        teacher_params,
        jnp.asarray(inputs, dtype=jnp.float32),
        legal_mask=jnp.asarray(legal_mask),
    )
    # Sharpen teacher logits with low temperature to emulate MCTS root visit distributions
    sharp_probs_jax = jax.nn.softmax(logits_jax / 0.015, axis=-1)
    probs_np = np.asarray(sharp_probs_jax, dtype=np.float32)
    probs_np = np.where(legal_mask, probs_np, 0.0)
    probs_np /= np.maximum(np.sum(probs_np, axis=-1, keepdims=True), 1e-12)
    policy_targets = probs_np.astype(np.float32)

    val_np = np.asarray(val_jax, dtype=np.float32)
    value_targets = np.where(val_np >= 0.0, 1.0, -1.0).astype(np.float32)
    return inputs, policy_targets, value_targets


class TestCrosshairJaxPipeline(unittest.TestCase):
    """End-to-end test suite for GPU setup, CXHRNET1, CXHRDAT1, and JAX GPU training."""

    def test_01_gpu_device_and_jit_matmul(self) -> None:
        self.assertEqual(os.environ.get("XLA_PYTHON_CLIENT_PREALLOCATE"), "false")
        self.assertEqual(os.environ.get("XLA_PYTHON_CLIENT_MEM_FRACTION"), "0.50")

        devices = jax.devices()
        self.assertGreaterEqual(len(devices), 1)
        dev_str = str(devices[0]).lower()
        self.assertTrue("cuda" in dev_str or "gpu" in dev_str, f"Expected CUDA GPU device, got {devices}")

        @jax.jit
        def matmul_op(a: jax.Array, b: jax.Array) -> jax.Array:
            return jnp.matmul(a, b, precision=jax.lax.Precision.HIGHEST)

        key = jax.random.PRNGKey(0)
        a = jax.random.normal(key, (512, 512), dtype=jnp.float32)
        c = matmul_op(a, a.T).block_until_ready()
        self.assertEqual(c.shape, (512, 512))
        self.assertTrue(bool(jnp.all(jnp.isfinite(c))))

    def test_02_cxhrnet1_round_trip_and_checksum_speed(self) -> None:
        self.assertEqual(PARAM_COUNT, 114290)
        self.assertEqual(PAYLOAD_BYTES, 457160)
        self.assertEqual(TOTAL_BYTES, 457192)
        self.assertEqual(HEADER_BYTES, 32)

        # Verify fast FNV-1a matches pure-Python reference on arbitrary bytes
        sample_bytes = bytes(range(256)) * 64
        ref_hash = FNV_OFFSET_BASIS
        for b in sample_bytes:
            ref_hash = ((ref_hash ^ b) * FNV_PRIME) & FNV_MASK
        self.assertEqual(fnv1a_checksum(sample_bytes), ref_hash)

        params = init_params(seed=17)
        with tempfile.TemporaryDirectory() as tmpdir:
            w_path = Path(tmpdir) / "test_model.bin"

            t0 = time.perf_counter()
            save_weights(params, w_path)
            save_ms = (time.perf_counter() - t0) * 1000.0

            t1 = time.perf_counter()
            loaded = load_weights(w_path)
            load_ms = (time.perf_counter() - t1) * 1000.0

            self.assertLess(save_ms, 50.0, f"save_weights took {save_ms:.2f} ms (> 50 ms limit)")
            self.assertLess(load_ms, 50.0, f"load_weights took {load_ms:.2f} ms (> 50 ms limit)")

            raw = w_path.read_bytes()
            self.assertEqual(len(raw), TOTAL_BYTES)

            # Inspect exact 32-byte header fields
            magic, ver, inp, hid, act, vhid, pcount, csum = HEADER_STRUCT.unpack_from(raw, 0)
            self.assertEqual(magic, MAGIC)
            self.assertEqual(ver, VERSION)
            self.assertEqual(inp, INPUTS)
            self.assertEqual(hid, HIDDEN)
            self.assertEqual(act, ACTIONS)
            self.assertEqual(vhid, VALUE_HIDDEN)
            self.assertEqual(pcount, PARAM_COUNT)
            self.assertEqual(csum, fnv1a_checksum(raw[HEADER_BYTES:]))

            # Verify bit-for-bit equality of all 16 parameter tensors
            for name, shape in PARAM_SPECS:
                orig_np = np.asarray(params[name], dtype=np.float32)
                load_np = np.asarray(loaded[name], dtype=np.float32)
                self.assertEqual(orig_np.shape, shape)
                self.assertEqual(load_np.shape, shape)
                np.testing.assert_array_equal(orig_np, load_np)

            # Verify corrupted checksum is rejected
            corrupted = bytearray(raw)
            corrupted[-1] ^= 0x01
            with self.assertRaises(ValueError):
                deserialize_weights(corrupted)

            # Verify corrupted magic is rejected
            bad_magic = bytearray(raw)
            bad_magic[0:8] = b"BADMAGIC"
            with self.assertRaises(ValueError):
                deserialize_weights(bad_magic)

    def test_03_forward_pass_and_numerical_parity(self) -> None:
        params = init_params(seed=29)
        inputs, _, _ = _make_synthetic_games(num_games=4, plies_per_game=8, seed=31)

        # Single sample (1D) and batched (2D) forward pass
        logits_1d, val_1d = forward(params, jnp.asarray(inputs[0]))
        self.assertEqual(logits_1d.shape, (49,))
        self.assertEqual(val_1d.shape, ())

        probs_batch, val_batch = predict(params, jnp.asarray(inputs))
        probs_np = np.asarray(probs_batch, dtype=np.float32)
        val_np = np.asarray(val_batch, dtype=np.float32)
        self.assertEqual(probs_np.shape, (32, 49))
        self.assertEqual(val_np.shape, (32,))

        # Illegal moves must have exact 0.0 probability and legal moves must sum to 1.0
        legal_mask = inputs[:, 98:147] > 0.5
        self.assertTrue(np.all(probs_np[~legal_mask] == 0.0))
        np.testing.assert_allclose(np.sum(probs_np, axis=-1), np.ones(32, dtype=np.float32), atol=1e-6)
        self.assertTrue(np.all((val_np >= -1.0) & (val_np <= 1.0)))

        # Compare JAX GPU output against CPU float32 reference implementation
        ref_probs, ref_val = reference_numpy_predict(params, inputs)
        max_pol_diff = float(np.max(np.abs(probs_np - ref_probs)))
        max_val_diff = float(np.max(np.abs(val_np - ref_val)))
        self.assertLess(max_pol_diff, 1e-5, f"max_pol_diff={max_pol_diff}")
        self.assertLess(max_val_diff, 1e-5, f"max_val_diff={max_val_diff}")

    def test_04_cxhrdat1_dataset_and_replay_buffer(self) -> None:
        inputs1, pol1, val1 = _make_synthetic_games(num_games=10, plies_per_game=10, seed=101)
        inputs2, pol2, val2 = _make_synthetic_games(num_games=12, plies_per_game=10, seed=102)

        with tempfile.TemporaryDirectory() as tmpdir:
            p1 = Path(tmpdir) / "gen_001.bin"
            p2 = Path(tmpdir) / "gen_002.bin"
            save_dataset_file(p1, inputs1, pol1, val1)
            save_dataset_file(p2, inputs2, pol2, val2)

            # Verify exact CXHRDAT1 binary layout
            raw1 = p1.read_bytes()
            self.assertEqual(len(raw1), DATA_HEADER_BYTES + 100 * SAMPLE_BYTES)
            magic, ver, count = DATA_HEADER_STRUCT.unpack_from(raw1, 0)
            self.assertEqual(magic, DATA_MAGIC)
            self.assertEqual(ver, DATA_VERSION)
            self.assertEqual(count, 100)

            split1 = load_dataset_file(p1)
            self.assertEqual(split1.num_samples, 100)
            np.testing.assert_array_equal(split1.inputs, inputs1)
            np.testing.assert_array_equal(split1.policy_targets, pol1)
            np.testing.assert_array_equal(split1.value_targets, val1)

            # Test sliding window replay buffer with capacity 150 (total added = 220)
            buf = ReplayBuffer(max_samples=150)
            buf.load_files([p1, p2])
            self.assertEqual(len(buf), 150)
            combined_split = buf.as_split()
            np.testing.assert_array_equal(combined_split.inputs[:30], inputs1[-30:])
            np.testing.assert_array_equal(combined_split.inputs[30:], inputs2)

            # Test game-level train/val split
            train_split, val_split = buf.split_train_val(val_fraction=0.2, seed=42)
            self.assertEqual(train_split.num_samples + val_split.num_samples, 150)
            self.assertGreater(train_split.num_samples, 0)
            self.assertGreater(val_split.num_samples, 0)

    def test_05_real_gpu_training_loss_reduction_and_throughput(self) -> None:
        inputs, pol_targets, val_targets = _make_synthetic_games(
            num_games=128,
            plies_per_game=16,
            seed=2026,
        )
        buf = ReplayBuffer(max_samples=10_000)
        buf.add_samples(inputs, pol_targets, val_targets)
        train_split, val_split = buf.split_train_val(val_fraction=0.15, seed=7)

        init_p = init_params(seed=11)
        cfg = TrainConfig(
            epochs=12,
            batch_size=256,
            lr=3e-3,
            weight_decay=1e-4,
            value_weight=1.0,
            seed=11,
            verbose=True,
        )
        trained_p, summary = train_model(init_p, train_split, val_split, config=cfg)

        pre_tr = summary["pre_train"]
        post_tr = summary["post_train"]
        pre_va = summary["pre_val"]
        post_va = summary["post_val"]

        # Verify training and validation losses decrease substantially
        self.assertLess(post_tr["total_loss"], pre_tr["total_loss"] * 0.75)
        self.assertLess(post_tr["policy_loss"], pre_tr["policy_loss"] * 0.85)
        self.assertLess(post_tr["value_mse"], pre_tr["value_mse"] * 0.65)
        self.assertGreater(post_tr["top1_acc"], pre_tr["top1_acc"] + 0.15)
        self.assertGreater(post_tr["value_sign_acc"], pre_tr["value_sign_acc"] + 0.15)

        self.assertLess(post_va["total_loss"], pre_va["total_loss"])
        self.assertGreater(summary["samples_per_sec"], 1_000.0)

        # Ensure trained weights serialize cleanly to CXHRNET1
        blob = serialize_weights(trained_p)
        self.assertEqual(len(blob), TOTAL_BYTES)

    def test_06_loop_parsers(self) -> None:
        json_out = json.dumps(
            {
                "samples": [
                    {"policy": [1.0 / 49.0] * 49, "value": 0.25},
                    {"policy": [1.0 / 49.0] * 49, "value": -0.5},
                ]
            }
        )
        pol, val = _parse_verify_forward_json(json_out)
        self.assertEqual(pol.shape, (2, 49))
        np.testing.assert_allclose(val, np.array([0.25, -0.5], dtype=np.float32))

        arena_json = '{"candidate_wins": 26, "baseline_wins": 14, "win_rate": 0.65}'
        parsed = _parse_arena_output(arena_json)
        self.assertEqual(parsed["candidate_wins"], 26)
        self.assertEqual(parsed["baseline_wins"], 14)
        self.assertAlmostEqual(parsed["win_rate"], 0.65)

        arena_kv = "Arena finished: candidate_wins=28 baseline_wins=12 win_rate=70.0%"
        parsed_kv = _parse_arena_output(arena_kv)
        self.assertEqual(parsed_kv["candidate_wins"], 28)
        self.assertEqual(parsed_kv["baseline_wins"], 12)
        self.assertAlmostEqual(parsed_kv["win_rate"], 0.70)


if __name__ == "__main__":
    unittest.main(verbosity=2)
