"""JAX GPU trainer for the Crosshair policy-value network."""

from __future__ import annotations

import argparse
import json
import os
import time
from dataclasses import asdict, dataclass
from pathlib import Path
from typing import Any, NamedTuple

# Disable XLA pool preallocation and cap device memory fraction before importing JAX.
os.environ.setdefault("XLA_PYTHON_CLIENT_PREALLOCATE", "false")
os.environ.setdefault("XLA_PYTHON_CLIENT_MEM_FRACTION", "0.50")

import jax
import jax.numpy as jnp
import numpy as np

from dataset import DatasetSplit, ReplayBuffer
from model import ACTIONS, INPUTS, Params, forward, init_params, load_weights, save_weights


class OptimizerState(NamedTuple):
    """Pure-JAX Adam optimizer state."""

    step: jax.Array
    m: Params
    v: Params


@dataclass
class EvalMetrics:
    """Aggregated evaluation metrics over a dataset split."""

    num_samples: int
    total_loss: float
    policy_loss: float
    top1_acc: float
    value_mse: float
    value_sign_acc: float
    target_entropy: float


@dataclass
class TrainConfig:
    """Hyperparameters for JAX GPU training."""

    epochs: int = 8
    batch_size: int = 512
    lr: float = 1.4e-3
    lr_min_ratio: float = 0.05
    weight_decay: float = 1.5e-4
    value_weight: float = 1.0
    beta1: float = 0.9
    beta2: float = 0.999
    eps: float = 1e-8
    max_grad_norm: float = 1.0
    seed: int = 42
    verbose: bool = True


def init_optimizer(params: Params) -> OptimizerState:
    """Initialize zero first/second moments for Adam on the same device as params."""
    m = {k: jnp.zeros_like(v) for k, v in params.items()}
    v = {k: jnp.zeros_like(v) for k, v in params.items()}
    return OptimizerState(step=jnp.array(0, dtype=jnp.int32), m=m, v=v)


def _l2_regularization(params: Params) -> jax.Array:
    """Compute sum of squared weights (excluding biases)."""
    terms = []
    for name, p in params.items():
        if name.startswith("W_"):
            terms.append(jnp.sum(jnp.square(p)))
    return jnp.sum(jnp.stack(terms))


def _subsample_split(train_split: DatasetSplit, max_samples: int = 32_768) -> tuple[np.ndarray, np.ndarray, np.ndarray]:
    """Return a deterministic strided subsample of (inputs, policy_targets, value_targets) in CPU RAM."""
    n = train_split.num_samples
    if n <= max_samples:
        return train_split.inputs, train_split.policy_targets, train_split.value_targets
    idx = np.linspace(0, n - 1, max_samples, dtype=np.int64)
    return (
        train_split.inputs[idx],
        train_split.policy_targets[idx],
        train_split.value_targets[idx],
    )


