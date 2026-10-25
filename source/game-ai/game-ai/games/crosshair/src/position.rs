use std::fmt;
use std::str::FromStr;

/// Width and height of the Crosshair board.
pub const SIZE: usize = 7;

/// Total number of cells on the 7×7 Crosshair board.
pub const CELLS: usize = SIZE * SIZE;

/// Bitmask covering all 49 valid board cells (`0..49`).
pub const BOARD_MASK: u64 = (1_u64 << CELLS) - 1;

/// Top edge mask (`rank == 0`, cells `a1..g1`). Red connects Top to Bottom.
pub const TOP_EDGE_MASK: u64 = 0b111_1111;

/// Bottom edge mask (`rank == 6`, cells `a7..g7`). Red connects Top to Bottom.
pub const BOTTOM_EDGE_MASK: u64 = 0b111_1111_u64 << 42;

/// Left edge mask (`file == 0`, cells `a1..a7`). Blue connects Left to Right.
pub const LEFT_EDGE_MASK: u64 = (1_u64 << 0)
    | (1_u64 << 7)
    | (1_u64 << 14)
    | (1_u64 << 21)
    | (1_u64 << 28)
    | (1_u64 << 35)
    | (1_u64 << 42);

/// Right edge mask (`file == 6`, cells `g1..g7`). Blue connects Left to Right.
pub const RIGHT_EDGE_MASK: u64 = LEFT_EDGE_MASK << 6;

const NOT_FILE_A: u64 = BOARD_MASK & !LEFT_EDGE_MASK;
const NOT_FILE_G: u64 = BOARD_MASK & !RIGHT_EDGE_MASK;

/// Player color on the Crosshair board.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Color {
    /// Red moves first (at ply 0) and connects the Top edge (`rank 0`) to the Bottom edge (`rank 6`).
    Red,
    /// Blue moves second (at ply 1) and connects the Left edge (`file 0`) to the Right edge (`file 6`).
    Blue,
}

impl Color {
    /// Returns the opposing player color.
    pub const fn opponent(self) -> Self {
        match self {
            Self::Red => Self::Blue,
            Self::Blue => Self::Red,
        }
    }

    /// Returns the single-character protocol code (`"R"` or `"B"`).
    pub const fn code(self) -> &'static str {
        match self {
            Self::Red => "R",
            Self::Blue => "B",
        }
    }

    /// Returns `0` for `Red` and `1` for `Blue`.
    pub const fn index(self) -> usize {
        match self {
            Self::Red => 0,
            Self::Blue => 1,
        }
    }
}

impl fmt::Display for Color {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}

/// Reason a game of Crosshair ended in victory.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum WinReason {
    /// A 4-orthogonal chain of stones connecting the player's two opposite target edges.
    Connection,
}

impl WinReason {
    /// Returns the lowercase JSON/protocol identifier (`"connection"`).
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Connection => "connection",
        }
    }
}

impl fmt::Display for WinReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Outcome status of a Crosshair position. Crosshair has zero draws.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum GameResult {
    /// The game is still in progress.
    Ongoing,
    /// One player has won the game.
    Win {
        /// Winning color.
        winner: Color,
        /// How the game was won.
        reason: WinReason,
    },
}

/// A board coordinate on the 7×7 grid (`0..49`, formatted `"a1"` through `"g7"`).
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Move(u8);

impl Move {
    /// Creates a move from zero-based `file` (`0..7`) and `rank` (`0..7`).
    pub const fn new(file: u8, rank: u8) -> Self {
        assert!(file < SIZE as u8 && rank < SIZE as u8, "move out of bounds");
        Self(rank * (SIZE as u8) + file)
    }

    /// Creates a move from a flat cell index in `0..49`.
    pub const fn from_index(index: u8) -> Self {
        assert!(index < CELLS as u8, "cell index out of bounds");
        Self(index)
    }

    /// Fallibly creates a move from a flat cell index in `0..49`.
    pub const fn try_from_index(index: u8) -> Result<Self, ParseMoveError> {
        if index < CELLS as u8 {
            Ok(Self(index))
        } else {
            Err(ParseMoveError::OutOfBounds)
        }
    }

