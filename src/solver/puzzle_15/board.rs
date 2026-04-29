#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TileMove {
    Up,
    Down,
    Left,
    Right,
}

impl TileMove {
    pub fn as_api_str(self) -> &'static str {
        match self {
            Self::Up => "up",
            Self::Down => "down",
            Self::Left => "left",
            Self::Right => "right",
        }
    }

    pub(super) fn reverse(self) -> Self {
        match self {
            Self::Up => Self::Down,
            Self::Down => Self::Up,
            Self::Left => Self::Right,
            Self::Right => Self::Left,
        }
    }
}

pub fn apply_tile_move(board: &[i32], size: i32, direction: TileMove) -> Option<Vec<i32>> {
    let size = usize::try_from(size).ok()?;
    let mut normalized = normalize_board(board, size).ok()?;
    apply_move_u8(&mut normalized, size, direction)?;
    Some(normalized.into_iter().map(i32::from).collect())
}

pub(super) fn normalize_board(board: &[i32], size: usize) -> Result<Vec<u8>, String> {
    let total = size
        .checked_mul(size)
        .ok_or_else(|| "棋盘尺寸过大".to_string())?;
    if total == 0 || total > u8::MAX as usize {
        return Err("棋盘尺寸无效".to_string());
    }
    if board.len() != total {
        return Err("棋盘格子数量和尺寸不匹配".to_string());
    }
    let mut seen = vec![false; total];
    let mut normalized = Vec::with_capacity(total);
    for value in board {
        let value = usize::try_from(*value).map_err(|_| "棋盘包含无效数字".to_string())?;
        if value >= total || seen[value] {
            return Err("棋盘包含重复或越界数字".to_string());
        }
        seen[value] = true;
        normalized.push(value as u8);
    }
    Ok(normalized)
}

pub(super) fn validate_solvable(board: &[u8], size: usize) -> Result<(), String> {
    let inversions = inversion_count(board);
    let blank = blank_index(board)?;
    let solvable = if size % 2 == 1 {
        inversions % 2 == 0
    } else {
        let blank_row_from_bottom = size - blank / size;
        (inversions + blank_row_from_bottom) % 2 == 1
    };
    if solvable {
        Ok(())
    } else {
        Err("当前棋盘不可解".to_string())
    }
}

pub(super) fn heuristic(board: &[u8], distance: &[Vec<i32>]) -> i32 {
    board
        .iter()
        .enumerate()
        .filter(|(_, tile)| **tile != 0)
        .map(|(index, tile)| distance[*tile as usize][index])
        .sum()
}

pub(super) fn manhattan_distance_table(size: usize) -> Vec<Vec<i32>> {
    let total = size * size;
    let mut table = vec![vec![0; total]; total];
    for tile in 1..total {
        let goal_index = tile - 1;
        let goal_row = goal_index / size;
        let goal_col = goal_index % size;
        for index in 0..total {
            let row = index / size;
            let col = index % size;
            table[tile][index] = goal_row.abs_diff(row) as i32 + goal_col.abs_diff(col) as i32;
        }
    }
    table
}

pub(super) fn legal_tile_moves(size: usize, blank: usize) -> Vec<(TileMove, usize)> {
    let row = blank / size;
    let col = blank % size;
    let mut moves = Vec::with_capacity(4);
    if row + 1 < size {
        moves.push((TileMove::Up, blank + size));
    }
    if row > 0 {
        moves.push((TileMove::Down, blank - size));
    }
    if col + 1 < size {
        moves.push((TileMove::Left, blank + 1));
    }
    if col > 0 {
        moves.push((TileMove::Right, blank - 1));
    }
    moves
}

pub(super) fn blank_index(board: &[u8]) -> Result<usize, String> {
    board
        .iter()
        .position(|value| *value == 0)
        .ok_or_else(|| "棋盘缺少空格".to_string())
}

pub(super) fn goal_board(size: usize) -> Vec<u8> {
    let total = size * size;
    let mut board = (1..total).map(|value| value as u8).collect::<Vec<_>>();
    board.push(0);
    board
}

pub(super) fn is_goal(board: &[u8]) -> bool {
    if board.is_empty() {
        return false;
    }
    for (index, value) in board.iter().enumerate().take(board.len() - 1) {
        if *value as usize != index + 1 {
            return false;
        }
    }
    board[board.len() - 1] == 0
}

fn inversion_count(board: &[u8]) -> usize {
    let mut inversions = 0usize;
    for left in 0..board.len() {
        if board[left] == 0 {
            continue;
        }
        for right in left + 1..board.len() {
            if board[right] != 0 && board[left] > board[right] {
                inversions += 1;
            }
        }
    }
    inversions
}

fn apply_move_u8(board: &mut [u8], size: usize, direction: TileMove) -> Option<()> {
    let blank = blank_index(board).ok()?;
    let tile_index = legal_tile_moves(size, blank)
        .into_iter()
        .find_map(|(candidate, index)| (candidate == direction).then_some(index))?;
    board[blank] = board[tile_index];
    board[tile_index] = 0;
    Some(())
}