def _prepare_policy_trunk(params: Params, train_split: DatasetSplit) -> Params:
    """Warm-start the 96-unit nonlinear policy trunk (`0..96`) and reserve `96..128` for value without dynamic-shape XLA compilation."""
    n = train_split.num_samples
    if n < 4096:
        return params

    x_np, pi_np, _ = _subsample_split(train_split, max_samples=32_768)
    sub_n = x_np.shape[0]
    legal_np = x_np[:, 98:147] > 0.5

    log_pi = np.where(legal_np, np.log(np.maximum(pi_np, 1e-4)), 0.0).astype(np.float32)
    legal_cnt = np.maximum(np.sum(legal_np.astype(np.float32), axis=1, keepdims=True), 1.0)
    log_pi = np.where(legal_np, log_pi - np.sum(log_pi, axis=1, keepdims=True) / legal_cnt, 0.0)

    xtx = (x_np.T @ x_np).astype(np.float64)
    xty = (x_np.T @ log_pi).astype(np.float64)
    reg_p = max(2500.0, 0.04 * float(sub_n))
    w_pol_np = np.linalg.solve(
        xtx + reg_p * np.eye(INPUTS, dtype=np.float64),
        xty,
    ).T.astype(np.float32)

    fresh_p = {k: np.array(v, dtype=np.float32, copy=True) for k, v in init_params(seed=42).items()}
    prev_p = {k: np.array(v, dtype=np.float32, copy=True) for k, v in params.items()}
    is_fresh = float(np.max(np.abs(prev_p["W_res1b"]))) < 1e-6

    new_p = dict(fresh_p)
    new_p["W_stem"][:49, :] = 0.5 * fresh_p["W_stem"][:49, :] + w_pol_np
    new_p["b_stem"][:49] = 1.2
    new_p["W_pol"][:, :49] = (
        0.5 * fresh_p["W_pol"][:, :49] + np.eye(ACTIONS, dtype=np.float32) * 0.8
    )
    if not is_fresh:
        new_p["W_stem"][49:96, :] = 0.75 * fresh_p["W_stem"][49:96, :] + 0.25 * prev_p["W_stem"][49:96, :]
        new_p["b_stem"][49:96] = 0.75 * fresh_p["b_stem"][49:96] + 0.25 * prev_p["b_stem"][49:96]
        for k in ("W_res1a", "b_res1a", "W_res1b", "b_res1b", "W_res2a", "b_res2a", "W_res2b", "b_res2b"):
            new_p[k] = 0.75 * fresh_p[k] + 0.25 * prev_p[k]

    # Reserve units 96..128 for the invariant value & road-corridor subspace
    new_p["W_stem"][96:, :] = 0.0
    new_p["b_stem"][96:] = 0.0
    new_p["W_res1a"][96:, :] = 0.0
    new_p["W_res1a"][:, 96:] = 0.0
    new_p["b_res1a"][96:] = 0.0
    new_p["W_res1b"][96:, :] = 0.0
    new_p["W_res1b"][:, 96:] = 0.0
    new_p["b_res1b"][96:] = 0.0
    new_p["W_res2a"][96:, :] = 0.0
    new_p["W_res2a"][:, 96:] = 0.0
    new_p["b_res2a"][96:] = 0.0
    new_p["W_res2b"][96:, :] = 0.0
    new_p["W_res2b"][:, 96:] = 0.0
    new_p["b_res2b"][96:] = 0.0
    new_p["W_pol"][:, 96:] = 0.0

    return {k: jnp.asarray(v, dtype=jnp.float32) for k, v in new_p.items()}


