#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CellFill {
    pub row: i32,
    pub col: i32,
    pub value: i32,
}

pub fn solve(givens: &[i32], size: i32, box_size: i32) -> Result<Vec<i32>, String> {
    let puzzle = Puzzle::new(givens, size, box_size)?;
    puzzle.solve()
}

pub fn solve_fills(
    givens: &[i32],
    user_board: &[i32],
    size: i32,
    box_size: i32,
) -> Result<Vec<CellFill>, String> {
    let puzzle = Puzzle::new(givens, size, box_size)?;
    puzzle.validate_user_board(user_board)?;
    let solution = puzzle.solve()?;
    let size = puzzle.size;
    let mut fills = Vec::new();
    for index in 0..solution.len() {
        if givens[index] != 0 {
            continue;
        }
        if user_board[index] == solution[index] {
            continue;
        }
        fills.push(CellFill {
            row: (index / size) as i32,
            col: (index % size) as i32,
            value: solution[index],
        });
    }
    Ok(fills)
}

#[derive(Debug, Clone)]
struct Puzzle<'a> {
    givens: &'a [i32],
    size: usize,
    box_size: usize,
    all_mask: u32,
}

impl<'a> Puzzle<'a> {
    fn new(givens: &'a [i32], size: i32, box_size: i32) -> Result<Self, String> {
        let size = usize::try_from(size).map_err(|_| "数独尺寸无效".to_string())?;
        let box_size = usize::try_from(box_size).map_err(|_| "数独宫格尺寸无效".to_string())?;
        if size == 0 || size > 31 {
            return Err("数独尺寸无效".to_string());
        }
        if box_size == 0 || box_size * box_size != size {
            return Err("数独宫格尺寸和棋盘尺寸不匹配".to_string());
        }
        let total = size
            .checked_mul(size)
            .ok_or_else(|| "数独棋盘尺寸过大".to_string())?;
        if givens.len() != total {
            return Err("数独格子数量和尺寸不匹配".to_string());
        }
        let all_mask = (1u32 << size) - 1;
        let puzzle = Self {
            givens,
            size,
            box_size,
            all_mask,
        };
        puzzle.validate_values(givens)?;
        Ok(puzzle)
    }

    fn validate_user_board(&self, user_board: &[i32]) -> Result<(), String> {
        if user_board.len() != self.givens.len() {
            return Err("当前数独局面和题目尺寸不匹配".to_string());
        }
        self.validate_values(user_board)?;
        for (index, given) in self.givens.iter().enumerate() {
            if *given != 0 && user_board[index] != *given {
                return Err("当前数独局面和固定数字不一致".to_string());
            }
        }
        Ok(())
    }

    fn validate_values(&self, board: &[i32]) -> Result<(), String> {
        let size = self.size as i32;
        for value in board {
            if *value < 0 || *value > size {
                return Err("数独棋盘包含无效数字".to_string());
            }
        }
        Ok(())
    }

    fn solve(&self) -> Result<Vec<i32>, String> {
        let mut board = self.givens.to_vec();
        let mut rows = vec![0u32; self.size];
        let mut cols = vec![0u32; self.size];
        let mut boxes = vec![0u32; self.size];

        for (index, value) in board.iter().copied().enumerate() {
            if value == 0 {
                continue;
            }
            let row = index / self.size;
            let col = index % self.size;
            let box_index = self.box_index(row, col);
            let bit = self.value_bit(value);
            if rows[row] & bit != 0 || cols[col] & bit != 0 || boxes[box_index] & bit != 0 {
                return Err("数独题目存在冲突数字".to_string());
            }
            rows[row] |= bit;
            cols[col] |= bit;
            boxes[box_index] |= bit;
        }

        if solve_recursive(self, &mut board, &mut rows, &mut cols, &mut boxes) {
            Ok(board)
        } else {
            Err("数独题目无解".to_string())
        }
    }

    fn box_index(&self, row: usize, col: usize) -> usize {
        (row / self.box_size) * self.box_size + (col / self.box_size)
    }

    fn value_bit(&self, value: i32) -> u32 {
        1u32 << (value as u32 - 1)
    }
}

fn solve_recursive(
    puzzle: &Puzzle<'_>,
    board: &mut [i32],
    rows: &mut [u32],
    cols: &mut [u32],
    boxes: &mut [u32],
) -> bool {
    let Some((index, candidates)) = choose_next_cell(puzzle, board, rows, cols, boxes) else {
        return true;
    };
    if candidates == 0 {
        return false;
    }

    let row = index / puzzle.size;
    let col = index % puzzle.size;
    let box_index = puzzle.box_index(row, col);
    let mut candidates = candidates;
    while candidates != 0 {
        let bit = candidates & candidates.wrapping_neg();
        candidates &= !bit;
        let value = bit.trailing_zeros() as i32 + 1;
        board[index] = value;
        rows[row] |= bit;
        cols[col] |= bit;
        boxes[box_index] |= bit;

        if solve_recursive(puzzle, board, rows, cols, boxes) {
            return true;
        }

        rows[row] &= !bit;
        cols[col] &= !bit;
        boxes[box_index] &= !bit;
        board[index] = 0;
    }
    false
}

fn choose_next_cell(
    puzzle: &Puzzle<'_>,
    board: &[i32],
    rows: &[u32],
    cols: &[u32],
    boxes: &[u32],
) -> Option<(usize, u32)> {
    let mut best = None;
    let mut best_count = u32::MAX;
    for (index, value) in board.iter().copied().enumerate() {
        if value != 0 {
            continue;
        }
        let row = index / puzzle.size;
        let col = index % puzzle.size;
        let box_index = puzzle.box_index(row, col);
        let candidates = puzzle.all_mask & !(rows[row] | cols[col] | boxes[box_index]);
        let count = candidates.count_ones();
        if count < best_count {
            best = Some((index, candidates));
            best_count = count;
            if count <= 1 {
                break;
            }
        }
    }
    best
}
