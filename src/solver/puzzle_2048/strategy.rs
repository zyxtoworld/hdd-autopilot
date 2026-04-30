use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use super::board::{
    DEFAULT_DIRECTIONS, Direction, MoveOutcome, apply_move, empty_cells, legal_moves, max_tile,
    normalize_board, tile_rank,
};

const WIN_SCORE: f64 = 1_000_000_000.0;
const LOSS_SCORE: f64 = -1_000_000_000.0;
const MAX_CELLS: usize = 25;

static EXACT_3X3_CACHE: OnceLock<Mutex<HashMap<ExactCacheKey, f64>>> = OnceLock::new();
static PACKED_EXACT_3X3_CACHE: OnceLock<Mutex<HashMap<PackedExactCacheKey, f64>>> = OnceLock::new();

pub(super) fn choose_next_move(
    board: &[Vec<i32>],
    target_tile: i32,
    four_ratio: f64,
    allowed_directions: &[Direction],
) -> Option<Direction> {
    choose_next_move_inner(board, target_tile, four_ratio, allowed_directions, true)
}

pub(super) fn choose_next_move_fast(
    board: &[Vec<i32>],
    target_tile: i32,
    four_ratio: f64,
    allowed_directions: &[Direction],
) -> Option<Direction> {
    choose_next_move_inner(board, target_tile, four_ratio, allowed_directions, false)
}

fn choose_next_move_inner(
    board: &[Vec<i32>],
    target_tile: i32,
    four_ratio: f64,
    allowed_directions: &[Direction],
    allow_strong_3x3: bool,
) -> Option<Direction> {
    let board = normalize_board(board)?;
    if target_tile > 0 && max_tile(&board) >= target_tile {
        return None;
    }
    let directions = normalized_directions(allowed_directions);
    let legal = legal_moves(&board, &directions);
    if legal.is_empty() {
        return None;
    }
    for direction in &legal {
        let outcome = apply_move(&board, *direction);
        if target_tile > 0 && max_tile(&outcome.board) >= target_tile {
            return Some(*direction);
        }
    }

    if let Some(direction) = choose_fast_3x3_move(
        &board,
        target_tile,
        four_ratio,
        &directions,
        allow_strong_3x3,
    ) {
        return Some(direction);
    }
    if let Some(direction) = choose_exact_3x3_move(&board, target_tile, four_ratio, &directions) {
        return Some(direction);
    }

    let legal = preferred_legal_moves(&board, legal);
    if let Some(direction) =
        choose_rollout_3x3_move(&board, target_tile, four_ratio, &directions, &legal)
    {
        return Some(direction);
    }
    let depth = search_depth(&board, target_tile, allow_strong_3x3);
    let mut search = SearchContext::new(
        board.len(),
        target_tile,
        four_ratio,
        directions,
        allow_strong_3x3,
    );
    let mut best_direction = None;
    let mut best_score = f64::NEG_INFINITY;

    for direction in legal {
        let outcome = apply_move(&board, direction);
        if search.has_reached_target(&outcome.board) {
            return Some(direction);
        }
        let score = search.expectimax_chance(&outcome.board, depth.saturating_sub(1))
            + f64::from(outcome.score_delta) * 0.001;
        if score > best_score {
            best_score = score;
            best_direction = Some(direction);
        }
    }

    best_direction
}

fn preferred_legal_moves(board: &[Vec<i32>], legal: Vec<Direction>) -> Vec<Direction> {
    if board.len() != 3 || legal.len() <= 1 {
        return legal;
    }

    let max_value = max_tile(board);
    if max_value < 128 {
        return legal;
    }
    let pinned = max_tile_corner_cells(board, max_value);
    let preferred = legal
        .iter()
        .copied()
        .filter(|direction| {
            let outcome = apply_move(board, *direction);
            if !pinned.is_empty() {
                return pinned
                    .iter()
                    .any(|(row, col)| outcome.board[*row][*col] == max_value);
            }
            !max_tile_corner_cells(&outcome.board, max_value).is_empty()
        })
        .collect::<Vec<_>>();

    if preferred.is_empty() {
        legal
    } else {
        preferred
    }
}

fn max_tile_corner_cells(board: &[Vec<i32>], max_value: i32) -> Vec<(usize, usize)> {
    let size = board.len();
    [(0, 0), (0, size - 1), (size - 1, 0), (size - 1, size - 1)]
        .into_iter()
        .filter(|(row, col)| board[*row][*col] == max_value)
        .collect()
}