def _finalize_hybrid_heads(
    params: Params,
    train_split: DatasetSplit,
    target_logit_std: float = 0.52,
    val_scale: float = 1.15,
) -> Params:
    """Attach the V4-symmetric road-corridor & span value subspace (`96..128`) and calibrate PUCT policy temperature."""
    n = train_split.num_samples
    if n < 4096:
        return params

    x_np, _, z_np = _subsample_split(train_split, max_samples=32_768)
    sub_n = x_np.shape[0]
    xtx = (x_np.T @ x_np).astype(np.float64)
    xtz = (x_np.T @ z_np).astype(np.float64)
    reg_v = max(3500.0, 0.06 * float(sub_n))
    w_val = np.linalg.solve(
        xtx + reg_v * np.eye(INPUTS, dtype=np.float64),
        xtz,
    ).astype(np.float32)

    # Symmetrize w_val under Klein four-group V4 reflections
    for ch in range(6):
        g = w_val[ch * 49 : (ch + 1) * 49].reshape(7, 7)
        w_val[ch * 49 : (ch + 1) * 49] = (
            0.25 * (g + g[::-1, :] + g[:, ::-1] + g[::-1, ::-1])
        ).reshape(49)

    # Pooled flip capture regression bonus on Channel 3 (indices 147..196)
    flips_np = x_np[:, 147:196]
    f_sum = np.sum(flips_np, axis=1)
    resid_val = z_np - (x_np @ w_val)
    beta_flip_val = float(np.sum(f_sum * resid_val) / (np.sum(f_sum * f_sum) + 200.0))
    w_val[147:196] += beta_flip_val

    new_p = {k: np.array(v, dtype=np.float32, copy=True) for k, v in params.items()}

    # Ensure units 96..128 bypass residual blocks and policy head cleanly
    new_p["W_stem"][96:, :] = 0.0
    new_p["b_stem"][96:] = 0.0
    new_p["W_res1a"][96:, :] = 0.0
    new_p["W_res1a"][:, 96:] = 0.0
    new_p["b_res1a"][96:] = 0.0
    new_p["W_res1b"][96:, :] = 0.0
    new_p["W_res1b"][:, 96:] = 0.0
    new_p["b_res1b"][96:] = 0.0
    new_p["W_res2a"][96:, :] = 0.0
    new_p["W_res2a"][:, 96:] = 0.0
    new_p["b_res2a"][96:] = 0.0
    new_p["W_res2b"][96:, :] = 0.0
    new_p["W_res2b"][:, 96:] = 0.0
    new_p["b_res2b"][96:] = 0.0
    new_p["W_pol"][:, 96:] = 0.0

    # Units 96..103 (7 units): own empty-rank detectors relu(1.0 - sum_c own[r, c])
    for r in range(7):
        new_p["W_stem"][96 + r, r * 7 : (r + 1) * 7] = -1.0
        new_p["b_stem"][96 + r] = 1.0

    # Units 103..110 (7 units): opp empty-file detectors relu(1.0 - sum_r opp[r, c])
    for c in range(7):
        for r in range(7):
            new_p["W_stem"][103 + c, 49 + r * 7 + c] = -1.0
        new_p["b_stem"][103 + c] = 1.0

    # Units 110..117 (7 units): own file-corridor vertical road continuity detectors
    for c in range(7):
        u = 110 + c
        for r in range(7):
            sq = r * 7 + c
            new_p["W_stem"][u, sq] = 0.50
            new_p["W_stem"][u, 49 + sq] = -0.35
            if c > 0:
                new_p["W_stem"][u, r * 7 + (c - 1)] += 0.20
            if c < 6:
                new_p["W_stem"][u, r * 7 + (c + 1)] += 0.20
        new_p["b_stem"][u] = -0.75

    # Units 117..124 (7 units): opp rank-corridor horizontal road continuity detectors
    for r in range(7):
        u = 117 + r
        for c in range(7):
            sq = r * 7 + c
            new_p["W_stem"][u, 49 + sq] = 0.50
            new_p["W_stem"][u, sq] = -0.35
            if r > 0:
                new_p["W_stem"][u, 49 + (r - 1) * 7 + c] += 0.20
            if r < 6:
                new_p["W_stem"][u, 49 + (r + 1) * 7 + c] += 0.20
        new_p["b_stem"][u] = -0.75

    # Units 124, 125: center 3x3 control & edge anchor advantage (+ and -)
    for r in range(2, 5):
        for c in range(2, 5):
            sq = r * 7 + c
            new_p["W_stem"][124, sq] = 0.25
            new_p["W_stem"][124, 49 + sq] = -0.25
            new_p["W_stem"][125, 49 + sq] = 0.25
            new_p["W_stem"][125, sq] = -0.25

    # Units 126, 127: V4-symmetric Ridge + pooled-flip linear value (+ and -)
    new_p["W_stem"][126, :] = w_val * 2.2
    new_p["W_stem"][127, :] = -w_val * 2.2

    h0 = np.maximum(0.0, x_np @ new_p["W_stem"].T + new_p["b_stem"])
    u1 = np.maximum(0.0, h0 @ new_p["W_res1a"].T + new_p["b_res1a"])
    h1 = np.maximum(0.0, h0 + u1 @ new_p["W_res1b"].T + new_p["b_res1b"])
    u2 = np.maximum(0.0, h1 @ new_p["W_res2a"].T + new_p["b_res2a"])
    h2 = np.maximum(0.0, h1 + u2 @ new_p["W_res2b"].T + new_p["b_res2b"])

    # Calibrate policy head temperature to target_logit_std across legal moves
    sample_k = min(sub_n, 4096)
    logits_sub = h2[:sample_k] @ new_p["W_pol"].T + new_p["b_pol"]
    leg_sub = (x_np[:sample_k, 98:147] > 0.5).astype(np.float32)
    leg_cnt = np.maximum(np.sum(leg_sub, axis=1, keepdims=True), 1.0)
    leg_mean = np.sum(logits_sub * leg_sub, axis=1, keepdims=True) / leg_cnt
    leg_var = np.sum(np.square((logits_sub - leg_mean) * leg_sub), axis=1) / np.squeeze(leg_cnt, axis=1)
    curr_std = float(np.mean(np.sqrt(np.maximum(leg_var, 1e-8))))
    if curr_std > 1e-4:
        pol_scale = float(np.clip(target_logit_std / curr_std, 0.65, 1.85))
        new_p["W_pol"] *= pol_scale
        new_p["b_pol"] *= pol_scale

    # Fit regularized value readout from the 96-unit nonlinear policy trunk + dedicated span/corridor units
    h_trunk = h2[:, :96]
    resid_z = z_np - 0.45 * (h2[:, 126] - h2[:, 127])
    hth = (h_trunk.T @ h_trunk).astype(np.float64)
    htr = (h_trunk.T @ resid_z).astype(np.float64)
    w_trunk_val = np.linalg.solve(
        hth + max(10000.0, 0.16 * float(sub_n)) * np.eye(96, dtype=np.float64),
        htr,
    ).astype(np.float32)

    new_p["W_val1"][:, :] = 0.0
    new_p["b_val1"][:] = 0.0
    new_p["W_val2"][:, :] = 0.0
    new_p["b_val2"][:] = 0.0

    new_p["W_val1"][0, 126] = 1.0
    new_p["W_val1"][1, 127] = 1.0
    new_p["W_val1"][2, 96:103] = 1.0
    new_p["W_val1"][3, 103:110] = 1.0
    new_p["W_val1"][4, 96:103] = -1.0
    new_p["b_val1"][4] = 1.0
    new_p["W_val1"][5, 103:110] = -1.0
    new_p["b_val1"][5] = 1.0
    new_p["W_val1"][6, 96:103] = -1.0
    new_p["b_val1"][6] = 2.0
    new_p["W_val1"][7, 103:110] = -1.0
    new_p["b_val1"][7] = 2.0
    new_p["W_val1"][8, 96:103] = -1.0
    new_p["b_val1"][8] = 3.0
    new_p["W_val1"][9, 103:110] = -1.0
    new_p["b_val1"][9] = 3.0
    new_p["W_val1"][10, [96, 102]] = -1.0
    new_p["b_val1"][10] = 1.0
    new_p["W_val1"][11, [103, 109]] = -1.0
    new_p["b_val1"][11] = 1.0
    new_p["W_val1"][12, 97:102] = -1.0
    new_p["b_val1"][12] = 1.0
    new_p["W_val1"][13, 104:109] = -1.0
    new_p["b_val1"][13] = 1.0
    new_p["W_val1"][14, 110:117] = 0.25
    new_p["W_val1"][15, 117:124] = 0.25
    new_p["W_val1"][16, :96] = w_trunk_val * 2.0
    new_p["W_val1"][17, :96] = -w_trunk_val * 2.0
    new_p["W_val1"][18, 124] = 1.0
    new_p["W_val1"][19, 125] = 1.0

    v_hid = np.maximum(0.0, h2 @ new_p["W_val1"].T + new_p["b_val1"])[:, :20]
    vtv = (v_hid.T @ v_hid).astype(np.float64)
    vtz = (v_hid.T @ z_np).astype(np.float64)
    w_v2 = np.linalg.solve(
        vtv + max(2500.0, 0.05 * float(sub_n)) * np.eye(20, dtype=np.float64),
        vtz,
    ).astype(np.float32)
    new_p["W_val2"][0, :20] = w_v2 * val_scale

    return {k: jnp.asarray(v, dtype=jnp.float32) for k, v in new_p.items()}


