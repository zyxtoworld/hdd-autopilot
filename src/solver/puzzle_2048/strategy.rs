use super::board::{
    DEFAULT_DIRECTIONS, Direction, apply_move, empty_cells, normalize_board, tile_rank,
};

pub(super) fn choose_next_move(
    board: &[Vec<i32>],
    allowed_directions: &[Direction],
) -> Option<Direction> {
    let board = normalize_board(board)?;
    let directions = ordered_directions(allowed_directions);
    if directions.is_empty() {
        return None;
    }

    let depth = search_depth(board.len());
    let mut best_dir = None;
    let mut best_score = f64::NEG_INFINITY;
    for direction in directions {
        let outcome = apply_move(&board, direction);
        if !outcome.moved {
            continue;
        }
        let score = expectimax_chance(&outcome.board, depth.saturating_sub(1));
        if score > best_score {
            best_score = score;
            best_dir = Some(direction);
        }
    }
    best_dir
}

fn search_depth(size: usize) -> usize {
    let env_key = match size {
        3 => Some("DEPTH_3"),
        4 => Some("DEPTH_4"),
        5 => Some("DEPTH_5"),
        _ => None,
    };
    if let Some(depth) = env_key.and_then(env_usize) {
        return depth;
    }
    match size {
        3 => 8,
        4 => 5,
        5 => 3,
        _ => 4,
    }
}

fn ordered_directions(_allowed_directions: &[Direction]) -> Vec<Direction> {
    DEFAULT_DIRECTIONS.to_vec()
}

fn expectimax_max(board: &[Vec<i32>], depth: usize) -> f64 {
    if depth == 0 {
        return evaluate(board);
    }

    let mut best = f64::NEG_INFINITY;
    let mut any_valid = false;
    for direction in ordered_directions(DEFAULT_DIRECTIONS) {
        let outcome = apply_move(board, direction);
        if !outcome.moved {
            continue;
        }
        any_valid = true;
        let score = expectimax_chance(&outcome.board, depth.saturating_sub(1));
        if score > best {
            best = score;
        }
    }
    if any_valid { best } else { -1_000_000_000.0 }
}

fn expectimax_chance(board: &[Vec<i32>], depth: usize) -> f64 {
    if depth == 0 {
        return evaluate(board);
    }

    let empties = empty_cells(board);
    if empties.is_empty() {
        return evaluate(board);
    }

    let sample = sample_empty_cells(&empties);
    let mut total = 0.0;
    for (row, col) in &sample {
        let mut with_two = board.to_vec();
        with_two[*row][*col] = 2;
        total += 0.9 * expectimax_max(&with_two, depth.saturating_sub(1));

        let mut with_four = board.to_vec();
        with_four[*row][*col] = 4;
        total += 0.1 * expectimax_max(&with_four, depth.saturating_sub(1));
    }
    total / sample.len() as f64
}

fn sample_empty_cells(empties: &[(usize, usize)]) -> Vec<(usize, usize)> {
    let chance_sample_limit = env_usize("CHANCE_SAMPLE_LIMIT").unwrap_or(6).max(1);
    if empties.len() <= chance_sample_limit {
        return empties.to_vec();
    }
    let stride = empties.len().div_ceil(chance_sample_limit);
    empties
        .iter()
        .enumerate()
        .filter_map(|(index, cell)| (index % stride == 0).then_some(*cell))
        .collect()
}

fn evaluate(board: &[Vec<i32>]) -> f64 {
    let size = board.len();
    let mut empty = 0;
    let mut max_tile = 0;
    let mut max_row = 0;
    let mut max_col = 0;
    for (row, values) in board.iter().enumerate() {
        for (col, &value) in values.iter().enumerate() {
            if value == 0 {
                empty += 1;
                continue;
            }
            if value > max_tile {
                max_tile = value;
                max_row = row;
                max_col = col;
            }
        }
    }

    let in_corner = (max_row == 0 || max_row + 1 == size) && (max_col == 0 || max_col + 1 == size);
    let corner_bonus = if in_corner {
        tile_rank(max_tile) * 2.0
    } else {
        0.0
    };
    let empty_score = if empty == 0 {
        -10.0
    } else {
        (empty as f64).ln()
    };

    monotonicity(board)
        + smoothness(board) * 0.1
        + empty_score * 2.7
        + corner_bonus
        + tile_rank(max_tile.max(1))
}

fn monotonicity(board: &[Vec<i32>]) -> f64 {
    let size = board.len();
    let mut up = 0.0;
    let mut down = 0.0;
    let mut left = 0.0;
    let mut right = 0.0;
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
    for col in 0..size {
        for row in 0..size.saturating_sub(1) {
            let current = tile_rank(board[row][col]);
            let next = tile_rank(board[row + 1][col]);
            if current > next {
                up += next - current;
            } else {
                down += current - next;
            }
        }
    }
    up.max(down) + left.max(right)
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
            if col + 1 < size && board[row][col + 1] != 0 {
                score -= (rank - tile_rank(board[row][col + 1])).abs();
            }
            if row + 1 < size && board[row + 1][col] != 0 {
                score -= (rank - tile_rank(board[row + 1][col])).abs();
            }
        }
    }
    score
}

fn env_usize(key: &str) -> Option<usize> {
    std::env::var(key)
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
}
