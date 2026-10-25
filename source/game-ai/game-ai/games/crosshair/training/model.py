"""Pure-JAX Crosshair policy-value network and CXHRNET1 binary serialization.

Input encoding (6 planes x 49 cells = 294 float32s) is canonicalized to the
side to move under Klein four-group (V4) symmetry. When Blue is to move, the
board is transposed ((file, rank) <-> (rank, file)) before applying the V4
reflection so the side to move (Channel 0) always connects Top (rank 0) to
Bottom (rank 6), while the opponent (Channel 1) connects Left (file 0) to
Right (file 6).
"""

from __future__ import annotations

import ctypes
import os
import struct
import subprocess
import tempfile
from pathlib import Path
from typing import Any

# Disable XLA pool preallocation and cap device memory fraction before importing JAX.
os.environ.setdefault("XLA_PYTHON_CLIENT_PREALLOCATE", "false")
os.environ.setdefault("XLA_PYTHON_CLIENT_MEM_FRACTION", "0.50")

import jax
import jax.numpy as jnp
import numpy as np

# Enforce IEEE-754 float32 matmul precision (disabling Ada Lovelace TF32 mantissa
# truncation) so JAX GPU forward passes match Rust CPU f32 inference within < 1e-6.
jax.config.update("jax_default_matmul_precision", "highest")

MAGIC = b"CXHRNET1"
VERSION = 1
HEADER_BYTES = 32

BOARD_SIZE = 7
CELLS = 49
INPUT_CHANNELS = 6
INPUTS = INPUT_CHANNELS * CELLS  # 294
HIDDEN = 128
ACTIONS = 49
VALUE_HIDDEN = 32

# Parameter specification in exact binary flattening order.
# All matrices are stored row-major [out_dim, in_dim] so W[out_idx, in_idx] is at out_idx * in_dim + in_idx.
PARAM_SPECS: tuple[tuple[str, tuple[int, ...]], ...] = (
    ("W_stem", (HIDDEN, INPUTS)),          # (128, 294) = 37,632
    ("b_stem", (HIDDEN,)),                 # (128,)     = 128
    ("W_res1a", (HIDDEN, HIDDEN)),         # (128, 128) = 16,384
    ("b_res1a", (HIDDEN,)),                # (128,)     = 128
    ("W_res1b", (HIDDEN, HIDDEN)),         # (128, 128) = 16,384
    ("b_res1b", (HIDDEN,)),                # (128,)     = 128
    ("W_res2a", (HIDDEN, HIDDEN)),         # (128, 128) = 16,384
    ("b_res2a", (HIDDEN,)),                # (128,)     = 128
    ("W_res2b", (HIDDEN, HIDDEN)),         # (128, 128) = 16,384
    ("b_res2b", (HIDDEN,)),                # (128,)     = 128
    ("W_pol", (ACTIONS, HIDDEN)),          # (49, 128)  = 6,272
    ("b_pol", (ACTIONS,)),                 # (49,)      = 49
    ("W_val1", (VALUE_HIDDEN, HIDDEN)),    # (32, 128)  = 4,096
    ("b_val1", (VALUE_HIDDEN,)),           # (32,)      = 32
    ("W_val2", (1, VALUE_HIDDEN)),         # (1, 32)    = 32
    ("b_val2", (1,)),                      # (1,)       = 1
)

PARAM_COUNT = sum(int(np.prod(shape)) for _, shape in PARAM_SPECS)  # 114,290
assert PARAM_COUNT == 114290, f"Expected 114290 parameters, got {PARAM_COUNT}"

PAYLOAD_BYTES = PARAM_COUNT * 4  # 457,160
TOTAL_BYTES = HEADER_BYTES + PAYLOAD_BYTES  # 457,192

HEADER_STRUCT = struct.Struct("<8sIHHHHIQ")
assert HEADER_STRUCT.size == HEADER_BYTES

FNV_OFFSET_BASIS = 0xCBF29CE484222325
FNV_PRIME = 0x0100000001B3
FNV_MASK = 0xFFFFFFFFFFFFFFFF

Params = dict[str, jax.Array]


def _load_fast_fnv1a() -> Any | None:
    """Compile and load a small C shared library in the system temp directory for FNV-1a checksums."""
    cache_dir = Path(tempfile.gettempdir()) / "ai_crosshair_fnv1a"
    cache_dir.mkdir(parents=True, exist_ok=True)
    so_path = cache_dir / "_fnv1a_fast.so"
    if not so_path.exists():
        c_code = """
        #include <stdint.h>
        #include <stddef.h>
        uint64_t fnv1a_64(const uint8_t *data, size_t len) {
            uint64_t hash = 0xcbf29ce484222325ULL;
            for (size_t i = 0; i < len; ++i) {
                hash ^= (uint64_t)data[i];
                hash *= 0x0100000001b3ULL;
            }
            return hash;
        }
        """
        try:
            with tempfile.NamedTemporaryFile(suffix=".c", mode="w", delete=False) as tmp:
                tmp.write(c_code)
                tmp_c = tmp.name
            try:
                subprocess.run(
                    ["cc", "-O3", "-shared", "-fPIC", tmp_c, "-o", str(so_path)],
                    check=True,
                    capture_output=True,
                )
            finally:
                Path(tmp_c).unlink(missing_ok=True)
        except Exception:
            return None

    try:
        lib = ctypes.CDLL(str(so_path))
        func = lib.fnv1a_64
        func.argtypes = [ctypes.c_char_p, ctypes.c_size_t]
        func.restype = ctypes.c_uint64
        return func
    except Exception:
        return None


