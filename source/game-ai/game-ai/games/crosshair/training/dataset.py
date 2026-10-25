"""CXHRDAT1 binary dataset loader, writer, and sliding-window replay buffer.

Each CXHRDAT1 file begins with a 16-byte little-endian header (`magic = b"CXHRDAT1"`,
`version = 1`, `num_samples: u32`) followed by `num_samples` contiguous records of
344 little-endian float32s (1,376 bytes per sample):
  - `input[294]`: 6x7x7 canonical feature planes (Blue transposed so side_to_move
    always connects Top to Bottom under Klein four-group V4 symmetry).
  - `policy[49]`: MCTS root visit distribution in canonical coordinates.
  - `value[1]`:   terminal outcome z in {-1.0, +1.0} from side_to_move's perspective.
"""

from __future__ import annotations

import struct
from dataclasses import dataclass
from pathlib import Path
from typing import Iterator, Sequence

import numpy as np

from model import ACTIONS, INPUTS

DATA_MAGIC = b"CXHRDAT1"
DATA_VERSION = 1
DATA_HEADER_BYTES = 16
DATA_HEADER_STRUCT = struct.Struct("<8sII")
assert DATA_HEADER_STRUCT.size == DATA_HEADER_BYTES

SAMPLE_FLOATS = INPUTS + ACTIONS + 1  # 294 + 49 + 1 = 344
SAMPLE_BYTES = SAMPLE_FLOATS * 4      # 1376 bytes


@dataclass(frozen=True)
class DatasetSplit:
    """Contiguous float32 dataset split ready for GPU mini-batch streaming."""

    inputs: np.ndarray          # (N, 294) float32
    policy_targets: np.ndarray  # (N, 49)  float32
    value_targets: np.ndarray   # (N,)     float32

    @property
    def num_samples(self) -> int:
        return int(self.inputs.shape[0])

    @property
    def legal_masks(self) -> np.ndarray:
        """Boolean legal move mask extracted from channel 2 (indices 98..147)."""
        return self.inputs[:, 98:147] > 0.5

    def iter_batches(
        self,
        batch_size: int,
        shuffle: bool = True,
        rng: np.random.Generator | None = None,
        drop_last: bool = False,
    ) -> Iterator[tuple[np.ndarray, np.ndarray, np.ndarray]]:
        """Yield (inputs, policy_targets, value_targets) mini-batches."""
        n = self.num_samples
        if n == 0:
            return
        if shuffle:
            if rng is None:
                rng = np.random.default_rng()
            indices = rng.permutation(n)
        else:
            indices = np.arange(n)

        for start in range(0, n, batch_size):
            end = start + batch_size
            if end > n and drop_last:
                break
            batch_idx = indices[start:min(end, n)]
            yield (
                self.inputs[batch_idx],
                self.policy_targets[batch_idx],
                self.value_targets[batch_idx],
            )


def load_raw_records(path: str | Path) -> np.ndarray:
    """Read a CXHRDAT1 file into a (num_samples, 344) float32 array."""
    file_path = Path(path)
    data = file_path.read_bytes()
    if len(data) < DATA_HEADER_BYTES:
        raise ValueError(
            f"CXHRDAT1 file {file_path} too short for 16-byte header ({len(data)} bytes)"
        )

    magic, version, num_samples = DATA_HEADER_STRUCT.unpack_from(data, 0)
    if magic != DATA_MAGIC:
        raise ValueError(f"Invalid CXHRDAT1 magic in {file_path}: {magic!r}")
    if version != DATA_VERSION:
        raise ValueError(f"Unsupported CXHRDAT1 version in {file_path}: {version}")

    expected_len = DATA_HEADER_BYTES + num_samples * SAMPLE_BYTES
    if len(data) != expected_len:
        raise ValueError(
            f"CXHRDAT1 byte length mismatch in {file_path}: "
            f"header says {num_samples} samples ({expected_len} bytes), got {len(data)} bytes"
        )

    if num_samples == 0:
        return np.zeros((0, SAMPLE_FLOATS), dtype=np.float32)

    records = np.frombuffer(data, dtype="<f4", offset=DATA_HEADER_BYTES).reshape(
        num_samples, SAMPLE_FLOATS
    )
    if not np.all(np.isfinite(records)):
        raise ValueError(f"CXHRDAT1 file {file_path} contains non-finite float32 values")
    return np.array(records, dtype=np.float32, copy=True)


def load_dataset_file(path: str | Path) -> DatasetSplit:
    """Load a single CXHRDAT1 binary file as a DatasetSplit."""
    records = load_raw_records(path)
    return records_to_split(records)