    /// Returns the flat cell index (`rank * 7 + file`) in `0..49`.
    pub const fn index(self) -> u8 {
        self.0
    }

    /// Returns the zero-based column (`0..7`, corresponding to `'a'..'g'`).
    pub const fn file(self) -> u8 {
        self.0 % (SIZE as u8)
    }

    /// Returns the zero-based row (`0..7`, corresponding to `'1'..'7'`).
    pub const fn rank(self) -> u8 {
        self.0 / (SIZE as u8)
    }

    /// Returns the single-bit mask `1 << index` for this cell.
    pub const fn bit(self) -> u64 {
        1_u64 << self.0
    }
}

impl fmt::Display for Move {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let file = (b'a' + self.file()) as char;
        let rank = (b'1' + self.rank()) as char;
        write!(f, "{file}{rank}")
    }
}

/// Error returned when parsing a coordinate string fails.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ParseMoveError {
    /// Input did not have the two-character `<file><rank>` format.
    InvalidFormat,
    /// Coordinate was outside `a1..g7`.
    OutOfBounds,
}

impl fmt::Display for ParseMoveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidFormat => f.write_str("move must be a two-character coordinate like d4"),
            Self::OutOfBounds => f.write_str("move coordinate must be within a1 through g7"),
        }
    }
}

impl std::error::Error for ParseMoveError {}

impl FromStr for Move {
    type Err = ParseMoveError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let bytes = s.trim().as_bytes();
        if bytes.len() != 2 {
            return Err(ParseMoveError::InvalidFormat);
        }
        let file_byte = bytes[0].to_ascii_lowercase();
        let rank_byte = bytes[1];
        if !(b'a'..=b'g').contains(&file_byte) || !(b'1'..=b'7').contains(&rank_byte) {
            return Err(ParseMoveError::OutOfBounds);
        }
        Ok(Self::new(file_byte - b'a', rank_byte - b'1'))
    }
}

/// Error returned when attempting to play an illegal move.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MoveError {
    /// The game has already ended.
    GameOver,
    /// The target cell already contains a stone.
    CellOccupied,
    /// The target cell lies outside the active crosshair row and column.
    OutsideCrosshair,
}

impl fmt::Display for MoveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::GameOver => f.write_str("game is already over"),
            Self::CellOccupied => f.write_str("cell is already occupied"),
            Self::OutsideCrosshair => f.write_str("move is outside the active crosshair"),
        }
    }
}

impl std::error::Error for MoveError {}

/// Bitboard-backed list of cells ordered by ascending cell index (`0..49`).
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct MoveList(u64);

impl MoveList {
    /// Creates a move list from a 49-bit mask.
    pub const fn from_mask(mask: u64) -> Self {
        Self(mask & BOARD_MASK)
    }

    /// Returns the underlying 49-bit mask.
    pub const fn mask(self) -> u64 {
        self.0
    }

    /// Returns the number of moves in the list.
    pub const fn len(self) -> usize {
        self.0.count_ones() as usize
    }

    /// Returns `true` if the list contains no moves.
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// Returns `true` if `mv` is present in the list.
    pub const fn contains(self, mv: Move) -> bool {
        (self.0 & mv.bit()) != 0
    }

    /// Returns an iterator over the moves in ascending cell index order.
    pub const fn iter(self) -> MoveIter {
        MoveIter(self.0)
    }
}

impl IntoIterator for MoveList {
    type Item = Move;
    type IntoIter = MoveIter;

    fn into_iter(self) -> Self::IntoIter {
        MoveIter(self.0)
    }
}

/// Iterator over a [`MoveList`] in ascending cell index order.
#[derive(Clone, Copy, Debug)]
pub struct MoveIter(u64);

impl Iterator for MoveIter {
    type Item = Move;