_FAST_FNV1A = _load_fast_fnv1a()


def fnv1a_checksum(payload: bytes | bytearray | memoryview) -> int:
    """Compute the 64-bit FNV-1a checksum of payload bytes."""
    raw = bytes(payload) if not isinstance(payload, bytes) else payload
    if _FAST_FNV1A is not None:
        return int(_FAST_FNV1A(raw, len(raw)))
    h = FNV_OFFSET_BASIS
    prime = FNV_PRIME
    mask = FNV_MASK
    for b in raw:
        h = ((h ^ b) * prime) & mask
    return h


def init_params(seed: int = 42) -> Params:
    """Initialize network parameters with He/Xavier scaling."""
    key = jax.random.PRNGKey(seed)
    params: Params = {}
    for name, shape in PARAM_SPECS:
        if name.startswith("b_"):
            params[name] = jnp.zeros(shape, dtype=jnp.float32)
        else:
            key, subkey = jax.random.split(key)
            out_dim, in_dim = shape
            if name in ("W_pol", "W_val2"):
                # Small initialization on output heads for near-uniform initial priors and near-zero value
                scale = float(np.sqrt(1.0 / in_dim)) * 0.1
            elif name in ("W_res1b", "W_res2b"):
                # Second layer of residual block scaled slightly down for stable identity path at start
                scale = float(np.sqrt(1.0 / in_dim))
            else:
                # He normal initialization for ReLU layers
                scale = float(np.sqrt(2.0 / in_dim))
            params[name] = jax.random.normal(subkey, shape, dtype=jnp.float32) * scale
    return params


def _linear(x: jax.Array, w: jax.Array, b: jax.Array) -> jax.Array:
    """Compute x @ w.T + b using highest float32 precision."""
    return jnp.matmul(x, jnp.swapaxes(w, -1, -2), precision=jax.lax.Precision.HIGHEST) + b


@jax.jit
def forward(
    params: Params,
    x: jax.Array,
    legal_mask: jax.Array | None = None,
) -> tuple[jax.Array, jax.Array]:
    """Run the forward pass of the Crosshair policy-value network.

    Args:
        params: Dictionary of network weights and biases matching PARAM_SPECS.
        x: Input tensor of shape (294,) or (batch, 294).
        legal_mask: Optional boolean/float mask of shape (49,) or (batch, 49).
            If provided, illegal entries in logits are masked with -1e9.

    Returns:
        (logits, value) where:
            logits has shape (49,) or (batch, 49)
            value has shape () or (batch,) in [-1.0, 1.0]
    """
    h0 = jax.nn.relu(_linear(x, params["W_stem"], params["b_stem"]))
    u1 = jax.nn.relu(_linear(h0, params["W_res1a"], params["b_res1a"]))
    h1 = jax.nn.relu(h0 + _linear(u1, params["W_res1b"], params["b_res1b"]))
    u2 = jax.nn.relu(_linear(h1, params["W_res2a"], params["b_res2a"]))
    h2 = jax.nn.relu(h1 + _linear(u2, params["W_res2b"], params["b_res2b"]))

    logits = _linear(h2, params["W_pol"], params["b_pol"])
    if legal_mask is not None:
        mask_bool = legal_mask > 0.5 if legal_mask.dtype != jnp.bool_ else legal_mask
        logits = jnp.where(mask_bool, logits, -1e9)

    v_hid = jax.nn.relu(_linear(h2, params["W_val1"], params["b_val1"]))
    value = jnp.tanh(_linear(v_hid, params["W_val2"], params["b_val2"])).squeeze(-1)
    return logits, value


@jax.jit
def predict(
    params: Params,
    x: jax.Array,
    legal_mask: jax.Array | None = None,
) -> tuple[jax.Array, jax.Array]:
    """Compute masked action probabilities and value prediction matching Rust inference.

    If legal_mask is None, extracts channel 2 (indices 98..147) from x as the legal move mask.
    """
    if legal_mask is None:
        legal_mask = x[..., 98:147] > 0.5
    else:
        legal_mask = legal_mask > 0.5 if legal_mask.dtype != jnp.bool_ else legal_mask

    logits, value = forward(params, x, legal_mask=legal_mask)
    probs = jax.nn.softmax(logits, axis=-1)
    probs = jnp.where(legal_mask, probs, 0.0)
    norm = jnp.sum(probs, axis=-1, keepdims=True)
    probs = jnp.where(norm > 0.0, probs / jnp.maximum(norm, 1e-12), 0.0)
    return probs, value