fn choose_fast_3x3_move(
    board: &[Vec<i32>],
    target_tile: i32,
    four_ratio: f64,
    allowed_directions: &[Direction],
    allow_strong: bool,
) -> Option<Direction> {
    if board.len() != 3 || target_tile <= 0 {
        return None;
    }

    let packed = Packed3x3Board::from_board(board)?;
    let directions = normalized_directions(allowed_directions);
    let legal = packed.legal_moves(&directions);
    if legal.is_empty() {
        return None;
    }

    if allow_strong
        && let Some(direction) =
            choose_packed_exact_3x3_move(packed, target_tile, four_ratio, &directions, &legal)
    {
        return Some(direction);
    }

    let depth = env_usize("PUZZLE_2048_3X3_FAST_DEPTH").unwrap_or(7).max(1);
    let mut search = Packed3x3Search::new(target_tile, four_ratio, directions);
    let mut best_direction = None;
    let mut best_score = f64::NEG_INFINITY;

    for direction in legal {
        let outcome = packed.apply_move(direction);
        if outcome.board.max_rank() >= search.target_rank {
            return Some(direction);
        }
        let score = search.chance_value(outcome.board, depth.saturating_sub(1))
            + f64::from(outcome.score_delta) * 0.001;
        if score > best_score {
            best_score = score;
            best_direction = Some(direction);
        }
    }

    best_direction
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct Packed3x3Board(u64);

#[derive(Debug, Clone, Copy)]
struct PackedMoveOutcome {
    board: Packed3x3Board,
    score_delta: i32,
    moved: bool,
}

impl Packed3x3Board {
    fn from_board(board: &[Vec<i32>]) -> Option<Self> {
        if board.len() != 3 || board.iter().any(|row| row.len() != 3) {
            return None;
        }

        let mut packed = 0_u64;
        for (index, value) in board.iter().flatten().enumerate() {
            let rank = rank_u8(*value);
            if rank > 15 {
                return None;
            }
            packed |= u64::from(rank) << (index * 4);
        }
        Some(Self(packed))
    }

    fn get(self, index: usize) -> u8 {
        ((self.0 >> (index * 4)) & 0x0f) as u8
    }

    fn with_rank(self, index: usize, rank: u8) -> Self {
        let shift = index * 4;
        Self((self.0 & !(0x0f_u64 << shift)) | (u64::from(rank.min(15)) << shift))
    }

    fn set_rank(raw: &mut u64, index: usize, rank: u8) {
        *raw |= u64::from(rank.min(15)) << (index * 4);
    }

    fn max_rank(self) -> u8 {
        (0..9).map(|index| self.get(index)).max().unwrap_or(0)
    }

    fn empty_indices(self) -> Vec<usize> {
        (0..9).filter(|index| self.get(*index) == 0).collect()
    }

    fn legal_moves(self, directions: &[Direction]) -> Vec<Direction> {
        directions
            .iter()
            .copied()
            .filter(|direction| self.apply_move(*direction).moved)
            .collect()
    }

    fn apply_move(self, direction: Direction) -> PackedMoveOutcome {
        const LEFT: [[usize; 3]; 3] = [[0, 1, 2], [3, 4, 5], [6, 7, 8]];
        const RIGHT: [[usize; 3]; 3] = [[2, 1, 0], [5, 4, 3], [8, 7, 6]];
        const UP: [[usize; 3]; 3] = [[0, 3, 6], [1, 4, 7], [2, 5, 8]];
        const DOWN: [[usize; 3]; 3] = [[6, 3, 0], [7, 4, 1], [8, 5, 2]];

        let lines = match direction {
            Direction::Left => LEFT,
            Direction::Right => RIGHT,
            Direction::Up => UP,
            Direction::Down => DOWN,
        };
        let mut raw = 0_u64;
        let mut score_delta = 0;
        for line in lines {
            let source = [self.get(line[0]), self.get(line[1]), self.get(line[2])];
            let (merged, score) = merge_rank_line(source);
            score_delta += score;
            for (offset, index) in line.into_iter().enumerate() {
                Self::set_rank(&mut raw, index, merged[offset]);
            }
        }
        let board = Self(raw);
        PackedMoveOutcome {
            board,
            score_delta,
            moved: board != self,
        }
    }
}

fn merge_rank_line(source: [u8; 3]) -> ([u8; 3], i32) {
    let mut compacted = [0_u8; 3];
    let mut len = 0;
    for rank in source {
        if rank > 0 {
            compacted[len] = rank;
            len += 1;
        }
    }

    let mut merged = [0_u8; 3];
    let mut write = 0;
    let mut read = 0;
    let mut score = 0;
    while read < len {
        if read + 1 < len && compacted[read] == compacted[read + 1] {
            let rank = compacted[read].saturating_add(1).min(15);
            merged[write] = rank;
            score += 1_i32.checked_shl(u32::from(rank)).unwrap_or(i32::MAX);
            read += 2;
        } else {
            merged[write] = compacted[read];
            read += 1;
        }
        write += 1;
    }
    (merged, score)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct PackedSearchKey {
    board: Packed3x3Board,
    depth: u8,
}

#[derive(Debug)]
struct Packed3x3Search {
    target_rank: u8,
    four_ratio: f64,
    allowed_directions: Vec<Direction>,
    max_cache: HashMap<PackedSearchKey, f64>,
    chance_cache: HashMap<PackedSearchKey, f64>,
}

impl Packed3x3Search {
    fn new(target_tile: i32, four_ratio: f64, allowed_directions: Vec<Direction>) -> Self {
        let four_ratio = if four_ratio.is_finite() {
            four_ratio.clamp(0.0, 1.0)
        } else {
            0.1
        };
        Self {
            target_rank: rank_u8(target_tile),
            four_ratio,
            allowed_directions,
            max_cache: HashMap::new(),
            chance_cache: HashMap::new(),
        }
    }

    fn max_value(&mut self, board: Packed3x3Board, depth: usize) -> f64 {
        if board.max_rank() >= self.target_rank {
            return WIN_SCORE + packed_3x3_score(board);
        }
        if depth == 0 {
            return packed_3x3_score(board);
        }

        let key = PackedSearchKey {
            board,
            depth: depth.min(u8::MAX as usize) as u8,
        };
        if let Some(score) = self.max_cache.get(&key) {
            return *score;
        }

        let moves = board.legal_moves(&self.allowed_directions);
        let score = if moves.is_empty() {
            LOSS_SCORE + packed_3x3_score(board)
        } else {
            moves
                .into_iter()
                .map(|direction| {
                    let outcome = board.apply_move(direction);
                    self.chance_value(outcome.board, depth.saturating_sub(1))
                        + f64::from(outcome.score_delta) * 0.001
                })
                .fold(f64::NEG_INFINITY, f64::max)
        };
        self.max_cache.insert(key, score);
        score
    }

    fn chance_value(&mut self, board: Packed3x3Board, depth: usize) -> f64 {
        if board.max_rank() >= self.target_rank {
            return WIN_SCORE + packed_3x3_score(board);
        }
        if depth == 0 {
            return packed_3x3_score(board);
        }

        let key = PackedSearchKey {
            board,
            depth: depth.min(u8::MAX as usize) as u8,
        };
        if let Some(score) = self.chance_cache.get(&key) {
            return *score;
        }

        let empties = board.empty_indices();
        let score = if empties.is_empty() {
            self.max_value(board, depth.saturating_sub(1))
        } else {
            let two_ratio = 1.0 - self.four_ratio;
            let mut total = 0.0;
            for index in &empties {
                total += two_ratio * self.max_value(board.with_rank(*index, 1), depth - 1);
                total += self.four_ratio * self.max_value(board.with_rank(*index, 2), depth - 1);
            }
            total / empties.len() as f64
        };
        self.chance_cache.insert(key, score);
        score
    }
}

fn choose_packed_exact_3x3_move(
    board: Packed3x3Board,
    target_tile: i32,
    four_ratio: f64,
    directions: &[Direction],
    legal: &[Direction],
) -> Option<Direction> {
    let target_rank = rank_u8(target_tile);
    let exact_min_rank = env_usize("PUZZLE_2048_3X3_EXACT_MIN_RANK")
        .map(|rank| rank.min(u8::MAX as usize) as u8)
        .unwrap_or(0);
    if target_rank == 0 || board.max_rank() < exact_min_rank {
        return None;
    }

    let budget = env_usize("PUZZLE_2048_3X3_EXACT_BUDGET").unwrap_or(500_000);
    if budget == 0 {
        return None;
    }

    let mut search = PackedExact3x3Search::new(target_rank, four_ratio, directions, budget);
    let mut best_direction = None;
    let mut best_value = f64::NEG_INFINITY;
    for direction in legal {
        let outcome = board.apply_move(*direction);
        if outcome.board.max_rank() >= target_rank {
            return Some(*direction);
        }
        let value = search.chance_value(outcome.board)?;
        if value > best_value {
            best_value = value;
            best_direction = Some(*direction);
        }
    }
    best_direction
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct PackedExactCacheKey {
    board: Packed3x3Board,
    target_rank: u8,
    four_ratio: u16,
    directions_mask: u8,
}

fn packed_exact_cache() -> &'static Mutex<HashMap<PackedExactCacheKey, f64>> {
    PACKED_EXACT_3X3_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

#[derive(Debug)]
struct PackedExact3x3Search {
    target_rank: u8,
    four_ratio: f64,
    four_ratio_key: u16,
    directions_mask: u8,
    directions: Vec<Direction>,
    cache: HashMap<Packed3x3Board, f64>,
    visited: usize,
    budget: usize,
}

impl PackedExact3x3Search {
    fn new(target_rank: u8, four_ratio: f64, directions: &[Direction], budget: usize) -> Self {
        let four_ratio = if four_ratio.is_finite() {
            four_ratio.clamp(0.0, 1.0)
        } else {
            0.1
        };
        Self {
            target_rank,
            four_ratio,
            four_ratio_key: (four_ratio * 10_000.0).round().clamp(0.0, 10_000.0) as u16,
            directions_mask: direction_mask(directions),
            directions: normalized_directions(directions),
            cache: HashMap::new(),
            visited: 0,
            budget: budget.max(1),
        }
    }

    fn value(&mut self, board: Packed3x3Board) -> Option<f64> {
        if board.max_rank() >= self.target_rank {
            return Some(1.0);
        }
        if board.legal_moves(&self.directions).is_empty() {
            return Some(0.0);
        }
        if self.visited >= self.budget {
            return None;
        }
        if let Some(value) = self.cache.get(&board) {
            return Some(*value);
        }

        let key = PackedExactCacheKey {
            board,
            target_rank: self.target_rank,
            four_ratio: self.four_ratio_key,
            directions_mask: self.directions_mask,
        };
        if let Some(value) = packed_exact_cache().lock().unwrap().get(&key).copied() {
            self.cache.insert(board, value);
            return Some(value);
        }

        self.visited += 1;
        let mut best = 0.0;
        for direction in board.legal_moves(&self.directions) {
            let outcome = board.apply_move(direction);
            let value = if outcome.board.max_rank() >= self.target_rank {
                1.0
            } else {
                self.chance_value(outcome.board)?
            };
            if value > best {
                best = value;
            }
        }
        self.cache.insert(board, best);
        packed_exact_cache().lock().unwrap().insert(key, best);
        Some(best)
    }

    fn chance_value(&mut self, board: Packed3x3Board) -> Option<f64> {
        let empties = board.empty_indices();
        if empties.is_empty() {
            return self.value(board);
        }

        let two_ratio = 1.0 - self.four_ratio;
        let mut total = 0.0;
        for index in &empties {
            total += two_ratio * self.value(board.with_rank(*index, 1))?;
            total += self.four_ratio * self.value(board.with_rank(*index, 2))?;
        }
        Some(total / empties.len() as f64)
    }
}

fn packed_3x3_score(board: Packed3x3Board) -> f64 {
    let ranks = [
        board.get(0),
        board.get(1),
        board.get(2),
        board.get(3),
        board.get(4),
        board.get(5),
        board.get(6),
        board.get(7),
        board.get(8),
    ];
    let empty = ranks.iter().filter(|rank| **rank == 0).count() as f64;
    let max_rank = f64::from(*ranks.iter().max().unwrap_or(&0));
    let mobility = board.legal_moves(DEFAULT_DIRECTIONS).len() as f64;
    packed_corner_core_score(&ranks)
        + packed_corner_order_score(&ranks) * 0.45
        + packed_monotonicity(&ranks) * 45.0
        + packed_smoothness(&ranks) * 22.0
        + packed_merge_potential(&ranks) * 520.0
        + small_board_empty_score(empty as usize)
        + mobility * 2_200.0
        + max_rank * max_rank * 180.0
        + packed_top_tile_score(&ranks)
}

fn packed_top_tile_score(ranks: &[u8; 9]) -> f64 {
    let mut sorted = *ranks;
    sorted.sort_unstable_by(|left, right| right.cmp(left));
    f64::from(sorted[0]) * 65.0 + f64::from(sorted[1] + sorted[2]) * 18.0
}

fn packed_corner_core_score(ranks: &[u8; 9]) -> f64 {
    const CORES: [[usize; 4]; 4] = [[0, 1, 3, 4], [2, 1, 5, 4], [6, 3, 7, 4], [8, 5, 7, 4]];
    let mut sorted = *ranks;
    sorted.sort_unstable_by(|left, right| right.cmp(left));

    CORES
        .iter()
        .map(|core| {
            let corner = f64::from(ranks[core[0]]);
            let edge_a = f64::from(ranks[core[1]]);
            let edge_b = f64::from(ranks[core[2]]);
            let center = f64::from(ranks[core[3]]);
            let edge_high = edge_a.max(edge_b);
            let edge_low = edge_a.min(edge_b);
            let mut top_coverage = 0.0;
            for rank in sorted.iter().take(4).copied().filter(|rank| *rank > 0) {
                if core.iter().any(|index| ranks[*index] == rank) {
                    top_coverage += 1.0;
                }
            }

            let mut builder = 0.0;
            for (index, rank) in ranks.iter().enumerate() {
                if core.contains(&index) || *rank == 0 {
                    continue;
                }
                let rank = f64::from(*rank);
                if center > 0.0 && (rank - center).abs() < f64::EPSILON {
                    builder += 3_200.0;
                } else if center > 1.0 && (rank + 1.0 - center).abs() < f64::EPSILON {
                    builder += 1_100.0;
                }
            }

            let order_penalty = (edge_high - corner).max(0.0).powi(2) * 18_000.0
                + (center - edge_low).max(0.0).powi(2) * 18_000.0
                + (edge_high - edge_low - 2.0).max(0.0).powi(2) * 1_800.0;
            top_coverage * 16_000.0
                + corner * 13_000.0
                + edge_high * 6_000.0
                + edge_low * 5_200.0
                + center * 4_600.0
                + builder
                - order_penalty
        })
        .fold(f64::NEG_INFINITY, f64::max)
}

fn packed_corner_order_score(ranks: &[u8; 9]) -> f64 {
    const PATHS: [[usize; 9]; 8] = [
        [0, 3, 1, 4, 2, 5, 6, 7, 8],
        [2, 5, 1, 4, 0, 3, 8, 7, 6],
        [6, 3, 7, 4, 8, 5, 0, 1, 2],
        [8, 5, 7, 4, 6, 3, 2, 1, 0],
        [0, 1, 3, 4, 6, 7, 2, 5, 8],
        [2, 1, 5, 4, 8, 7, 0, 3, 6],
        [6, 7, 3, 4, 0, 1, 8, 5, 2],
        [8, 7, 5, 4, 2, 1, 6, 3, 0],
    ];
    PATHS
        .iter()
        .map(|path| {
            let mut weighted = 0.0;
            let mut inversion_penalty = 0.0;
            for (position, index) in path.iter().enumerate() {
                weighted += f64::from(ranks[*index]) * 1.7_f64.powi((8 - position) as i32) * 95.0;
                if position > 0 {
                    let previous = f64::from(ranks[path[position - 1]]);
                    let current = f64::from(ranks[*index]);
                    inversion_penalty += (current - previous).max(0.0).powi(2) * 2_200.0;
                }
            }
            weighted - inversion_penalty
        })
        .fold(f64::NEG_INFINITY, f64::max)
}

fn packed_monotonicity(ranks: &[u8; 9]) -> f64 {
    let mut left = 0.0;
    let mut right = 0.0;
    let mut up = 0.0;
    let mut down = 0.0;
    for row in 0..3 {
        for col in 0..2 {
            let current = f64::from(ranks[row * 3 + col]);
            let next = f64::from(ranks[row * 3 + col + 1]);
            if current > next {
                left += next - current;
            } else {
                right += current - next;
            }
        }
    }
    for row in 0..2 {
        for col in 0..3 {
            let current = f64::from(ranks[row * 3 + col]);
            let next = f64::from(ranks[(row + 1) * 3 + col]);
            if current > next {
                up += next - current;
            } else {
                down += current - next;
            }
        }
    }
    left.max(right) + up.max(down)
}

fn packed_smoothness(ranks: &[u8; 9]) -> f64 {
    let mut score = 0.0;
    for row in 0..3 {
        for col in 0..3 {
            let index = row * 3 + col;
            let rank = ranks[index];
            if rank == 0 {
                continue;
            }
            if col + 1 < 3 && ranks[index + 1] > 0 {
                score -= (f64::from(rank) - f64::from(ranks[index + 1])).abs();
            }
            if row + 1 < 3 && ranks[index + 3] > 0 {
                score -= (f64::from(rank) - f64::from(ranks[index + 3])).abs();
            }
        }
    }
    score
}

fn packed_merge_potential(ranks: &[u8; 9]) -> f64 {
    const LINES: [[usize; 3]; 6] = [
        [0, 1, 2],
        [3, 4, 5],
        [6, 7, 8],
        [0, 3, 6],
        [1, 4, 7],
        [2, 5, 8],
    ];
    let mut score = 0.0;
    for line in LINES {
        let compacted = line
            .into_iter()
            .filter_map(|index| (ranks[index] > 0).then_some(ranks[index]))
            .collect::<Vec<_>>();
        for pair in compacted.windows(2) {
            if pair[0] == pair[1] {
                score += f64::from(pair[0]).powi(2);
            }
        }
    }
    score
}

fn choose_rollout_3x3_move(
    board: &[Vec<i32>],
    target_tile: i32,
    four_ratio: f64,
    allowed_directions: &[Direction],
    legal: &[Direction],
) -> Option<Direction> {
    if board.len() != 3 || target_tile <= 0 {
        return None;
    }

    let rollouts = env_usize("PUZZLE_2048_3X3_ROLLOUTS").unwrap_or(0);
    if rollouts == 0 {
        return None;
    }

    let mut best_direction = None;
    let mut best_score = f64::NEG_INFINITY;
    for direction in legal {
        let outcome = apply_move(board, *direction);
        let mut total = 0.0;
        for index in 0..rollouts {
            let seed = rollout_seed(&outcome.board, *direction, index);
            total += rollout_score_3x3(
                outcome.board.clone(),
                target_tile,
                four_ratio,
                allowed_directions,
                seed,
            );
        }
        let score = total / rollouts as f64 + f64::from(outcome.score_delta) * 0.01;
        if score > best_score {
            best_score = score;
            best_direction = Some(*direction);
        }
    }
    best_direction
}

fn rollout_score_3x3(
    mut board: Vec<Vec<i32>>,
    target_tile: i32,
    four_ratio: f64,
    allowed_directions: &[Direction],
    seed: u64,
) -> f64 {
    let mut rng = RolloutRng::new(seed);
    let four_ratio = if four_ratio.is_finite() {
        four_ratio.clamp(0.0, 1.0)
    } else {
        0.1
    };
    for step in 0..420 {
        let current_max = max_tile(&board);
        if current_max >= target_tile {
            return 1_000_000.0 - step as f64;
        }
        if !rollout_spawn(&mut board, &mut rng, four_ratio) {
            return rollout_static_score_3x3(&board, allowed_directions);
        }

        let legal = legal_moves(&board, allowed_directions);
        if legal.is_empty() {
            return rollout_static_score_3x3(&board, allowed_directions);
        }
        let Some(direction) = greedy_rollout_move_3x3(&board, allowed_directions, &legal) else {
            return rollout_static_score_3x3(&board, allowed_directions);
        };
        let outcome = apply_move(&board, direction);
        if !outcome.moved {
            return rollout_static_score_3x3(&board, allowed_directions);
        }
        board = outcome.board;
    }

    rollout_static_score_3x3(&board, allowed_directions)
}

fn greedy_rollout_move_3x3(
    board: &[Vec<i32>],
    allowed_directions: &[Direction],
    legal: &[Direction],
) -> Option<Direction> {
    let legal = preferred_legal_moves(board, legal.to_vec());
    let mut best_direction = None;
    let mut best_score = f64::NEG_INFINITY;
    for direction in legal {
        let outcome = apply_move(board, direction);
        let score = rollout_static_score_3x3(&outcome.board, allowed_directions)
            + f64::from(outcome.score_delta) * 0.02;
        if score > best_score {
            best_score = score;
            best_direction = Some(direction);
        }
    }
    best_direction
}

fn rollout_static_score_3x3(board: &[Vec<i32>], allowed_directions: &[Direction]) -> f64 {
    let empty_count = empty_cells(board).len();
    let max_value = max_tile(board);
    let max_rank = tile_rank(max_value.max(1));
    corner_core_score(board)
        + corner_cluster_score(board) * 0.35
        + corner_order_score(board) * 0.35
        + monotonicity(board) * 45.0
        + smoothness(board) * 22.0
        + merge_potential(board) * 520.0
        + small_board_empty_score(empty_count)
        + max_tile_placement(board, max_value) * 120.0
        + max_rank * max_rank * 180.0
        + tile_growth_score(3, board, max_value)
        + legal_moves(board, allowed_directions).len() as f64 * 150.0
}

fn rollout_spawn(board: &mut [Vec<i32>], rng: &mut RolloutRng, four_ratio: f64) -> bool {
    let empties = empty_cells(board);
    if empties.is_empty() {
        return false;
    }
    let (row, col) = empties[rng.index(empties.len())];
    board[row][col] = if rng.chance(four_ratio) { 4 } else { 2 };
    true
}

fn rollout_seed(board: &[Vec<i32>], direction: Direction, index: usize) -> u64 {
    let mut seed = 0x9e37_79b9_7f4a_7c15_u64 ^ index as u64;
    seed ^= match direction {
        Direction::Up => 0x10,
        Direction::Down => 0x20,
        Direction::Left => 0x30,
        Direction::Right => 0x40,
    };
    for value in board.iter().flatten() {
        seed = seed.rotate_left(7) ^ (*value as u64).wrapping_mul(0x1000_0000_01b3);
    }
    seed
}

#[derive(Debug, Clone, Copy)]
struct RolloutRng(u64);

impl RolloutRng {
    fn new(seed: u64) -> Self {
        Self(seed)
    }

    fn next_u64(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0
    }

    fn index(&mut self, len: usize) -> usize {
        (self.next_u64() as usize) % len
    }

    fn chance(&mut self, probability: f64) -> bool {
        (self.next_u64() % 1_000_000) as f64 / 1_000_000.0 < probability
    }
}

fn normalized_directions(allowed_directions: &[Direction]) -> Vec<Direction> {
    let source = if allowed_directions.is_empty() {
        DEFAULT_DIRECTIONS
    } else {
        allowed_directions
    };
    let mut directions = Vec::with_capacity(source.len());
    for direction in source {
        if !directions.contains(direction) {
            directions.push(*direction);
        }
    }
    directions
}

fn choose_exact_3x3_move(
    board: &[Vec<i32>],
    target_tile: i32,
    four_ratio: f64,
    allowed_directions: &[Direction],
) -> Option<Direction> {
    if board.len() != 3 || target_tile <= 0 || max_tile(board) < target_tile / 2 {
        return None;
    }

    let limit = env_usize("PUZZLE_2048_3X3_EXACT_LIMIT").unwrap_or(0);
    if limit == 0 {
        return None;
    }
    let mut solver = Exact3x3Solver::new(target_tile, four_ratio, allowed_directions, limit);
    solver.choose(board)
}

fn search_depth(board: &[Vec<i32>], target_tile: i32, strong: bool) -> usize {
    let size = board.len();
    let env_key = match size {
        3 => Some("DEPTH_3"),
        4 => Some("DEPTH_4"),
        5 => Some("DEPTH_5"),
        _ => None,
    };
    if let Some(depth) = env_key
        .and_then(env_usize)
        .or_else(|| env_usize(&format!("PUZZLE_2048_DEPTH_{size}")))
    {
        return depth.max(1);
    }

    let empty_count = empty_cells(board).len();
    let max_rank = tile_rank(max_tile(board));
    let target_rank = if target_tile > 0 {
        tile_rank(target_tile)
    } else {
        f64::INFINITY
    };

    let base_depth = if strong {
        match size {
            3 if empty_count <= 2 => 7,
            3 if empty_count <= 5 => 6,
            3 => 5,
            4 if empty_count <= 4 => 6,
            4 if empty_count <= 8 => 5,
            4 => 4,
            5 if empty_count <= 5 => 5,
            5 if empty_count <= 12 => 4,
            5 => 3,
            _ => 5,
        }
    } else {
        match size {
            3 if empty_count <= 2 => 6,
            3 if empty_count <= 5 => 5,
            3 => 4,
            4 if empty_count <= 6 => 4,
            4 => 3,
            5 => 2,
            _ => 4,
        }
    };

    if size == 3 && target_tile > 0 && max_rank + 2.0 >= target_rank {
        base_depth + 1
    } else {
        base_depth
    }
}

#[derive(Debug, Clone)]
struct SearchContext {
    size: usize,
    target_tile: i32,
    four_ratio: f64,
    allowed_directions: Vec<Direction>,
    gradients: Vec<Vec<f64>>,
    max_cache: HashMap<SearchKey, f64>,
    chance_cache: HashMap<SearchKey, f64>,
    chance_cell_limit: Option<usize>,
    strong: bool,
}

impl SearchContext {
    fn new(
        size: usize,
        target_tile: i32,
        four_ratio: f64,
        allowed_directions: Vec<Direction>,
        strong: bool,
    ) -> Self {
        let four_ratio = if four_ratio.is_finite() {
            four_ratio.clamp(0.0, 1.0)
        } else {
            0.1
        };
        Self {
            size,
            target_tile,
            four_ratio,
            allowed_directions,
            gradients: snake_gradients(size),
            max_cache: HashMap::new(),
            chance_cache: HashMap::new(),
            chance_cell_limit: env_usize("PUZZLE_2048_CHANCE_LIMIT")
                .or_else(|| env_usize("CHANCE_SAMPLE_LIMIT"))
                .map(|limit| limit.max(1)),
            strong,
        }
    }

    fn expectimax_max(&mut self, board: &[Vec<i32>], depth: usize) -> f64 {
        if self.has_reached_target(board) {
            return WIN_SCORE + self.position_score(board);
        }
        if depth == 0 {
            return self.evaluate(board);
        }

        let key = SearchKey::new(board, depth);
        if let Some(score) = self.max_cache.get(&key) {
            return *score;
        }

        let moves = self.ordered_legal_moves(board);
        let score = if moves.is_empty() {
            LOSS_SCORE + self.position_score(board)
        } else {
            moves
                .into_iter()
                .map(|(_, outcome)| {
                    self.expectimax_chance(&outcome.board, depth.saturating_sub(1))
                        + f64::from(outcome.score_delta) * 0.001
                })
                .fold(f64::NEG_INFINITY, f64::max)
        };
        self.max_cache.insert(key, score);
        score
    }

    fn expectimax_chance(&mut self, board: &[Vec<i32>], depth: usize) -> f64 {
        if self.has_reached_target(board) {
            return WIN_SCORE + self.position_score(board);
        }
        if depth == 0 {
            return self.evaluate(board);
        }

        let key = SearchKey::new(board, depth);
        if let Some(score) = self.chance_cache.get(&key) {
            return *score;
        }

        let empties = self.chance_cells(board, depth);
        let score = if empties.is_empty() {
            self.expectimax_max(board, depth.saturating_sub(1))
        } else {
            let two_ratio = 1.0 - self.four_ratio;
            let mut total = 0.0;
            for (row, col) in &empties {
                let mut with_two = board.to_vec();
                with_two[*row][*col] = 2;
                total += two_ratio * self.expectimax_max(&with_two, depth.saturating_sub(1));

                let mut with_four = board.to_vec();
                with_four[*row][*col] = 4;
                total += self.four_ratio * self.expectimax_max(&with_four, depth.saturating_sub(1));
            }
            total / empties.len() as f64
        };
        self.chance_cache.insert(key, score);
        score
    }

    fn has_reached_target(&self, board: &[Vec<i32>]) -> bool {
        self.target_tile > 0 && max_tile(board) >= self.target_tile
    }

    fn ordered_legal_moves(&self, board: &[Vec<i32>]) -> Vec<(Direction, MoveOutcome)> {
        let mut moves = self
            .allowed_directions
            .iter()
            .copied()
            .filter_map(|direction| {
                let outcome = apply_move(board, direction);
                outcome.moved.then_some((direction, outcome))
            })
            .collect::<Vec<_>>();
        moves.sort_by(|(_, left), (_, right)| {
            self.move_order_score(right)
                .total_cmp(&self.move_order_score(left))
        });
        moves
    }

    fn move_order_score(&self, outcome: &MoveOutcome) -> f64 {
        self.evaluate(&outcome.board) + f64::from(outcome.score_delta) * 0.01
    }

    fn evaluate(&self, board: &[Vec<i32>]) -> f64 {
        if self.has_reached_target(board) {
            return WIN_SCORE + self.position_score(board);
        }
        if legal_moves(board, &self.allowed_directions).is_empty() {
            return LOSS_SCORE + self.position_score(board);
        }
        self.position_score(board)
    }

    fn position_score(&self, board: &[Vec<i32>]) -> f64 {
        let size = board.len();
        let empty_count = empty_cells(board).len();
        let max_value = max_tile(board);
        let max_rank = tile_rank(max_value.max(1));
        let merge_potential = merge_potential(board);

        if size == 3 {
            return self.small_board_score(
                board,
                empty_count,
                max_value,
                max_rank,
                merge_potential,
            );
        }
        if size >= 4 {
            return public_expectimax_heuristic(board)
                + self.snake_score(board) * size_profile(size).snake_weight
                + max_tile_placement(board, max_value) * size_profile(size).corner_weight
                + max_rank * max_rank * size_profile(size).max_rank_weight;
        }

        self.layout_score(board)
            + monotonicity(board) * 18.0
            + smoothness(board) * 5.5
            + merge_potential * merge_potential_weight(size)
            + empty_score(size, empty_count)
            + max_tile_placement(board, max_value) * max_tile_placement_weight(size)
            + max_rank * max_rank * 20.0
            + tile_growth_score(size, board, max_value)
            + legal_moves(board, &self.allowed_directions).len() as f64 * mobility_weight(size)
    }

    fn layout_score(&self, board: &[Vec<i32>]) -> f64 {
        if self.size == 3 {
            corner_cluster_score(board) * 4.2 + self.snake_score(board) * 0.5
        } else {
            self.snake_score(board) * 2.4
        }
    }

    fn snake_score(&self, board: &[Vec<i32>]) -> f64 {
        self.gradients
            .iter()
            .map(|gradient| {
                board
                    .iter()
                    .enumerate()
                    .flat_map(|(row, values)| {
                        values
                            .iter()
                            .enumerate()
                            .map(move |(col, value)| (row, col, value))
                    })
                    .map(|(row, col, value)| tile_rank(*value) * gradient[row * board.len() + col])
                    .sum::<f64>()
            })
            .fold(f64::NEG_INFINITY, f64::max)
    }

    fn small_board_score(
        &self,
        board: &[Vec<i32>],
        empty_count: usize,
        max_value: i32,
        max_rank: f64,
        merge_potential: f64,
    ) -> f64 {
        let corner = max_tile_placement(board, max_value);
        self.snake_score(board) * 0.25
            + corner_core_score(board)
            + corner_cluster_score(board) * 0.35
            + corner_order_score(board) * 0.35
            + monotonicity(board) * 45.0
            + smoothness(board) * 22.0
            + merge_potential * 520.0
            + small_board_empty_score(empty_count)
            + corner * 120.0
            + max_rank * max_rank * 180.0
            + tile_growth_score(3, board, max_value)
            + legal_moves(board, &self.allowed_directions).len() as f64 * 150.0
    }

    fn chance_cells(&self, board: &[Vec<i32>], depth: usize) -> Vec<(usize, usize)> {
        let empties = empty_cells(board);
        let limit = self.chance_limit(board.len(), depth, empties.len());
        if empties.len() <= limit {
            return empties;
        }

        let mut ranked = empties
            .into_iter()
            .map(|cell| (self.cell_risk(cell), cell))
            .collect::<Vec<_>>();
        ranked.sort_by(|(left, _), (right, _)| right.total_cmp(left));
        ranked
            .into_iter()
            .take(limit)
            .map(|(_, cell)| cell)
            .collect()
    }

    fn chance_limit(&self, size: usize, depth: usize, empty_count: usize) -> usize {
        if let Some(limit) = self.chance_cell_limit {
            return limit.min(empty_count).max(1);
        }
        let limit = match size {
            3 => empty_count,
            4 if self.strong && depth <= 2 => empty_count,
            4 if self.strong && depth <= 4 => 12,
            4 if self.strong => 10,
            4 if depth <= 1 => empty_count,
            4 if depth <= 3 => 8,
            4 => 6,
            5 if self.strong && depth <= 2 => empty_count,
            5 if self.strong && depth <= 4 => 16,
            5 if self.strong => 12,
            5 if depth <= 1 => 10,
            5 => 6,
            _ => 8,
        };
        limit.min(empty_count).max(1)
    }

    fn cell_risk(&self, cell: (usize, usize)) -> f64 {
        let index = cell.0 * self.size + cell.1;
        self.gradients
            .iter()
            .map(|gradient| gradient[index])
            .fold(0.0, f64::max)
    }
}

#[derive(Debug, Clone, Copy)]
struct SizeProfile {
    snake_weight: f64,
    corner_weight: f64,
    max_rank_weight: f64,
}

fn size_profile(size: usize) -> SizeProfile {
    match size {
        4 => SizeProfile {
            snake_weight: 1.05,
            corner_weight: 70.0,
            max_rank_weight: 145.0,
        },
        5 => SizeProfile {
            snake_weight: 0.75,
            corner_weight: 55.0,
            max_rank_weight: 130.0,
        },
        _ => SizeProfile {
            snake_weight: 0.0,
            corner_weight: 0.0,
            max_rank_weight: 0.0,
        },
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct SearchKey {
    board: BoardKey,
    depth: u8,
}

impl SearchKey {
    fn new(board: &[Vec<i32>], depth: usize) -> Self {
        Self {
            board: BoardKey::from_board(board),
            depth: depth.min(u8::MAX as usize) as u8,
        }
    }
}

#[derive(Debug)]
struct Exact3x3Solver {
    target_tile: i32,
    target_rank: u8,
    four_ratio: f64,
    four_ratio_key: u16,
    directions_mask: u8,
    allowed_directions: Vec<Direction>,
    cache: HashMap<BoardKey, f64>,
    visited: usize,
    limit: usize,
}

impl Exact3x3Solver {
    fn new(
        target_tile: i32,
        four_ratio: f64,
        allowed_directions: &[Direction],
        limit: usize,
    ) -> Self {
        let four_ratio = if four_ratio.is_finite() {
            four_ratio.clamp(0.0, 1.0)
        } else {
            0.1
        };
        Self {
            target_tile,
            target_rank: rank_u8(target_tile),
            four_ratio,
            four_ratio_key: (four_ratio * 10_000.0).round().clamp(0.0, 10_000.0) as u16,
            directions_mask: direction_mask(allowed_directions),
            allowed_directions: normalized_directions(allowed_directions),
            cache: HashMap::new(),
            visited: 0,
            limit: limit.max(1),
        }
    }

    fn choose(&mut self, board: &[Vec<i32>]) -> Option<Direction> {
        let mut best_direction = None;
        let mut best_value = f64::NEG_INFINITY;
        for direction in legal_moves(board, &self.allowed_directions) {
            let outcome = apply_move(board, direction);
            if max_tile(&outcome.board) >= self.target_tile {
                return Some(direction);
            }
            let value = self.chance_value(&outcome.board)?;
            if value > best_value {
                best_value = value;
                best_direction = Some(direction);
            }
        }
        best_direction
    }

    fn value(&mut self, board: &[Vec<i32>]) -> Option<f64> {
        if max_tile(board) >= self.target_tile {
            return Some(1.0);
        }
        if legal_moves(board, &self.allowed_directions).is_empty() {
            return Some(0.0);
        }
        if self.visited >= self.limit {
            return None;
        }

        let board_key = BoardKey::from_board(board);
        if let Some(value) = self.cache.get(&board_key) {
            return Some(*value);
        }
        let exact_key = ExactCacheKey {
            board: board_key,
            target_rank: self.target_rank,
            four_ratio: self.four_ratio_key,
            directions_mask: self.directions_mask,
        };
        if let Some(value) = exact_cache().lock().unwrap().get(&exact_key).copied() {
            self.cache.insert(board_key, value);
            return Some(value);
        }
        self.visited += 1;

        let mut best = 0.0;
        for direction in legal_moves(board, &self.allowed_directions) {
            let outcome = apply_move(board, direction);
            let value = if max_tile(&outcome.board) >= self.target_tile {
                1.0
            } else {
                self.chance_value(&outcome.board)?
            };
            if value > best {
                best = value;
            }
        }
        self.cache.insert(board_key, best);
        exact_cache().lock().unwrap().insert(exact_key, best);
        Some(best)
    }

    fn chance_value(&mut self, board: &[Vec<i32>]) -> Option<f64> {
        let empties = empty_cells(board);
        if empties.is_empty() {
            return self.value(board);
        }

        let two_ratio = 1.0 - self.four_ratio;
        let mut total = 0.0;
        for (row, col) in &empties {
            let mut with_two = board.to_vec();
            with_two[*row][*col] = 2;
            total += two_ratio * self.value(&with_two)?;

            let mut with_four = board.to_vec();
            with_four[*row][*col] = 4;
            total += self.four_ratio * self.value(&with_four)?;
        }
        Some(total / empties.len() as f64)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct ExactCacheKey {
    board: BoardKey,
    target_rank: u8,
    four_ratio: u16,
    directions_mask: u8,
}

fn exact_cache() -> &'static Mutex<HashMap<ExactCacheKey, f64>> {
    EXACT_3X3_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn direction_mask(directions: &[Direction]) -> u8 {
    let mut mask = 0;
    for direction in normalized_directions(directions) {
        mask |= match direction {
            Direction::Up => 1,
            Direction::Down => 2,
            Direction::Left => 4,
            Direction::Right => 8,
        };
    }
    mask
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct BoardKey {
    size: u8,
    cells: [u8; MAX_CELLS],
}

impl BoardKey {
    fn from_board(board: &[Vec<i32>]) -> Self {
        let mut cells = [0; MAX_CELLS];
        for (index, value) in board.iter().flatten().take(MAX_CELLS).enumerate() {
            cells[index] = rank_u8(*value);
        }
        Self {
            size: board.len().min(u8::MAX as usize) as u8,
            cells,
        }
    }
}

fn rank_u8(value: i32) -> u8 {
    tile_rank(value).round().clamp(0.0, f64::from(u8::MAX)) as u8
}

fn snake_gradients(size: usize) -> Vec<Vec<f64>> {
    let horizontal = horizontal_snake_path(size);
    let vertical = horizontal
        .iter()
        .map(|(row, col)| (*col, *row))
        .collect::<Vec<_>>();

    let mut paths = Vec::new();
    for path in [horizontal, vertical] {
        paths.push(path.clone());
        paths.push(flip_horizontal(&path, size));
        paths.push(flip_vertical(&path, size));
        paths.push(flip_vertical(&flip_horizontal(&path, size), size));
    }

    let base = match size {
        3 => 1.95_f64,
        4 => 1.65_f64,
        5 => 1.42_f64,
        _ => 1.5_f64,
    };
    let mut gradients = Vec::new();
    for path in paths {
        let mut weights = vec![0.0; size * size];
        for (position, (row, col)) in path.iter().enumerate() {
            weights[row * size + col] = base.powi((size * size - position - 1) as i32);
        }
        if !gradients.contains(&weights) {
            gradients.push(weights);
        }
    }
    gradients
}

fn horizontal_snake_path(size: usize) -> Vec<(usize, usize)> {
    let mut path = Vec::with_capacity(size * size);
    for row in 0..size {
        if row % 2 == 0 {
            path.extend((0..size).map(|col| (row, col)));
        } else {
            path.extend((0..size).rev().map(|col| (row, col)));
        }
    }
    path
}

fn flip_horizontal(path: &[(usize, usize)], size: usize) -> Vec<(usize, usize)> {
    path.iter()
        .map(|(row, col)| (*row, size - 1 - *col))
        .collect()
}

fn flip_vertical(path: &[(usize, usize)], size: usize) -> Vec<(usize, usize)> {
    path.iter()
        .map(|(row, col)| (size - 1 - *row, *col))
        .collect()
}

fn empty_score(size: usize, empty_count: usize) -> f64 {
    let empty = empty_count as f64;
    let linear = match size {
        3 => 220.0,
        4 => 95.0,
        5 => 80.0,
        _ => 75.0,
    };
    let square = if size == 3 { 35.0 } else { 8.0 };
    linear * empty + square * empty * empty
}

fn small_board_empty_score(empty_count: usize) -> f64 {
    if empty_count == 0 {
        -28_000.0
    } else {
        let empty = empty_count as f64;
        4_800.0 * empty + 1_650.0 * empty * empty
    }
}

fn max_tile_placement_weight(size: usize) -> f64 {
    if size == 3 { 90.0 } else { 350.0 }
}

fn merge_potential_weight(size: usize) -> f64 {
    if size == 3 { 90.0 } else { 15.0 }
}

fn mobility_weight(size: usize) -> f64 {
    if size == 3 { 120.0 } else { 25.0 }
}

fn tile_growth_score(size: usize, board: &[Vec<i32>], max_value: i32) -> f64 {
    if size != 3 {
        return 0.0;
    }

    let mut values = board
        .iter()
        .flatten()
        .copied()
        .filter(|value| *value > 0)
        .collect::<Vec<_>>();
    values.sort_unstable_by(|left, right| right.cmp(left));
    let second = values.get(1).copied().unwrap_or(0);
    let third = values.get(2).copied().unwrap_or(0);
    f64::from(max_value) * 65.0 + f64::from(second + third) * 18.0
}

fn max_tile_placement(board: &[Vec<i32>], max_value: i32) -> f64 {
    let size = board.len();
    let mut best = f64::NEG_INFINITY;
    for (row, values) in board.iter().enumerate() {
        for (col, value) in values.iter().enumerate() {
            if *value != max_value {
                continue;
            }
            let row_edge = row == 0 || row + 1 == size;
            let col_edge = col == 0 || col + 1 == size;
            let score = if row_edge && col_edge {
                8.0
            } else if row_edge || col_edge {
                -4.0
            } else {
                -8.0
            };
            best = best.max(score * tile_rank(max_value.max(1)));
        }
    }
    best
}

fn public_expectimax_heuristic(board: &[Vec<i32>]) -> f64 {
    let row_score = board
        .iter()
        .map(|row| line_heuristic(row.iter().copied()))
        .sum::<f64>();
    let size = board.len();
    let column_score = (0..size)
        .map(|col| line_heuristic((0..size).map(|row| board[row][col])))
        .sum::<f64>();
    row_score + column_score
}

fn line_heuristic<I>(values: I) -> f64
where
    I: IntoIterator<Item = i32>,
{
    const EMPTY_WEIGHT: f64 = 270.0;
    const MERGES_WEIGHT: f64 = 700.0;
    const MONOTONICITY_POWER: i32 = 4;
    const MONOTONICITY_WEIGHT: f64 = 47.0;
    const SUM_POWER: f64 = 3.5;
    const SUM_WEIGHT: f64 = 11.0;

    let ranks = values.into_iter().map(tile_rank).collect::<Vec<_>>();

    let empty = ranks.iter().filter(|rank| **rank == 0.0).count() as f64;
    let sum = ranks.iter().map(|rank| rank.powf(SUM_POWER)).sum::<f64>();

    let mut merges = 0.0;
    let mut previous = 0.0;
    let mut counter = 0;
    for rank in &ranks {
        if *rank == 0.0 {
            continue;
        }
        if *rank == previous {
            counter += 1;
        } else if counter > 0 {
            merges += f64::from(1 + counter);
            counter = 0;
        }
        previous = *rank;
    }
    if counter > 0 {
        merges += f64::from(1 + counter);
    }

    let mut monotonicity_left = 0.0;
    let mut monotonicity_right = 0.0;
    for pair in ranks.windows(2) {
        if pair[0] > pair[1] {
            monotonicity_left +=
                pair[0].powi(MONOTONICITY_POWER) - pair[1].powi(MONOTONICITY_POWER);
        } else {
            monotonicity_right +=
                pair[1].powi(MONOTONICITY_POWER) - pair[0].powi(MONOTONICITY_POWER);
        }
    }

    EMPTY_WEIGHT * empty + MERGES_WEIGHT * merges
        - MONOTONICITY_WEIGHT * monotonicity_left.min(monotonicity_right)
        - SUM_WEIGHT * sum
}

fn corner_cluster_score(board: &[Vec<i32>]) -> f64 {
    const BASE: [f64; 9] = [4096.0, 1024.0, 128.0, 2048.0, 512.0, 64.0, 16.0, 32.0, 8.0];
    let transposed = transpose_3x3(BASE);
    let mut best = f64::NEG_INFINITY;
    for pattern in [BASE, transposed] {
        for transformed in [
            pattern,
            flip_pattern_horizontal(pattern),
            flip_pattern_vertical(pattern),
            flip_pattern_vertical(flip_pattern_horizontal(pattern)),
        ] {
            let score = board
                .iter()
                .enumerate()
                .flat_map(|(row, values)| {
                    values
                        .iter()
                        .enumerate()
                        .map(move |(col, value)| (row, col, value))
                })
                .map(|(row, col, value)| tile_rank(*value) * transformed[row * 3 + col])
                .sum::<f64>();
            best = best.max(score);
        }
    }
    best
}

fn corner_order_score(board: &[Vec<i32>]) -> f64 {
    let base = [
        (0, 0),
        (1, 0),
        (0, 1),
        (1, 1),
        (0, 2),
        (1, 2),
        (2, 0),
        (2, 1),
        (2, 2),
    ];
    let transposed = base
        .iter()
        .map(|(row, col)| (*col, *row))
        .collect::<Vec<_>>();
    let mut paths = Vec::new();
    for path in [base.to_vec(), transposed] {
        paths.push(path.clone());
        paths.push(flip_horizontal(&path, 3));
        paths.push(flip_vertical(&path, 3));
        paths.push(flip_vertical(&flip_horizontal(&path, 3), 3));
    }

    paths
        .into_iter()
        .map(|path| {
            let ranks = path
                .iter()
                .map(|(row, col)| tile_rank(board[*row][*col]))
                .collect::<Vec<_>>();
            let weighted = ranks
                .iter()
                .enumerate()
                .map(|(index, rank)| rank * 1.7_f64.powi((8 - index) as i32) * 95.0)
                .sum::<f64>();
            let inversion_penalty = ranks
                .windows(2)
                .map(|pair| (pair[1] - pair[0]).max(0.0).powi(2) * 2_200.0)
                .sum::<f64>();
            weighted - inversion_penalty
        })
        .fold(f64::NEG_INFINITY, f64::max)
}

fn corner_core_score(board: &[Vec<i32>]) -> f64 {
    let cores = [
        [(0, 0), (0, 1), (1, 0), (1, 1)],
        [(0, 2), (0, 1), (1, 2), (1, 1)],
        [(2, 0), (1, 0), (2, 1), (1, 1)],
        [(2, 2), (1, 2), (2, 1), (1, 1)],
    ];
    let mut tiles = board
        .iter()
        .flatten()
        .copied()
        .filter(|value| *value > 0)
        .collect::<Vec<_>>();
    tiles.sort_unstable_by(|left, right| right.cmp(left));

    cores
        .iter()
        .map(|core| {
            let corner = board[core[0].0][core[0].1];
            let edge_a = board[core[1].0][core[1].1];
            let edge_b = board[core[2].0][core[2].1];
            let center = board[core[3].0][core[3].1];
            let ranks = [
                tile_rank(corner),
                tile_rank(edge_a),
                tile_rank(edge_b),
                tile_rank(center),
            ];
            let core_values = [corner, edge_a, edge_b, center];
            let top_coverage = tiles
                .iter()
                .take(4)
                .filter(|value| core_values.contains(value))
                .count() as f64;
            let outside = board
                .iter()
                .enumerate()
                .flat_map(|(row, values)| {
                    values
                        .iter()
                        .enumerate()
                        .map(move |(col, value)| ((row, col), *value))
                })
                .filter(|(cell, _)| !core.contains(cell))
                .map(|(_, value)| value)
                .collect::<Vec<_>>();

            let center_rank = ranks[3];
            let builder = outside
                .iter()
                .map(|value| {
                    let rank = tile_rank(*value);
                    if rank == center_rank {
                        3_200.0
                    } else if center_rank > 0.0 && rank + 1.0 == center_rank {
                        1_100.0
                    } else {
                        0.0
                    }
                })
                .sum::<f64>();
            let edge_low = ranks[1].min(ranks[2]);
            let edge_high = ranks[1].max(ranks[2]);
            let order_penalty = (edge_high - ranks[0]).max(0.0).powi(2) * 18_000.0
                + (center_rank - edge_low).max(0.0).powi(2) * 18_000.0
                + (edge_high - edge_low - 2.0).max(0.0).powi(2) * 1_800.0;
            top_coverage * 16_000.0
                + ranks[0] * 13_000.0
                + edge_high * 6_000.0
                + edge_low * 5_200.0
                + center_rank * 4_600.0
                + builder
                - order_penalty
        })
        .fold(f64::NEG_INFINITY, f64::max)
}

fn transpose_3x3(pattern: [f64; 9]) -> [f64; 9] {
    [
        pattern[0], pattern[3], pattern[6], pattern[1], pattern[4], pattern[7], pattern[2],
        pattern[5], pattern[8],
    ]
}

fn flip_pattern_horizontal(pattern: [f64; 9]) -> [f64; 9] {
    [
        pattern[2], pattern[1], pattern[0], pattern[5], pattern[4], pattern[3], pattern[8],
        pattern[7], pattern[6],
    ]
}

fn flip_pattern_vertical(pattern: [f64; 9]) -> [f64; 9] {
    [
        pattern[6], pattern[7], pattern[8], pattern[3], pattern[4], pattern[5], pattern[0],
        pattern[1], pattern[2],
    ]
}

fn monotonicity(board: &[Vec<i32>]) -> f64 {
    let size = board.len();
    let mut left = 0.0;
    let mut right = 0.0;
    let mut up = 0.0;
    let mut down = 0.0;

    for row in board {
        for col in 0..size.saturating_sub(1) {
            let current = tile_rank(row[col]);
            let next = tile_rank(row[col + 1]);
            if current > next {
                left += next - current;
            } else {
                right += current - next;
            }
        }
    }

    for rows in board.windows(2) {
        for (&current_value, &next_value) in rows[0].iter().zip(&rows[1]) {
            let current = tile_rank(current_value);
            let next = tile_rank(next_value);
            if current > next {
                up += next - current;
            } else {
                down += current - next;
            }
        }
    }

    left.max(right) + up.max(down)
}

fn smoothness(board: &[Vec<i32>]) -> f64 {
    let size = board.len();
    let mut score = 0.0;
    for row in 0..size {
        for col in 0..size {
            let value = board[row][col];
            if value == 0 {
                continue;
            }
            let rank = tile_rank(value);
            if let Some(next) = next_non_empty_right(board, row, col) {
                score -= (rank - tile_rank(next)).abs();
            }
            if let Some(next) = next_non_empty_down(board, row, col) {
                score -= (rank - tile_rank(next)).abs();
            }
        }
    }
    score
}

fn next_non_empty_right(board: &[Vec<i32>], row: usize, col: usize) -> Option<i32> {
    board[row][col + 1..]
        .iter()
        .copied()
        .find(|value| *value > 0)
}

fn next_non_empty_down(board: &[Vec<i32>], row: usize, col: usize) -> Option<i32> {
    board
        .iter()
        .skip(row + 1)
        .map(|values| values[col])
        .find(|value| *value > 0)
}

fn merge_potential(board: &[Vec<i32>]) -> f64 {
    let size = board.len();
    let mut score = 0.0;
    for row in board {
        score += line_merge_potential(row.iter().copied());
    }
    for col in 0..size {
        score += line_merge_potential(board.iter().take(size).map(|row| row[col]));
    }
    score
}

fn line_merge_potential<I>(values: I) -> f64
where
    I: IntoIterator<Item = i32>,
{
    let compacted = values
        .into_iter()
        .filter(|value| *value > 0)
        .collect::<Vec<_>>();
    compacted
        .windows(2)
        .filter(|values| values[0] == values[1])
        .map(|values| tile_rank(values[0]).powi(2))
        .sum()
}

fn env_usize(key: &str) -> Option<usize> {
    std::env::var(key)
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
}