    fn next(&mut self) -> Option<Self::Item> {
        if self.0 == 0 {
            return None;
        }
        let index = self.0.trailing_zeros() as u8;
        self.0 &= self.0 - 1;
        Some(Move::from_index(index))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.0.count_ones() as usize;
        (len, Some(len))
    }
}

impl ExactSizeIterator for MoveIter {}

/// Tactical and routing preview for a single legal move in the current position.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MovePreview {
    /// The candidate legal move.
    pub mv: Move,
    /// Opponent cells that would flip if `mv` is played (both linear and 2×2 crosscut flips).
    pub flips: MoveList,
    /// Next active crosshair row (`0..7`), or `None` if `next_wildcard` is true.
    pub next_active_row: Option<u8>,
    /// Next active crosshair column (`0..7`), or `None` if `next_wildcard` is true.
    pub next_active_col: Option<u8>,
    /// Whether playing `mv` leaves the opponent with a full-board wildcard.
    pub next_wildcard: bool,
    /// Whether playing `mv` immediately wins the game for the current player.
    pub immediate_win: bool,
}

/// Complete Markov state of a Crosshair game.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Position {
    red: u64,
    blue: u64,
    side_to_move: Color,
    ply: u8,
    last_move: Option<Move>,
    last_flips: u64,
    result: GameResult,
    winning_line: u64,
    hash: u64,
}

impl Default for Position {
    fn default() -> Self {
        Self::start()
    }
}

impl Position {
    /// Returns the empty starting position at ply 0 with Red to move.
    pub const fn new() -> Self {
        Self::start()
    }

    /// Returns the empty starting position at ply 0 with Red to move.
    pub const fn start() -> Self {
        Self {
            red: 0,
            blue: 0,
            side_to_move: Color::Red,
            ply: 0,
            last_move: None,
            last_flips: 0,
            result: GameResult::Ongoing,
            winning_line: 0,
            hash: ZOBRIST.last_move[CELLS],
        }
    }

    /// Replays a sequence of moves from the starting position.
    pub fn from_moves(moves: &[Move]) -> Result<Self, MoveError> {
        let mut pos = Self::start();
        for &mv in moves {
            pos = pos.play(mv)?;
        }
        Ok(pos)
    }

    /// Returns the color to move next.
    pub const fn side_to_move(self) -> Color {
        self.side_to_move
    }

    /// Returns the number of plies played so far (`0..=49`).
    pub const fn ply(self) -> u8 {
        self.ply
    }

    /// Returns the current game result.
    pub const fn result(self) -> GameResult {
        self.result
    }

    /// Returns the most recently played move, or `None` at ply 0.
    pub const fn last_move(self) -> Option<Move> {
        self.last_move
    }

    /// Returns the cells flipped by the most recent move in ascending index order.
    pub const fn last_flips(self) -> MoveList {
        MoveList::from_mask(self.last_flips)
    }

    /// Returns the cells forming the shortest 4-orthogonal winning road (empty if ongoing).
    pub const fn winning_line(self) -> MoveList {
        MoveList::from_mask(self.winning_line)
    }

    /// Returns the 49-bit mask of Red stones.
    pub const fn red_bits(self) -> u64 {
        self.red
    }

    /// Returns the 49-bit mask of Blue stones.
    pub const fn blue_bits(self) -> u64 {
        self.blue
    }

    /// Returns the 49-bit mask of stones belonging to `color`.
    pub const fn stones_for(self, color: Color) -> u64 {
        match color {
            Color::Red => self.red,
            Color::Blue => self.blue,
        }
    }

    /// Returns the number of stones on the board for `color`.
    pub const fn count(self, color: Color) -> u8 {
        self.stones_for(color).count_ones() as u8
    }

    /// Returns `(red_count, blue_count)`.
    pub const fn counts(self) -> (u8, u8) {
        (self.red.count_ones() as u8, self.blue.count_ones() as u8)
    }

    /// Returns the stone color at `mv`, or `None` if the cell is empty.
    pub const fn color_at(self, mv: Move) -> Option<Color> {
        let bit = mv.bit();
        if (self.red & bit) != 0 {
            Some(Color::Red)
        } else if (self.blue & bit) != 0 {
            Some(Color::Blue)
        } else {
            None
        }
    }