def records_to_split(records: np.ndarray) -> DatasetSplit:
    """Unpack a (N, 344) float32 array into a DatasetSplit."""
    if records.ndim != 2 or records.shape[1] != SAMPLE_FLOATS:
        raise ValueError(f"Expected shape (N, {SAMPLE_FLOATS}), got {records.shape}")
    inputs = np.ascontiguousarray(records[:, :INPUTS], dtype=np.float32)
    policy_targets = np.ascontiguousarray(
        records[:, INPUTS : INPUTS + ACTIONS], dtype=np.float32
    )
    value_targets = np.ascontiguousarray(records[:, INPUTS + ACTIONS], dtype=np.float32)
    return DatasetSplit(
        inputs=inputs,
        policy_targets=policy_targets,
        value_targets=value_targets,
    )


def save_dataset_file(
    path: str | Path,
    inputs: np.ndarray,
    policy_targets: np.ndarray,
    value_targets: np.ndarray,
) -> Path:
    """Write samples to disk in CXHRDAT1 binary format."""
    inp = np.asarray(inputs, dtype=np.float32)
    pol = np.asarray(policy_targets, dtype=np.float32)
    val = np.asarray(value_targets, dtype=np.float32).reshape(-1, 1)

    n = inp.shape[0]
    if inp.shape != (n, INPUTS):
        raise ValueError(f"Expected inputs shape {(n, INPUTS)}, got {inp.shape}")
    if pol.shape != (n, ACTIONS):
        raise ValueError(f"Expected policy_targets shape {(n, ACTIONS)}, got {pol.shape}")
    if val.shape != (n, 1):
        raise ValueError(f"Expected value_targets shape {(n, 1)}, got {val.shape}")

    records = np.concatenate([inp, pol, val], axis=1).astype("<f4", copy=False)
    assert records.shape == (n, SAMPLE_FLOATS)

    header = DATA_HEADER_STRUCT.pack(DATA_MAGIC, DATA_VERSION, n)
    out_path = Path(path)
    out_path.parent.mkdir(parents=True, exist_ok=True)
    tmp_path = out_path.with_suffix(out_path.suffix + ".tmp")
    with tmp_path.open("wb") as f:
        f.write(header)
        f.write(records.tobytes())
    tmp_path.replace(out_path)
    return out_path


