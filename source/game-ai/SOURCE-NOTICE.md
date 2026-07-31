# Source and license notice

The Game AI browser interface and its corresponding source are distributed
under the GNU General Public License, version 3 or any later version.

Copyright © Nick Gustafson.

The chessboard interface uses
[@lichess-org/chessground](https://github.com/lichess-org/chessground) 10.1.1,
copyright the Lichess team and contributors, under GPL-3.0-or-later. Its
preferred TypeScript source, package metadata, assets, README, and license are
included under `vendor/chessground/`.

The Rust engines and the exact `tiny-v1.gainnue` network consumed by the chess
engine are included under `game-ai/`. No training corpus is required to
rebuild the distributed WebAssembly. Training utilities are present because
they are part of the engine source; private corpora and author-only experiments
are not.

There is no warranty, to the extent permitted by law. See `LICENSE` for the
complete terms.