    /// Returns the 64-bit Zobrist hash of this position.
    pub const fn hash(self) -> u64 {
        self.hash
    }

    /// Returns the active crosshair row (`0..7`), or `None` when wildcard is active or the game is over.
    pub const fn active_row(self) -> Option<u8> {
        if !matches!(self.result, GameResult::Ongoing) {
            return None;
        }
        let Some(last) = self.last_move else {
            return None;
        };
        let empty = !(self.red | self.blue) & BOARD_MASK;
        if (empty & CROSSHAIR_MASKS[last.index() as usize]) != 0 {
            Some(last.rank())
        } else {
            None
        }
    }

    /// Returns the active crosshair column (`0..7`), or `None` when wildcard is active or the game is over.
    pub const fn active_col(self) -> Option<u8> {
        if !matches!(self.result, GameResult::Ongoing) {
            return None;
        }
        let Some(last) = self.last_move else {
            return None;
        };
        let empty = !(self.red | self.blue) & BOARD_MASK;
        if (empty & CROSSHAIR_MASKS[last.index() as usize]) != 0 {
            Some(last.file())
        } else {
            None
        }
    }

    /// Returns `true` if the current player may play on any empty cell on the board.
    pub const fn is_wildcard(self) -> bool {
        if !matches!(self.result, GameResult::Ongoing) {
            return false;
        }
        let Some(last) = self.last_move else {
            return true;
        };
        let empty = !(self.red | self.blue) & BOARD_MASK;
        (empty & CROSSHAIR_MASKS[last.index() as usize]) == 0
    }

    /// Returns the legal moves in ascending cell index order (`0..49`).
    pub const fn legal_moves(self) -> MoveList {
        if !matches!(self.result, GameResult::Ongoing) {
            return MoveList(0);
        }
        let empty = !(self.red | self.blue) & BOARD_MASK;
        if let Some(last) = self.last_move {
            let constrained = empty & CROSSHAIR_MASKS[last.index() as usize];
            if constrained != 0 {
                return MoveList(constrained);
            }
        }
        MoveList(empty)
    }

    /// Computes all linear and 2×2 crosscut pinch flips that would occur if `side_to_move` plays at `mv`.
    pub const fn flips_for_move(self, mv: Move) -> MoveList {
        let (own, opp) = match self.side_to_move {
            Color::Red => (self.red, self.blue),
            Color::Blue => (self.blue, self.red),
        };
        MoveList(compute_flips(mv.index() as usize, own, opp))
    }

    /// Returns tactical and routing previews for every legal move in the position.
    pub fn move_previews(self) -> Vec<MovePreview> {
        let legal = self.legal_moves();
        let mut previews = Vec::with_capacity(legal.len());
        let side = self.side_to_move;
        for mv in legal {
            let child = self.play(mv).expect("legal move must succeed");
            let empty_after = !(child.red | child.blue) & BOARD_MASK;
            let constrained_after = empty_after & CROSSHAIR_MASKS[mv.index() as usize];
            let next_wildcard = constrained_after == 0;
            let (next_active_row, next_active_col) = if next_wildcard {
                (None, None)
            } else {
                (Some(mv.rank()), Some(mv.file()))
            };
            let immediate_win = matches!(
                child.result,
                GameResult::Win { winner, .. } if winner == side
            );
            previews.push(MovePreview {
                mv,
                flips: child.last_flips(),
                next_active_row,
                next_active_col,
                next_wildcard,
                immediate_win,
            });
        }
        previews
    }