@jax.jit
def eval_batch(
    params: Params,
    x: jax.Array,
    pi_target: jax.Array,
    z_target: jax.Array,
    sample_mask: jax.Array,
    value_weight: float = 1.0,
    weight_decay: float = 1e-4,
) -> dict[str, jax.Array]:
    """Evaluate losses and accuracy metrics on a fixed-shape mini-batch with a validity mask."""
    legal_mask = x[:, 98:147] > 0.5
    logits, v_pred = forward(params, x, legal_mask=legal_mask)
    log_probs = jax.nn.log_softmax(logits, axis=-1)

    pi_masked = jnp.where(legal_mask, pi_target, 0.0)
    pi_norm = jnp.sum(pi_masked, axis=-1, keepdims=True)
    pi_masked = jnp.where(pi_norm > 0.0, pi_masked / jnp.maximum(pi_norm, 1e-12), pi_masked)

    weights = sample_mask.astype(jnp.float32)
    denom = jnp.maximum(jnp.sum(weights), 1.0)

    per_sample_pol = -jnp.sum(pi_masked * log_probs, axis=-1)
    per_sample_ent = -jnp.sum(
        jnp.where(pi_masked > 1e-12, pi_masked * jnp.log(jnp.maximum(pi_masked, 1e-12)), 0.0),
        axis=-1,
    )
    per_sample_val = jnp.square(v_pred - z_target)

    policy_loss = jnp.sum(per_sample_pol * weights) / denom
    target_entropy = jnp.sum(per_sample_ent * weights) / denom
    value_mse = jnp.sum(per_sample_val * weights) / denom
    l2_reg = 0.5 * weight_decay * _l2_regularization(params)
    total_loss = policy_loss + value_weight * value_mse + l2_reg

    pred_top1 = jnp.argmax(logits, axis=-1)
    target_top1 = jnp.argmax(pi_masked, axis=-1)
    top1_acc = jnp.sum((pred_top1 == target_top1).astype(jnp.float32) * weights) / denom
    value_sign_acc = jnp.sum(((v_pred * z_target) > 0.0).astype(jnp.float32) * weights) / denom

    return {
        "total_loss": total_loss,
        "policy_loss": policy_loss,
        "value_mse": value_mse,
        "l2_reg": l2_reg,
        "top1_acc": top1_acc,
        "value_sign_acc": value_sign_acc,
        "target_entropy": target_entropy,
    }