def flatten_params(params: Params) -> np.ndarray:
    """Flatten parameter dictionary into a 1D float32 numpy array of length 114,290."""
    chunks: list[np.ndarray] = []
    for name, expected_shape in PARAM_SPECS:
        if name not in params:
            raise KeyError(f"Missing parameter {name!r}")
        arr = np.asarray(params[name], dtype=np.float32)
        if arr.shape != expected_shape:
            raise ValueError(
                f"Parameter {name!r} has shape {arr.shape}, expected {expected_shape}"
            )
        if not np.all(np.isfinite(arr)):
            raise ValueError(f"Parameter {name!r} contains non-finite values")
        chunks.append(np.ascontiguousarray(arr, dtype="<f4").reshape(-1))
    flat = np.concatenate(chunks, axis=0)
    assert flat.shape == (PARAM_COUNT,)
    return flat


def unflatten_params(flat: np.ndarray, as_jax: bool = True) -> Params:
    """Unflatten a 1D float32 array of length 114,290 into the parameter dictionary."""
    arr = np.asarray(flat, dtype=np.float32).reshape(-1)
    if arr.size != PARAM_COUNT:
        raise ValueError(f"Expected {PARAM_COUNT} floats, got {arr.size}")
    if not np.all(np.isfinite(arr)):
        raise ValueError("Model parameters contain non-finite values")

    params: dict[str, Any] = {}
    offset = 0
    for name, shape in PARAM_SPECS:
        count = int(np.prod(shape))
        sub = arr[offset : offset + count].reshape(shape)
        params[name] = jnp.asarray(sub, dtype=jnp.float32) if as_jax else sub.copy()
        offset += count
    assert offset == PARAM_COUNT
    return params


def serialize_weights(params: Params) -> bytes:
    """Serialize parameters into the 457,192-byte CXHRNET1 binary format."""
    flat = flatten_params(params)
    payload = flat.astype("<f4", copy=False).tobytes()
    assert len(payload) == PAYLOAD_BYTES
    checksum = fnv1a_checksum(payload)
    header = HEADER_STRUCT.pack(
        MAGIC,
        VERSION,
        INPUTS,
        HIDDEN,
        ACTIONS,
        VALUE_HIDDEN,
        PARAM_COUNT,
        checksum,
    )
    return header + payload


def deserialize_weights(data: bytes | bytearray | memoryview, as_jax: bool = True) -> Params:
    """Deserialize and validate a CXHRNET1 binary buffer into network parameters."""
    raw = bytes(data)
    if len(raw) != TOTAL_BYTES:
        raise ValueError(f"Invalid CXHRNET1 byte length: expected {TOTAL_BYTES}, got {len(raw)}")

    (
        magic,
        version,
        inputs,
        hidden,
        actions,
        value_hidden,
        param_count,
        expected_checksum,
    ) = HEADER_STRUCT.unpack_from(raw, 0)

    if magic != MAGIC:
        raise ValueError(f"Invalid CXHRNET1 magic: {magic!r}")
    if version != VERSION:
        raise ValueError(f"Unsupported CXHRNET1 version: {version}")
    if (inputs, hidden, actions, value_hidden, param_count) != (
        INPUTS,
        HIDDEN,
        ACTIONS,
        VALUE_HIDDEN,
        PARAM_COUNT,
    ):
        raise ValueError(
            f"CXHRNET1 architecture mismatch: got "
            f"(inputs={inputs}, hidden={hidden}, actions={actions}, "
            f"value_hidden={value_hidden}, param_count={param_count})"
        )

    payload = raw[HEADER_BYTES:]
    actual_checksum = fnv1a_checksum(payload)
    if actual_checksum != expected_checksum:
        raise ValueError(
            f"CXHRNET1 FNV-1a checksum mismatch: header=0x{expected_checksum:016x}, "
            f"computed=0x{actual_checksum:016x}"
        )

    flat = np.frombuffer(payload, dtype="<f4").copy()
    return unflatten_params(flat, as_jax=as_jax)


def save_weights(params: Params, path: str | Path) -> Path:
    """Write parameters in CXHRNET1 format atomically to disk."""
    out_path = Path(path)
    out_path.parent.mkdir(parents=True, exist_ok=True)
    blob = serialize_weights(params)
    tmp_path = out_path.with_suffix(out_path.suffix + ".tmp")
    tmp_path.write_bytes(blob)
    tmp_path.replace(out_path)
    return out_path


def load_weights(path: str | Path, as_jax: bool = True) -> Params:
    """Load and validate parameters in CXHRNET1 format from disk."""
    return deserialize_weights(Path(path).read_bytes(), as_jax=as_jax)