def _find_game_chunks(records: np.ndarray, default_chunk_size: int = 64) -> list[tuple[int, int]]:
    """Segment records into game trajectories (or V4-symmetry-aligned chunks) for train/val splitting.

    Channel 5 (index 245) encodes ply / 49.0. Within a single game trajectory (including 4x Klein
    four-group V4 symmetry augmentation), ply is monotonically non-decreasing; when a new game
    starts, ply drops. If all samples share the same ply (e.g. synthetic single-ply test data),
    falls back to fixed chunks of size `default_chunk_size` (aligned to 4-way V4 symmetries).
    """
    n = records.shape[0]
    if n == 0:
        return []

    plies = np.rint(records[:, 245] * 49.0).astype(np.int32)
    drops = np.where(plies[1:] < plies[:-1])[0] + 1

    if len(drops) >= 2:
        boundaries = [0, *drops.tolist(), n]
        return [(boundaries[i], boundaries[i + 1]) for i in range(len(boundaries) - 1)]

    # Fallback: fixed chunks of multiples of 4 (or smaller if dataset is tiny)
    step = min(default_chunk_size, max(4, n // 10)) if n >= 16 else max(1, n // 5)
    return [(start, min(start + step, n)) for start in range(0, n, step)]


class ReplayBuffer:
    """Sliding-window replay buffer stored in contiguous float32 memory.

    Default capacity of 250,000 samples occupies 250,000 * 1,376 B = 344 MB.
    """

    def __init__(self, max_samples: int = 250_000) -> None:
        if max_samples <= 0:
            raise ValueError("max_samples must be positive")
        self.max_samples = int(max_samples)
        self._buffer = np.empty((0, SAMPLE_FLOATS), dtype=np.float32)
        self.total_added = 0
        self.files_loaded: list[Path] = []

    def __len__(self) -> int:
        return int(self._buffer.shape[0])

    @property
    def memory_bytes(self) -> int:
        return int(self._buffer.nbytes)

    def add_raw(self, records: np.ndarray) -> int:
        """Append raw (N, 344) records and trim oldest samples if over capacity."""
        arr = np.asarray(records, dtype=np.float32)
        if arr.ndim != 2 or arr.shape[1] != SAMPLE_FLOATS:
            raise ValueError(f"Expected records of shape (N, {SAMPLE_FLOATS}), got {arr.shape}")
        n_new = arr.shape[0]
        if n_new == 0:
            return len(self)

        self.total_added += n_new
        if n_new >= self.max_samples:
            self._buffer = np.ascontiguousarray(arr[-self.max_samples :], dtype=np.float32)
        elif len(self._buffer) == 0:
            self._buffer = np.ascontiguousarray(arr, dtype=np.float32)
        else:
            combined_len = len(self._buffer) + n_new
            if combined_len <= self.max_samples:
                self._buffer = np.concatenate([self._buffer, arr], axis=0)
            else:
                keep_old = self.max_samples - n_new
                self._buffer = np.concatenate([self._buffer[-keep_old:], arr], axis=0)
        return len(self)

    def add_samples(
        self,
        inputs: np.ndarray,
        policy_targets: np.ndarray,
        value_targets: np.ndarray,
    ) -> int:
        """Append unpacked (inputs, policy_targets, value_targets) to the buffer."""
        inp = np.asarray(inputs, dtype=np.float32)
        pol = np.asarray(policy_targets, dtype=np.float32)
        val = np.asarray(value_targets, dtype=np.float32).reshape(-1, 1)
        records = np.concatenate([inp, pol, val], axis=1)
        return self.add_raw(records)

    def add_file(self, path: str | Path) -> int:
        """Load a CXHRDAT1 file and append its records to the replay buffer."""
        file_path = Path(path)
        records = load_raw_records(file_path)
        self.files_loaded.append(file_path)
        return self.add_raw(records)

    def load_files(self, paths: Sequence[str | Path]) -> int:
        """Load multiple generation files in order, only reading the tail needed to fill max_samples."""
        path_list = [Path(p) for p in paths]
        # Inspect sample counts from headers first so we skip reading ancient files if already over capacity
        counts: list[int] = []
        for p in path_list:
            with p.open("rb") as f:
                hdr = f.read(DATA_HEADER_BYTES)
            if len(hdr) < DATA_HEADER_BYTES:
                raise ValueError(f"CXHRDAT1 file {p} too short for header")
            magic, version, num_samples = DATA_HEADER_STRUCT.unpack(hdr)
            if magic != DATA_MAGIC or version != DATA_VERSION:
                raise ValueError(f"Invalid CXHRDAT1 header in {p}")
            counts.append(num_samples)

        # Find earliest file needed to fill remaining capacity
        needed = self.max_samples
        start_idx = len(path_list)
        accum = 0
        for idx in range(len(path_list) - 1, -1, -1):
            accum += counts[idx]
            start_idx = idx
            if accum >= needed:
                break

        for p in path_list[start_idx:]:
            self.add_file(p)
        return len(self)

    def as_split(self) -> DatasetSplit:
        """Return the entire buffer as a single DatasetSplit."""
        return records_to_split(self._buffer)

    def split_train_val(
        self,
        val_fraction: float = 0.1,
        seed: int = 42,
    ) -> tuple[DatasetSplit, DatasetSplit]:
        """Split the replay buffer at the game/chunk level into (train_split, val_split)."""
        n = len(self._buffer)
        if n == 0:
            empty = records_to_split(self._buffer)
            return empty, empty

        if val_fraction <= 0.0 or n < 2:
            return records_to_split(self._buffer), records_to_split(self._buffer[:0])

        chunks = _find_game_chunks(self._buffer)
        rng = np.random.default_rng(seed)

        if len(chunks) <= 1:
            # Fallback sample-level split if only 1 chunk
            val_count = max(1, int(round(n * val_fraction)))
            perm = rng.permutation(n)
            val_idx = np.sort(perm[:val_count])
            train_idx = np.sort(perm[val_count:])
            return (
                records_to_split(self._buffer[train_idx]),
                records_to_split(self._buffer[val_idx]),
            )

        # Deterministic per-game-chunk assignment so train and val trajectories never leak
        # across generations when the replay buffer grows.
        stride = max(2, int(round(1.0 / val_fraction)))
        offset = int(seed) % stride
        val_chunks: list[tuple[int, int]] = []
        train_chunks: list[tuple[int, int]] = []

        for c_idx, (start, end) in enumerate(chunks):
            if (c_idx % stride) == offset and len(train_chunks) + (len(chunks) - c_idx - 1) >= 1:
                val_chunks.append((start, end))
            else:
                train_chunks.append((start, end))

        if not val_chunks and len(chunks) >= 2:
            val_chunks.append(chunks[-1])
            train_chunks = chunks[:-1]

        train_records = (
            np.concatenate([self._buffer[s:e] for s, e in train_chunks], axis=0)
            if train_chunks
            else self._buffer[:0]
        )
        val_records = (
            np.concatenate([self._buffer[s:e] for s, e in val_chunks], axis=0)
            if val_chunks
            else self._buffer[:0]
        )
        return records_to_split(train_records), records_to_split(val_records)