def _cosine_lr(
    step: jax.Array,
    total_steps: jax.Array,
    lr_max: jax.Array,
    lr_min_ratio: jax.Array,
) -> jax.Array:
    """Cosine learning rate schedule with a short linear warmup."""
    total = jnp.maximum(total_steps.astype(jnp.float32), 1.0)
    warmup = jnp.minimum(25.0, jnp.floor(total * 0.1))
    s = step.astype(jnp.float32)

    warmup_lr = lr_max * ((s + 1.0) / jnp.maximum(warmup, 1.0))
    progress = jnp.clip((s - warmup) / jnp.maximum(total - warmup, 1.0), 0.0, 1.0)
    lr_min = lr_max * lr_min_ratio
    cosine_lr = lr_min + 0.5 * (lr_max - lr_min) * (1.0 + jnp.cos(jnp.pi * progress))
    return jnp.where(s < warmup, warmup_lr, cosine_lr)


@jax.jit
def train_step(
    params: Params,
    opt_state: OptimizerState,
    x: jax.Array,
    pi_target: jax.Array,
    z_target: jax.Array,
    sample_mask: jax.Array,
    total_steps: jax.Array,
    lr_max: jax.Array,
    lr_min_ratio: jax.Array,
    value_weight: jax.Array,
    weight_decay: jax.Array,
    beta1: jax.Array,
    beta2: jax.Array,
    eps: jax.Array,
    max_grad_norm: jax.Array,
) -> tuple[Params, OptimizerState, dict[str, jax.Array]]:
    """Perform one JIT-compiled forward, backward, and AdamW/L2 update step on the GPU."""

    def loss_fn(p: Params) -> tuple[jax.Array, dict[str, jax.Array]]:
        metrics = eval_batch(
            p,
            x,
            pi_target,
            z_target,
            sample_mask,
            value_weight=value_weight,
            weight_decay=weight_decay,
        )
        return metrics["total_loss"], metrics

    (_, metrics), grads = jax.value_and_grad(loss_fn, has_aux=True)(params)

    # Global norm gradient clipping
    sq_norms = [jnp.sum(jnp.square(g)) for g in grads.values()]
    grad_norm = jnp.sqrt(jnp.sum(jnp.stack(sq_norms)) + 1e-12)
    clip_scale = jnp.minimum(1.0, max_grad_norm / grad_norm)
    grads = {k: g * clip_scale for k, g in grads.items()}

    new_step = opt_state.step + 1
    lr_t = _cosine_lr(opt_state.step, total_steps, lr_max, lr_min_ratio)

    t_float = new_step.astype(jnp.float32)
    b1_corr = 1.0 - jnp.power(beta1, t_float)
    b2_corr = 1.0 - jnp.power(beta2, t_float)

    new_m: Params = {}
    new_v: Params = {}
    new_params: Params = {}

    for k in params:
        g = grads[k]
        m_k = beta1 * opt_state.m[k] + (1.0 - beta1) * g
        v_k = beta2 * opt_state.v[k] + (1.0 - beta2) * jnp.square(g)
        m_hat = m_k / b1_corr
        v_hat = v_k / b2_corr
        new_m[k] = m_k
        new_v[k] = v_k
        new_params[k] = params[k] - lr_t * m_hat / (jnp.sqrt(v_hat) + eps)

    metrics = dict(metrics)
    metrics["lr"] = lr_t
    metrics["grad_norm"] = grad_norm
    return new_params, OptimizerState(step=new_step, m=new_m, v=new_v), metrics


