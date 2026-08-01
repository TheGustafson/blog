# Classical PSQT tuning

The runtime chess engine contains only immutable integer material and
piece-square tables. This directory documents how the generated
`src/psqt_tuned.rs` values were selected.

## Seed and corpus

- Seed: Ronald Friederich's public PeSTO middlegame/endgame material values and
  Texel-tuned tables, transcribed as data from the
  [Chess Programming Wiki](https://www.chessprogramming.org/PeSTO%27s_Evaluation_Function).
- Local source: a private Archon hard-negative corpus.
- Source shape: `FEN | white-POV teacher cp | white game result`.
- Teacher: Stockfish 18, fixed 50,000-node searches.
- Source scores were multiplied by 1.94 for Archon's historical training
  scale. The tuner divides by 1.94 before fitting conventional centipawns.
- Source rows: 1,723,284.
- Source FNV-1a, including normalized line endings:
  `b83e00ab72d28e6d`.

The large corpus remains private training data and is not part of a public
site publish. The generator, exact settings, checksum, and emitted weights are
versioned here.

## Clean-position boundary

The tuner deterministically samples by the production Zobrist key, then uses
the production FEN parser, attack detector, and legal move generator. It
rejects:

- malformed rows or FEN;
- raw labels outside ±2,400 corpus units;
- duplicate position identities;
- positions where the side to move is in check;
- terminal positions;
- positions with any legal capture, en passant, or promotion.

This left 62,119 quiet training positions and a disjoint 15,629-position
validation split. The split is hash-based so the source file's lexical order
does not leak related rows between sets.

## Model and constraints

The predictor is exactly the runtime evaluator: White-relative material plus
one square value per piece, independently accumulated in middlegame and
endgame channels and tapered by the 24-unit phase.

The material anchors stay fixed. The 768 square values use mini-batch Adam,
Huber loss with a 200 cp transition, 12 deterministic epochs, and a
seed-relative penalty. Every learned table entry is clamped to ±32 cp from its
PeSTO seed. That bound exists because interpretability is part of the model
contract; a lower regression error cannot justify nonsensical compensating
piece values.

## Locked result

| Weights | Validation MAE | Validation RMSE |
| --- | ---: | ---: |
| PeSTO seed | 154.10 cp | 204.05 cp |
| tuned float | 149.38 cp | 196.97 cp |
| emitted integer | 149.38 cp | 196.97 cp |

These measurements describe fit to this hard-position distribution. They are
not an Elo estimate. Playing strength still needs equal-budget engine games.

Reproduction:

```bash
cargo run --release --bin ai-chess-psqt-tuner -- \
  /path/to/hn_cp_wdl_filtered.txt
```

The tool prints a complete `psqt_tuned.rs` module. Generated constants are
reviewed and committed; the runtime never opens the corpus.