    /// Plays `mv` and returns the resulting position, or an error if `mv` is illegal.
    pub fn play(self, mv: Move) -> Result<Self, MoveError> {
        if !matches!(self.result, GameResult::Ongoing) {
            return Err(MoveError::GameOver);
        }
        let occupied = self.red | self.blue;
        let bit = mv.bit();
        if (occupied & bit) != 0 {
            return Err(MoveError::CellOccupied);
        }
        let legal = self.legal_moves();
        if !legal.contains(mv) {
            return Err(MoveError::OutsideCrosshair);
        }

        let side = self.side_to_move;
        let (mut own, mut opp) = match side {
            Color::Red => (self.red, self.blue),
            Color::Blue => (self.blue, self.red),
        };

        let idx = mv.index() as usize;
        let flips = compute_flips(idx, own, opp);
        own |= bit | flips;
        opp &= !flips;

        let (red, blue) = match side {
            Color::Red => (own, opp),
            Color::Blue => (opp, own),
        };
        let new_ply = self.ply + 1;

        let winning_line = find_winning_road(own, side);
        let result = if winning_line != 0 {
            GameResult::Win {
                winner: side,
                reason: WinReason::Connection,
            }
        } else {
            GameResult::Ongoing
        };

        // Include last_move in the Zobrist key because the active crosshair row and column
        // depend on the previous move; identical stone bitboards reached via different last
        // moves have different legal move sets and are distinct Markov states.
        let mut hash = self.hash;
        let prev_last_idx = match self.last_move {
            Some(prev) => prev.index() as usize,
            None => CELLS,
        };
        hash ^= ZOBRIST.last_move[prev_last_idx];
        hash ^= ZOBRIST.last_move[idx];
        hash ^= ZOBRIST.side_to_move;
        hash ^= ZOBRIST.piece[side.index()][idx];
        let mut flip_bits = flips;
        while flip_bits != 0 {
            let flip_idx = flip_bits.trailing_zeros() as usize;
            flip_bits &= flip_bits - 1;
            hash ^= ZOBRIST.piece[0][flip_idx] ^ ZOBRIST.piece[1][flip_idx];
        }

        Ok(Self {
            red,
            blue,
            side_to_move: side.opponent(),
            ply: new_ply,
            last_move: Some(mv),
            last_flips: flips,
            result,
            winning_line,
            hash,
        })
    }
}

/// Computes Step A (4-orthogonal custodial pinch flips) followed by Step B (cascading 2×2 crosscut flips).
///
/// On a 4-orthogonal square grid, a 2×2 checkerboard (`R B / B R`) lets two diagonal pairs
/// cross without either color forming a 4-orthogonal connection, which would otherwise permit
/// drawn full boards. Flipping both opponent corners whenever a placed or flipped stone completes
/// a 2×2 diagonal eliminates every checkerboard deadlock and guarantees a winner on or before ply 49.
#[inline]
pub(crate) const fn compute_flips(idx: usize, mut own: u64, mut opp: u64) -> u64 {
    let placed_bit = 1_u64 << idx;
    own |= placed_bit;

    // Step A: 4-orthogonal linear custodial pinch
    let pairs = &PINCH_PAIRS[idx];
    let mut flips = 0_u64;
    let mut d = 0;
    while d < 4 {
        if let Some((adj, far)) = pairs[d] {
            let adj_bit = 1_u64 << adj;
            let far_bit = 1_u64 << far;
            if (opp & adj_bit) != 0 && (own & far_bit) != 0 {
                flips |= adj_bit;
            }
        }
        d += 1;
    }
    own |= flips;
    opp &= !flips;

    // Step B: 2x2 crosscut pinch (eliminates all R B / B R 2x2 deadlocks)
    let mut newly_changed = placed_bit | flips;
    while newly_changed != 0 {
        let c_idx = newly_changed.trailing_zeros() as usize;
        newly_changed &= newly_changed - 1;
        let quads = &CROSSCUT_QUADS[c_idx];
        let mut q = 0;
        while q < 4 {
            if let Some((diag_mate, ortho1, ortho2)) = quads[q] {
                let diag_bit = 1_u64 << diag_mate;
                let cc = (1_u64 << ortho1) | (1_u64 << ortho2);
                if (own & (1_u64 << c_idx)) != 0 && (own & diag_bit) != 0 && (opp & cc) == cc {
                    flips |= cc;
                    own |= cc;
                    opp &= !cc;
                    newly_changed |= cc;
                }
            }
            q += 1;
        }
    }
    flips
}

