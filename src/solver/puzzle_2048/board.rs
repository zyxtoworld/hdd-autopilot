#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
}

impl Direction {
    pub fn as_api_str(self) -> &'static str {
        match self {
            Self::Up => "up",
            Self::Down => "down",
            Self::Left => "left",
            Self::Right => "right",
        }
    }
}

pub const DEFAULT_DIRECTIONS: &[Direction] = &[
    Direction::Up,
    Direction::Down,
    Direction::Left,
    Direction::Right,
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MoveOutcome {
    pub board: Vec<Vec<i32>>,
    pub score_delta: i32,
    pub moved: bool,
    pub max_tile: i32,
}

pub fn apply_move(board: &[Vec<i32>], direction: Direction) -> MoveOutcome {
    let size = board.len();
    if size == 0 || board.iter().any(|row| row.len() != size) {
        return MoveOutcome {
            board: board.to_vec(),
            score_delta: 0,
            moved: false,
            max_tile: max_tile(board),
        };
    }

    let mut next = vec![vec![0; size]; size];
    let mut score_delta = 0;
    match direction {
        Direction::Left => {
            for row in 0..size {
                let (line, score) = merge_line(board[row].iter().copied());
                next[row][..size].copy_from_slice(&line[..size]);
                score_delta += score;
            }
        }
        Direction::Right => {
            for row in 0..size {
                let (mut line, score) = merge_line(board[row].iter().rev().copied());
                line.reverse();
                next[row][..size].copy_from_slice(&line[..size]);
                score_delta += score;
            }
        }
        Direction::Up => {
            for col in 0..size {
                let (line, score) = merge_line((0..size).map(|row| board[row][col]));
                for row in 0..size {
                    next[row][col] = line[row];
                }
                score_delta += score;
            }
        }
        Direction::Down => {
            for col in 0..size {
                let (mut line, score) = merge_line((0..size).rev().map(|row| board[row][col]));
                line.reverse();
                for row in 0..size {
                    next[row][col] = line[row];
                }
                score_delta += score;
            }
        }
    }

    MoveOutcome {
        moved: next != board,
        max_tile: max_tile(&next),
        board: next,
        score_delta,
    }
}

pub fn legal_moves(board: &[Vec<i32>], allowed_directions: &[Direction]) -> Vec<Direction> {
    allowed_directions
        .iter()
        .copied()
        .filter(|direction| apply_move(board, *direction).moved)
        .collect()
}

pub(super) fn empty_cells(board: &[Vec<i32>]) -> Vec<(usize, usize)> {
    let mut cells = Vec::new();
    for (row, values) in board.iter().enumerate() {
        for (col, value) in values.iter().enumerate() {
            if *value == 0 {
                cells.push((row, col));
            }
        }
    }
    cells
}

pub(super) fn max_tile(board: &[Vec<i32>]) -> i32 {
    board.iter().flatten().copied().max().unwrap_or(0)
}

pub(super) fn normalize_board(board: &[Vec<i32>]) -> Option<Vec<Vec<i32>>> {
    let size = board.len();
    if !(3..=5).contains(&size) || board.iter().any(|row| row.len() != size) {
        return None;
    }
    Some(board.to_vec())
}

pub(super) fn tile_rank(value: i32) -> f64 {
    if value <= 0 {
        0.0
    } else {
        (value as f64).log2()
    }
}

fn merge_line<I>(values: I) -> (Vec<i32>, i32)
where
    I: IntoIterator<Item = i32>,
{
    let source = values.into_iter().collect::<Vec<_>>();
    let compacted = source
        .iter()
        .copied()
        .filter(|value| *value > 0)
        .collect::<Vec<_>>();
    let mut merged = Vec::with_capacity(source.len());
    let mut score_delta = 0;
    let mut index = 0;
    while index < compacted.len() {
        if index + 1 < compacted.len() && compacted[index] == compacted[index + 1] {
            let value = compacted[index] * 2;
            merged.push(value);
            score_delta += value;
            index += 2;
        } else {
            merged.push(compacted[index]);
            index += 1;
        }
    }
    while merged.len() < source.len() {
        merged.push(0);
    }
    (merged, score_delta)
}