def _pad_batch(
    x_np: np.ndarray,
    pi_np: np.ndarray,
    z_np: np.ndarray,
    batch_size: int,
) -> tuple[np.ndarray, np.ndarray, np.ndarray, np.ndarray, int]:
    """Pad a partial mini-batch to fixed `batch_size` so XLA never recompiles for tail shapes."""
    b = int(x_np.shape[0])
    if b == batch_size:
        mask = np.ones((batch_size,), dtype=np.float32)
        return x_np, pi_np, z_np, mask, b

    pad = batch_size - b
    x_pad = np.pad(x_np, ((0, pad), (0, 0)), mode="edge")
    pi_pad = np.pad(pi_np, ((0, pad), (0, 0)), mode="edge")
    z_pad = np.pad(z_np, (0, pad), mode="edge")
    mask = np.zeros((batch_size,), dtype=np.float32)
    mask[:b] = 1.0
    return x_pad, pi_pad, z_pad, mask, b


def evaluate_split(
    params: Params,
    split: DatasetSplit,
    batch_size: int = 512,
    value_weight: float = 1.0,
    weight_decay: float = 1e-4,
) -> EvalMetrics:
    """Evaluate a DatasetSplit in fixed-shape mini-batches on the GPU."""
    n = split.num_samples
    if n == 0:
        return EvalMetrics(
            num_samples=0,
            total_loss=0.0,
            policy_loss=0.0,
            top1_acc=0.0,
            value_mse=0.0,
            value_sign_acc=0.0,
            target_entropy=0.0,
        )

    totals = {
        "total_loss": 0.0,
        "policy_loss": 0.0,
        "top1_acc": 0.0,
        "value_mse": 0.0,
        "value_sign_acc": 0.0,
        "target_entropy": 0.0,
    }
    seen = 0
    vw = jnp.asarray(value_weight, dtype=jnp.float32)
    wd = jnp.asarray(weight_decay, dtype=jnp.float32)

    for x_np, pi_np, z_np in split.iter_batches(batch_size=batch_size, shuffle=False):
        x_p, pi_p, z_p, mask_p, b = _pad_batch(x_np, pi_np, z_np, batch_size)
        m = eval_batch(
            params,
            jnp.asarray(x_p, dtype=jnp.float32),
            jnp.asarray(pi_p, dtype=jnp.float32),
            jnp.asarray(z_p, dtype=jnp.float32),
            jnp.asarray(mask_p, dtype=jnp.float32),
            value_weight=vw,
            weight_decay=wd,
        )
        for k in totals:
            totals[k] += float(m[k]) * b
        seen += b

    return EvalMetrics(
        num_samples=seen,
        total_loss=totals["total_loss"] / seen,
        policy_loss=totals["policy_loss"] / seen,
        top1_acc=totals["top1_acc"] / seen,
        value_mse=totals["value_mse"] / seen,
        value_sign_acc=totals["value_sign_acc"] / seen,
        target_entropy=totals["target_entropy"] / seen,
    )


def format_metrics(label: str, m: EvalMetrics) -> str:
    """Format EvalMetrics into a single readable log line."""
    if m.num_samples == 0:
        return f"[{label}] (0 samples)"
    return (
        f"[{label}] n={m.num_samples:,} | "
        f"total={m.total_loss:.4f} | "
        f"policy_ce={m.policy_loss:.4f} (H={m.target_entropy:.4f}) | "
        f"top1={m.top1_acc * 100:.2f}% | "
        f"value_mse={m.value_mse:.4f} | "
        f"sign_acc={m.value_sign_acc * 100:.2f}%"
    )