/// Expands `frontier` by one 4-orthogonal step restricted to `mask`.
#[inline]
pub(crate) const fn dilate_ortho(frontier: u64, mask: u64) -> u64 {
    let left = (frontier & NOT_FILE_A) >> 1;
    let right = (frontier & NOT_FILE_G) << 1;
    let down = frontier >> 7;
    let up = (frontier << 7) & BOARD_MASK;
    (frontier | left | right | down | up) & mask
}

/// Fast bitboard check returning `true` if `stones` connects `color`'s two opposite goal edges.
#[inline]
pub(crate) const fn has_winning_road(stones: u64, color: Color) -> bool {
    let (start_edge, target_edge) = match color {
        Color::Red => (TOP_EDGE_MASK, BOTTOM_EDGE_MASK),
        Color::Blue => (LEFT_EDGE_MASK, RIGHT_EDGE_MASK),
    };
    if (stones & start_edge) == 0 || (stones & target_edge) == 0 {
        return false;
    }
    let mut reached = stones & start_edge;
    loop {
        if (reached & target_edge) != 0 {
            return true;
        }
        let next = dilate_ortho(reached, stones);
        if next == reached {
            return false;
        }
        reached = next;
    }
}

/// Computes the shortest 4-orthogonal path connecting `color`'s goal edges within `stones`,
/// or `0` if no winning connection exists.
pub(crate) const fn find_winning_road(stones: u64, color: Color) -> u64 {
    let (start_edge, target_edge) = match color {
        Color::Red => (TOP_EDGE_MASK, BOTTOM_EDGE_MASK),
        Color::Blue => (LEFT_EDGE_MASK, RIGHT_EDGE_MASK),
    };
    let start = stones & start_edge;
    if start == 0 || (stones & target_edge) == 0 {
        return 0;
    }

    let mut layers = [0_u64; CELLS];
    layers[0] = start;
    let mut visited = start;
    let mut dist = 0_usize;

    while (layers[dist] & target_edge) == 0 {
        let expanded = dilate_ortho(layers[dist], stones) & !visited;
        if expanded == 0 {
            return 0;
        }
        dist += 1;
        layers[dist] = expanded;
        visited |= expanded;
    }

    // Backtrack from the first reached target-edge cell to reconstruct a shortest 4-orthogonal path
    let mut curr = (layers[dist] & target_edge).trailing_zeros() as usize;
    let mut path = 1_u64 << curr;
    while dist > 0 {
        dist -= 1;
        let candidates = ORTHO_NEIGHBORS[curr] & layers[dist];
        curr = candidates.trailing_zeros() as usize;
        path |= 1_u64 << curr;
    }
    path
}

pub(crate) const CROSSHAIR_MASKS: [u64; CELLS] = build_crosshair_masks();
pub(crate) const ORTHO_NEIGHBORS: [u64; CELLS] = build_ortho_neighbors();
const PINCH_PAIRS: [[Option<(u8, u8)>; 4]; CELLS] = build_pinch_pairs();
const CROSSCUT_QUADS: [[Option<(u8, u8, u8)>; 4]; CELLS] = build_crosscut_quads();

const fn build_crosshair_masks() -> [u64; CELLS] {
    let mut masks = [0_u64; CELLS];
    let mut idx = 0;
    while idx < CELLS {
        let file = idx % SIZE;
        let rank = idx / SIZE;
        let mut mask = 0_u64;
        let mut c = 0;
        while c < SIZE {
            mask |= 1_u64 << (rank * SIZE + c);
            mask |= 1_u64 << (c * SIZE + file);
            c += 1;
        }
        masks[idx] = mask;
        idx += 1;
    }
    masks
}

const fn build_ortho_neighbors() -> [u64; CELLS] {
    let mut table = [0_u64; CELLS];
    let mut idx = 0;
    while idx < CELLS {
        table[idx] = dilate_ortho(1_u64 << idx, BOARD_MASK) & !(1_u64 << idx);
        idx += 1;
    }
    table
}

const fn build_pinch_pairs() -> [[Option<(u8, u8)>; 4]; CELLS] {
    let dirs: [(i32, i32); 4] = [(0, -1), (-1, 0), (1, 0), (0, 1)];
    let mut table = [[None; 4]; CELLS];
    let mut idx = 0;
    while idx < CELLS {
        let file = (idx % SIZE) as i32;
        let rank = (idx / SIZE) as i32;
        let mut d = 0;
        while d < 4 {
            let (df, dr) = dirs[d];
            let adj_f = file + df;
            let adj_r = rank + dr;
            let far_f = file + 2 * df;
            let far_r = rank + 2 * dr;
            if adj_f >= 0
                && adj_f < SIZE as i32
                && adj_r >= 0
                && adj_r < SIZE as i32
                && far_f >= 0
                && far_f < SIZE as i32
                && far_r >= 0
                && far_r < SIZE as i32
            {
                let adj_idx = (adj_r as u8) * (SIZE as u8) + (adj_f as u8);
                let far_idx = (far_r as u8) * (SIZE as u8) + (far_f as u8);
                table[idx][d] = Some((adj_idx, far_idx));
            }
            d += 1;
        }
        idx += 1;
    }
    table
}

const fn build_crosscut_quads() -> [[Option<(u8, u8, u8)>; 4]; CELLS] {
    let diag_dirs: [(i32, i32); 4] = [(-1, -1), (1, -1), (-1, 1), (1, 1)];
    let mut table = [[None; 4]; CELLS];
    let mut idx = 0;
    while idx < CELLS {
        let file = (idx % SIZE) as i32;
        let rank = (idx / SIZE) as i32;
        let mut q = 0;
        while q < 4 {
            let (df, dr) = diag_dirs[q];
            let nf = file + df;
            let nr = rank + dr;
            if nf >= 0 && nf < SIZE as i32 && nr >= 0 && nr < SIZE as i32 {
                let diag_mate = (nr as u8) * (SIZE as u8) + (nf as u8);
                let ortho1 = (rank as u8) * (SIZE as u8) + (nf as u8);
                let ortho2 = (nr as u8) * (SIZE as u8) + (file as u8);
                table[idx][q] = Some((diag_mate, ortho1, ortho2));
            }
            q += 1;
        }
        idx += 1;
    }
    table
}

struct ZobristTable {
    piece: [[u64; CELLS]; 2],
    last_move: [u64; CELLS + 1],
    side_to_move: u64,
}

const ZOBRIST: ZobristTable = build_zobrist();

const fn build_zobrist() -> ZobristTable {
    let mut state = 0x5851_f42d_4c95_7f2d_u64;
    let mut piece = [[0_u64; CELLS]; 2];
    let mut c = 0;
    while c < 2 {
        let mut i = 0;
        while i < CELLS {
            state = splitmix64(state);
            piece[c][i] = state;
            i += 1;
        }
        c += 1;
    }
    let mut last_move = [0_u64; CELLS + 1];
    let mut i = 0;
    while i <= CELLS {
        state = splitmix64(state);
        last_move[i] = state;
        i += 1;
    }
    state = splitmix64(state);
    ZobristTable {
        piece,
        last_move,
        side_to_move: state,
    }
}