def train_model(
    params: Params,
    train_split: DatasetSplit,
    val_split: DatasetSplit | None = None,
    config: TrainConfig | None = None,
) -> tuple[Params, dict[str, Any]]:
    """Train the policy-value network on the GPU and return updated params + summary."""
    cfg = config or TrainConfig()
    if train_split.num_samples == 0:
        raise ValueError("Cannot train on an empty train_split")

    val = val_split if val_split is not None else DatasetSplit(
        inputs=np.empty((0, INPUTS), dtype=np.float32),
        policy_targets=np.empty((0, ACTIONS), dtype=np.float32),
        value_targets=np.empty((0,), dtype=np.float32),
    )

    pre_train = evaluate_split(
        params,
        train_split,
        batch_size=cfg.batch_size,
        value_weight=cfg.value_weight,
        weight_decay=cfg.weight_decay,
    )
    pre_val = evaluate_split(
        params,
        val,
        batch_size=cfg.batch_size,
        value_weight=cfg.value_weight,
        weight_decay=cfg.weight_decay,
    )

    if cfg.verbose:
        print(format_metrics("PRE-TRAIN (train)", pre_train))
        if val.num_samples > 0:
            print(format_metrics("PRE-TRAIN (val)  ", pre_val))

    use_hybrid = train_split.num_samples >= 4096
    params = _prepare_policy_trunk(params, train_split)

    steps_per_epoch = max(1, (train_split.num_samples + cfg.batch_size - 1) // cfg.batch_size)
    total_steps = max(1, cfg.epochs * steps_per_epoch)
    opt_state = init_optimizer(params)
    rng = np.random.default_rng(cfg.seed)

    # Pre-convert scalar hyperparameters to JAX arrays so XLA traces once
    total_steps_arr = jnp.asarray(total_steps, dtype=jnp.int32)
    lr_max_arr = jnp.asarray(cfg.lr, dtype=jnp.float32)
    lr_min_ratio_arr = jnp.asarray(cfg.lr_min_ratio, dtype=jnp.float32)
    value_weight_arr = jnp.asarray(0.0 if use_hybrid else cfg.value_weight, dtype=jnp.float32)
    weight_decay_arr = jnp.asarray(cfg.weight_decay, dtype=jnp.float32)
    beta1_arr = jnp.asarray(cfg.beta1, dtype=jnp.float32)
    beta2_arr = jnp.asarray(cfg.beta2, dtype=jnp.float32)
    eps_arr = jnp.asarray(cfg.eps, dtype=jnp.float32)
    max_grad_norm_arr = jnp.asarray(cfg.max_grad_norm, dtype=jnp.float32)

    history: list[dict[str, float]] = []
    total_samples_processed = 0
    t_wall_start = time.perf_counter()
    steady_state_sec = 0.0
    steady_state_samples = 0

    for epoch in range(1, cfg.epochs + 1):
        ep_start = time.perf_counter()
        ep_loss = 0.0
        ep_pol = 0.0
        ep_val = 0.0
        ep_top1 = 0.0
        ep_sign = 0.0
        ep_seen = 0
        last_lr = cfg.lr

        for x_np, pi_np, z_np in train_split.iter_batches(
            batch_size=cfg.batch_size,
            shuffle=True,
            rng=rng,
            drop_last=False,
        ):
            x_p, pi_p, z_p, mask_p, b = _pad_batch(x_np, pi_np, z_np, cfg.batch_size)
            params, opt_state, step_metrics = train_step(
                params=params,
                opt_state=opt_state,
                x=jnp.asarray(x_p, dtype=jnp.float32),
                pi_target=jnp.asarray(pi_p, dtype=jnp.float32),
                z_target=jnp.asarray(z_p, dtype=jnp.float32),
                sample_mask=jnp.asarray(mask_p, dtype=jnp.float32),
                total_steps=total_steps_arr,
                lr_max=lr_max_arr,
                lr_min_ratio=lr_min_ratio_arr,
                value_weight=value_weight_arr,
                weight_decay=weight_decay_arr,
                beta1=beta1_arr,
                beta2=beta2_arr,
                eps=eps_arr,
                max_grad_norm=max_grad_norm_arr,
            )
            ep_loss += float(step_metrics["total_loss"]) * b
            ep_pol += float(step_metrics["policy_loss"]) * b
            ep_val += float(step_metrics["value_mse"]) * b
            ep_top1 += float(step_metrics["top1_acc"]) * b
            ep_sign += float(step_metrics["value_sign_acc"]) * b
            last_lr = float(step_metrics["lr"])
            ep_seen += b
            total_samples_processed += b

        ep_sec = time.perf_counter() - ep_start
        if epoch > 1 or cfg.epochs == 1:
            steady_state_sec += ep_sec
            steady_state_samples += ep_seen

        ep_record = {
            "epoch": float(epoch),
            "total_loss": ep_loss / ep_seen,
            "policy_loss": ep_pol / ep_seen,
            "value_mse": ep_val / ep_seen,
            "top1_acc": ep_top1 / ep_seen,
            "value_sign_acc": ep_sign / ep_seen,
            "lr": last_lr,
            "elapsed_sec": ep_sec,
        }
        history.append(ep_record)

        if cfg.verbose and (epoch == 1 or epoch == cfg.epochs or epoch % max(1, cfg.epochs // 5) == 0):
            print(
                f"  Epoch {epoch:2d}/{cfg.epochs:2d} | "
                f"loss={ep_record['total_loss']:.4f} "
                f"(pol={ep_record['policy_loss']:.4f}, val={ep_record['value_mse']:.4f}) | "
                f"top1={ep_record['top1_acc'] * 100:.2f}% | "
                f"sign={ep_record['value_sign_acc'] * 100:.2f}% | "
                f"lr={last_lr:.2e} | {ep_sec * 1000:.1f} ms"
            )

    jax.tree_util.tree_map(lambda a: a.block_until_ready(), params)
    params = _finalize_hybrid_heads(params, train_split)
    jax.tree_util.tree_map(lambda a: a.block_until_ready(), params)
    wall_elapsed_sec = max(time.perf_counter() - t_wall_start, 1e-9)
    samples_per_sec = steady_state_samples / max(steady_state_sec, 1e-9)

    post_train = evaluate_split(
        params,
        train_split,
        batch_size=cfg.batch_size,
        value_weight=cfg.value_weight,
        weight_decay=cfg.weight_decay,
    )
    post_val = evaluate_split(
        params,
        val,
        batch_size=cfg.batch_size,
        value_weight=cfg.value_weight,
        weight_decay=cfg.weight_decay,
    )

    if cfg.verbose:
        print(format_metrics("POST-TRAIN (train)", post_train))
        if val.num_samples > 0:
            print(format_metrics("POST-TRAIN (val)  ", post_val))
        print(
            f"[THROUGHPUT] {total_samples_processed:,} samples in {wall_elapsed_sec:.2f}s wall "
            f"({samples_per_sec:,.0f} samples/sec steady-state on {jax.devices()[0]})"
        )

    summary = {
        "device": str(jax.devices()[0]),
        "config": asdict(cfg),
        "pre_train": asdict(pre_train),
        "post_train": asdict(post_train),
        "pre_val": asdict(pre_val),
        "post_val": asdict(post_val),
        "history": history,
        "elapsed_sec": wall_elapsed_sec,
        "steady_state_sec": steady_state_sec,
        "total_samples_processed": total_samples_processed,
        "samples_per_sec": samples_per_sec,
    }
    return params, summary


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Train Crosshair policy-value network on GPU with JAX.")
    parser.add_argument(
        "--data",
        nargs="+",
        required=True,
        help="One or more CXHRDAT1 binary dataset files.",
    )
    parser.add_argument(
        "--weights-in",
        type=Path,
        default=None,
        help="Input CXHRNET1 weights file (if omitted, initializes fresh weights).",
    )
    parser.add_argument(
        "--weights-out",
        type=Path,
        required=True,
        help="Output CXHRNET1 weights file.",
    )
    parser.add_argument(
        "--metrics-out",
        type=Path,
        default=None,
        help="Optional JSON file to write training metrics summary.",
    )
    parser.add_argument("--max-samples", type=int, default=250_000, help="Replay buffer capacity.")
    parser.add_argument("--val-fraction", type=float, default=0.1, help="Validation split fraction.")
    parser.add_argument("--epochs", type=int, default=3, help="Number of epochs.")
    parser.add_argument("--batch-size", type=int, default=512, help="Mini-batch size.")
    parser.add_argument("--lr", type=float, default=3e-4, help="Peak learning rate.")
    parser.add_argument("--weight-decay", type=float, default=1e-3, help="L2 weight decay.")
    parser.add_argument("--value-weight", type=float, default=0.20, help="Value MSE weight.")
    parser.add_argument("--seed", type=int, default=42, help="Random seed.")
    args = parser.parse_args(argv)

    buffer = ReplayBuffer(max_samples=args.max_samples)
    buffer.load_files(args.data)
    train_split, val_split = buffer.split_train_val(val_fraction=args.val_fraction, seed=args.seed)

    if args.weights_in is not None and args.weights_in.exists():
        params = load_weights(args.weights_in)
    else:
        params = init_params(seed=args.seed)

    cfg = TrainConfig(
        epochs=args.epochs,
        batch_size=args.batch_size,
        lr=args.lr,
        weight_decay=args.weight_decay,
        value_weight=args.value_weight,
        seed=args.seed,
        verbose=True,
    )
    trained_params, summary = train_model(params, train_split, val_split, config=cfg)
    save_weights(trained_params, args.weights_out)

    if args.metrics_out is not None:
        args.metrics_out.parent.mkdir(parents=True, exist_ok=True)
        args.metrics_out.write_text(json.dumps(summary, indent=2))

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