const fn splitmix64(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9e37_79b9_7f4a_7c15);
    let mut z = x;
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mv(coord: &str) -> Move {
        coord.parse().unwrap()
    }

    #[test]
    fn move_coordinates_round_trip_across_all_49_cells() {
        assert_eq!(mv("a1").index(), 0);
        assert_eq!(mv("g1").index(), 6);
        assert_eq!(mv("a7").index(), 42);
        assert_eq!(mv("g7").index(), 48);
        for index in 0..49_u8 {
            let parsed = Move::from_index(index);
            assert_eq!(parsed.to_string().parse::<Move>(), Ok(parsed));
        }
        assert!("h1".parse::<Move>().is_err());
        assert!("a8".parse::<Move>().is_err());
        assert!("a0".parse::<Move>().is_err());
    }

    #[test]
    fn opening_position_exposes_all_49_cells_and_routes_crosshair() {
        let start = Position::start();
        assert_eq!(start.legal_moves().len(), 49);
        assert!(start.is_wildcard());
        assert_eq!(start.active_row(), None);
        assert_eq!(start.active_col(), None);

        let after_d4 = start.play(mv("d4")).unwrap();
        assert_eq!(after_d4.side_to_move(), Color::Blue);
        assert_eq!(after_d4.active_row(), Some(3));
        assert_eq!(after_d4.active_col(), Some(3));
        assert!(!after_d4.is_wildcard());
        assert_eq!(after_d4.legal_moves().len(), 12);
        assert!(after_d4.legal_moves().contains(mv("d1")));
        assert!(after_d4.legal_moves().contains(mv("a4")));
        assert!(!after_d4.legal_moves().contains(mv("d4")));
        assert_eq!(after_d4.play(mv("a1")), Err(MoveError::OutsideCrosshair));
    }

    #[test]
    fn linear_and_crosscut_pinch_flips_work_as_specified() {
        // 1. Linear pinch: R a1, B b1, R c1 -> flips b1 to R
        let linear = Position::from_moves(&[mv("a1"), mv("b1"), mv("c1")]).unwrap();
        assert_eq!(linear.color_at(mv("b1")), Some(Color::Red));
        assert_eq!(linear.counts(), (3, 0));
        assert_eq!(
            linear.last_flips().iter().collect::<Vec<_>>(),
            vec![mv("b1")]
        );

        // 2. 2x2 Crosscut pinch:
        // R plays a1, B plays b1, R plays b2, B plays a2 -> B has b1 & a2 (diagonal), R has a1 & b2 (diagonal)!
        // When B plays a2, B completes the 2x2 crosscut with b1 and flips BOTH a1 and b2 to Blue!
        let crosscut = Position::from_moves(&[mv("a1"), mv("b1"), mv("b2"), mv("a2")]).unwrap();
        assert_eq!(crosscut.color_at(mv("a1")), Some(Color::Blue));
        assert_eq!(crosscut.color_at(mv("b1")), Some(Color::Blue));
        assert_eq!(crosscut.color_at(mv("a2")), Some(Color::Blue));
        assert_eq!(crosscut.color_at(mv("b2")), Some(Color::Blue));
        assert_eq!(crosscut.counts(), (0, 4));
        assert_eq!(
            crosscut.last_flips().iter().collect::<Vec<_>>(),
            vec![mv("a1"), mv("b2")]
        );
    }

    #[test]
    fn orthogonal_edge_to_edge_connection_wins_immediately() {
        // Red builds a vertical column on file d from d1 (rank 0) to d7 (rank 6)
        // 1. R d1, 2. B a1, 3. R a2, 4. B d2, 5. R d3 (pinches d2 to R! Now R has d1, d2, d3),
        // 6. B a3, 7. R a4, 8. B d4, 9. R d5 (pinches d4 to R! Now R has d1..d5),
        // 10. B a5, 11. R a6, 12. B d6, 13. R d7 (pinches d6 to R! Now R has d1..d7 connecting Top to Bottom!)
        let pos = Position::from_moves(&[
            mv("d1"),
            mv("a1"),
            mv("a2"),
            mv("d2"),
            mv("d3"),
            mv("a3"),
            mv("a4"),
            mv("d4"),
            mv("d5"),
            mv("a5"),
            mv("a6"),
            mv("d6"),
            mv("d7"),
        ])
        .unwrap();
        assert_eq!(
            pos.result(),
            GameResult::Win {
                winner: Color::Red,
                reason: WinReason::Connection,
            }
        );
        assert_eq!(
            pos.winning_line().iter().collect::<Vec<_>>(),
            vec![
                mv("d1"),
                mv("d2"),
                mv("d3"),
                mv("d4"),
                mv("d5"),
                mv("d6"),
                mv("d7")
            ]
        );
        assert!(pos.legal_moves().is_empty());
    }
}
