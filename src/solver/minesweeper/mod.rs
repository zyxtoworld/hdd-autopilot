use std::collections::{BTreeSet, HashMap, HashSet};

const ENUMERATION_ASSIGNMENT_LIMIT: usize = 1_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Cell {
    pub row: i32,
    pub col: i32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Board {
    rows: i32,
    cols: i32,
    mine_count: i32,
    revealed: Vec<bool>,
    flagged: Vec<bool>,
    numbers: Vec<Option<i32>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Move {
    pub action: &'static str,
    pub x: i32,
    pub y: i32,
    pub risk: f64,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Constraint {
    vars: BTreeSet<Cell>,
    mines: i32,
}

#[derive(Debug, Clone, Default)]
struct ComponentStats {
    vars: Vec<Cell>,
    solutions: usize,
    mine_hits: Vec<usize>,
    mines_total: usize,
    solutions_by_mines: Vec<usize>,
    mine_hits_by_mines: Vec<Vec<usize>>,
    cutoff: bool,
}

impl Board {
    pub fn new(
        rows: i32,
        cols: i32,
        mine_count: i32,
        revealed_matrix: &[Vec<bool>],
        flagged_matrix: &[Vec<bool>],
    ) -> Result<Self, String> {
        if rows <= 0 || cols <= 0 {
            return Err("扫雷棋盘尺寸无效".to_string());
        }
        let cell_count = checked_cell_count(rows, cols)?;
        let revealed = flatten_bool_matrix(rows, cols, revealed_matrix, "revealed")?;
        let flagged = flatten_bool_matrix(rows, cols, flagged_matrix, "flagged")?;
        Ok(Self {
            rows,
            cols,
            mine_count: mine_count.max(0),
            revealed,
            flagged,
            numbers: vec![None; cell_count],
        })
    }

    pub fn rows(&self) -> i32 {
        self.rows
    }

    pub fn cols(&self) -> i32 {
        self.cols
    }

    pub fn mine_count(&self) -> i32 {
        self.mine_count
    }

    pub fn reveal_number(&mut self, cell: Cell, value: i32) -> Result<(), String> {
        if !self.in_bounds(cell) {
            return Err(format!("扫雷坐标越界：({}, {})", cell.row, cell.col));
        }
        let index = self.index(cell);
        self.revealed[index] = true;
        self.flagged[index] = false;
        self.numbers[index] = Some(value.clamp(0, 8));
        Ok(())
    }

    pub fn sync_masks(
        &mut self,
        revealed_matrix: &[Vec<bool>],
        flagged_matrix: &[Vec<bool>],
    ) -> Result<(), String> {
        self.revealed = flatten_bool_matrix(self.rows, self.cols, revealed_matrix, "revealed")?;
        self.flagged = flatten_bool_matrix(self.rows, self.cols, flagged_matrix, "flagged")?;
        for index in 0..self.numbers.len() {
            if !self.revealed[index] {
                self.numbers[index] = None;
            }
            if self.revealed[index] {
                self.flagged[index] = false;
            }
        }
        Ok(())
    }

    pub fn apply_revealed_cells(&mut self, cells: &[[i32; 3]]) -> Result<(), String> {
        for [row, col, value] in cells {
            self.reveal_number(
                Cell {
                    row: *row,
                    col: *col,
                },
                *value,
            )?;
        }
        Ok(())
    }

    pub fn apply_flag(&mut self, cell: Cell, flagged: bool) -> Result<(), String> {
        if !self.in_bounds(cell) {
            return Err(format!("扫雷坐标越界：({}, {})", cell.row, cell.col));
        }
        let index = self.index(cell);
        if !self.revealed[index] {
            self.flagged[index] = flagged;
        }
        Ok(())
    }

    pub fn is_revealed(&self, cell: Cell) -> bool {
        self.revealed[self.index(cell)]
    }

    pub fn is_flagged(&self, cell: Cell) -> bool {
        self.flagged[self.index(cell)]
    }

    pub fn number(&self, cell: Cell) -> Option<i32> {
        self.numbers[self.index(cell)]
    }

    pub fn unknown_cells(&self) -> Vec<Cell> {
        self.cells()
            .into_iter()
            .filter(|cell| !self.is_revealed(*cell) && !self.is_flagged(*cell))
            .collect()
    }

    pub fn flagged_count(&self) -> i32 {
        self.flagged.iter().filter(|flagged| **flagged).count() as i32
    }

    pub fn revealed_count(&self) -> i32 {
        self.revealed.iter().filter(|revealed| **revealed).count() as i32
    }

    pub fn known_number_count(&self) -> i32 {
        self.numbers
            .iter()
            .filter(|number| number.is_some())
            .count() as i32
    }

    pub fn complete_by_masks(&self) -> bool {
        let safe_cells = (self.rows * self.cols - self.mine_count).max(0);
        self.revealed_count() >= safe_cells
    }

    fn cells(&self) -> Vec<Cell> {
        let mut cells = Vec::with_capacity(self.revealed.len());
        for row in 0..self.rows {
            for col in 0..self.cols {
                cells.push(Cell { row, col });
            }
        }
        cells
    }

    fn neighbors(&self, cell: Cell) -> Vec<Cell> {
        let mut neighbors = Vec::with_capacity(8);
        for row in (cell.row - 1)..=(cell.row + 1) {
            for col in (cell.col - 1)..=(cell.col + 1) {
                let neighbor = Cell { row, col };
                if neighbor == cell || !self.in_bounds(neighbor) {
                    continue;
                }
                neighbors.push(neighbor);
            }
        }
        neighbors
    }

    fn in_bounds(&self, cell: Cell) -> bool {
        cell.row >= 0 && cell.row < self.rows && cell.col >= 0 && cell.col < self.cols
    }

    fn index(&self, cell: Cell) -> usize {
        (cell.row * self.cols + cell.col) as usize
    }
}

pub fn next_move(board: &Board) -> Option<Move> {
    if board.complete_by_masks() {
        return None;
    }
    if board.revealed_count() == 0 {
        let opening = opening_cell(board);
        return Some(Move {
            action: "reveal",
            x: opening.row,
            y: opening.col,
            risk: 0.0,
            reason: "首翻格子，服务端保证第一下不踩雷".to_string(),
        });
    }

    if let Some(next) = repair_flag_move(board) {
        return Some(next);
    }
    if let Some(next) = deterministic_move(board) {
        return Some(next);
    }
    if let Some(next) = subset_move(board) {
        return Some(next);
    }
    if let Some(next) = overlap_move(board) {
        return Some(next);
    }
    probability_move(board)
}

fn opening_cell(_board: &Board) -> Cell {
    Cell { row: 0, col: 0 }
}

fn repair_flag_move(board: &Board) -> Option<Move> {
    if board.flagged_count() > board.mine_count() {
        return flagged_cells(board).first().copied().map(unflag_move);
    }
    for numbered in numbered_cells(board) {
        let value = board.number(numbered).unwrap_or(0);
        let flagged = board
            .neighbors(numbered)
            .into_iter()
            .filter(|cell| board.is_flagged(*cell))
            .collect::<Vec<_>>();
        if flagged.len() as i32 > value {
            return flagged.first().copied().map(unflag_move);
        }
    }
    None
}

fn unflag_move(cell: Cell) -> Move {
    Move {
        action: "unflag",
        x: cell.row,
        y: cell.col,
        risk: 0.0,
        reason: "flag conflicts with revealed numbers; unflag and recompute".to_string(),
    }
}

fn deterministic_move(board: &Board) -> Option<Move> {
    let mut safe = BTreeSet::new();
    let mut mines = BTreeSet::new();
    let mut chord = None;
    for numbered in numbered_cells(board) {
        let value = board.number(numbered).unwrap_or(0);
        let neighbors = board.neighbors(numbered);
        let flagged = neighbors
            .iter()
            .filter(|cell| board.is_flagged(**cell))
            .count() as i32;
        let hidden = neighbors
            .into_iter()
            .filter(|cell| !board.is_revealed(*cell) && !board.is_flagged(*cell))
            .collect::<Vec<_>>();
        let remaining = value - flagged;
        if remaining == 0 && !hidden.is_empty() {
            chord.get_or_insert(numbered);
            safe.extend(hidden);
        } else if remaining > 0 && remaining == hidden.len() as i32 {
            mines.extend(hidden);
        }
    }
    if let Some(cell) = mines.first().copied() {
        return Some(Move {
            action: "flag",
            x: cell.row,
            y: cell.col,
            risk: 1.0,
            reason: "确定为雷，先标旗降低后续误点风险".to_string(),
        });
    }
    if let Some(cell) = chord {
        return Some(Move {
            action: "chord",
            x: cell.row,
            y: cell.col,
            risk: 0.0,
            reason: "周围雷数已由旗子满足，双击展开剩余安全格".to_string(),
        });
    }
    safe.first().copied().map(|cell| Move {
        action: "reveal",
        x: cell.row,
        y: cell.col,
        risk: 0.0,
        reason: "确定安全，直接翻开".to_string(),
    })
}

fn subset_move(board: &Board) -> Option<Move> {
    let constraints = constraints(board);
    let mut safe = BTreeSet::new();
    let mut mines = BTreeSet::new();
    for left in &constraints {
        for right in &constraints {
            if left == right || !left.vars.is_subset(&right.vars) {
                continue;
            }
            let diff = right
                .vars
                .difference(&left.vars)
                .copied()
                .collect::<BTreeSet<_>>();
            if diff.is_empty() {
                continue;
            }
            let mine_diff = right.mines - left.mines;
            if mine_diff == 0 {
                safe.extend(diff);
            } else if mine_diff == safe_len_as_i32(&diff) {
                mines.extend(diff);
            }
        }
    }
    if let Some(cell) = mines.first().copied() {
        return Some(Move {
            action: "flag",
            x: cell.row,
            y: cell.col,
            risk: 1.0,
            reason: "集合约束推出这里必定是雷".to_string(),
        });
    }
    safe.first().copied().map(|cell| Move {
        action: "reveal",
        x: cell.row,
        y: cell.col,
        risk: 0.0,
        reason: "集合约束推出这里安全".to_string(),
    })
}

fn overlap_move(board: &Board) -> Option<Move> {
    let constraints = constraints(board);
    overlap_constraint_move(&constraints)
}

fn overlap_constraint_move(constraints: &[Constraint]) -> Option<Move> {
    let mut safe = BTreeSet::new();
    let mut mines = BTreeSet::new();
    for left in constraints {
        for right in constraints {
            if left == right {
                continue;
            }
            let left_only = left
                .vars
                .difference(&right.vars)
                .copied()
                .collect::<BTreeSet<_>>();
            let right_only = right
                .vars
                .difference(&left.vars)
                .copied()
                .collect::<BTreeSet<_>>();
            if left_only.is_empty() && right_only.is_empty() {
                continue;
            }
            let mine_diff = left.mines - right.mines;
            if mine_diff == safe_len_as_i32(&left_only) {
                mines.extend(left_only);
                safe.extend(right_only);
            } else if mine_diff == -safe_len_as_i32(&right_only) {
                safe.extend(left_only);
                mines.extend(right_only);
            }
        }
    }
    if let Some(cell) = mines.first().copied() {
        return Some(Move {
            action: "flag",
            x: cell.row,
            y: cell.col,
            risk: 1.0,
            reason: "閲嶅彔绾︽潫鎺ㄥ嚭杩欓噷蹇呭畾鏄浄".to_string(),
        });
    }
    safe.first().copied().map(|cell| Move {
        action: "reveal",
        x: cell.row,
        y: cell.col,
        risk: 0.0,
        reason: "閲嶅彔绾︽潫鎺ㄥ嚭杩欓噷瀹夊叏".to_string(),
    })
}

fn probability_move(board: &Board) -> Option<Move> {
    let constraints = constraints(board);
    if constraints.is_empty() {
        return best_unconstrained_guess(board, 0.0);
    }

    let remaining_mines = remaining_mines(board);
    let components = enumerate_components(&constraints, Some(remaining_mines));
    if let Some(next) = exact_probability_move(board, &components) {
        return Some(next);
    }

    let mut best_frontier: Option<(Cell, f64)> = None;
    let mut expected_frontier_mines = 0.0;
    let mut exact_frontier = HashSet::new();
    let mut any_cutoff = false;

    for stats in components {
        if stats.solutions == 0 {
            any_cutoff = true;
            continue;
        }
        any_cutoff |= stats.cutoff;
        expected_frontier_mines += stats.mines_total as f64 / stats.solutions as f64;
        for (index, cell) in stats.vars.iter().copied().enumerate() {
            exact_frontier.insert(cell);
            let risk = stats.mine_hits[index] as f64 / stats.solutions as f64;
            if !stats.cutoff && risk == 0.0 {
                return Some(Move {
                    action: "reveal",
                    x: cell.row,
                    y: cell.col,
                    risk,
                    reason: "枚举所有边界雷型后确定安全".to_string(),
                });
            }
            if !stats.cutoff && (risk - 1.0).abs() < f64::EPSILON {
                return Some(Move {
                    action: "flag",
                    x: cell.row,
                    y: cell.col,
                    risk,
                    reason: "枚举所有边界雷型后确定为雷".to_string(),
                });
            }
            if is_better_guess(board, (cell, risk), best_frontier) {
                best_frontier = Some((cell, risk));
            }
        }
    }

    let outside = board
        .unknown_cells()
        .into_iter()
        .filter(|cell| !exact_frontier.contains(cell))
        .collect::<Vec<_>>();
    let mines_left = remaining_mines as f64;
    let outside_risk = if outside.is_empty() {
        None
    } else {
        Some(((mines_left - expected_frontier_mines) / outside.len() as f64).clamp(0.0, 1.0))
    };

    if let Some(risk) = outside_risk
        && best_frontier
            .as_ref()
            .map(|best| {
                is_better_guess(
                    board,
                    (choose_spread_out_cell(board, &outside), risk),
                    Some(*best),
                )
            })
            .unwrap_or(true)
    {
        let cell = choose_spread_out_cell(board, &outside);
        return Some(Move {
            action: "reveal",
            x: cell.row,
            y: cell.col,
            risk,
            reason: if any_cutoff {
                "边界组合太多，选择约束外估算风险最低的位置".to_string()
            } else {
                "约束外整体雷率更低，选择远离数字边界的位置".to_string()
            },
        });
    }

    best_frontier.map(|(cell, risk)| Move {
        action: "reveal",
        x: cell.row,
        y: cell.col,
        risk,
        reason: if any_cutoff {
            "边界组合太多，选择当前估算风险最低的位置".to_string()
        } else {
            "无必然安全步，选择枚举概率最低的位置".to_string()
        },
    })
}

fn exact_probability_move(board: &Board, components: &[ComponentStats]) -> Option<Move> {
    if components.is_empty()
        || components
            .iter()
            .any(|stats| stats.cutoff || stats.solutions == 0)
    {
        return None;
    }

    let unknown_cells = board.unknown_cells();
    let remaining_mines = (board.mine_count() - board.flagged_count())
        .clamp(0, unknown_cells.len().min(i32::MAX as usize) as i32)
        as usize;
    let mut frontier = BTreeSet::new();
    for stats in components {
        frontier.extend(stats.vars.iter().copied());
    }
    let outside = unknown_cells
        .into_iter()
        .filter(|cell| !frontier.contains(cell))
        .collect::<Vec<_>>();

    let component_polys = components
        .iter()
        .map(|stats| component_solution_poly(stats, remaining_mines))
        .collect::<Vec<_>>();
    let outside_poly = outside_solution_poly(outside.len(), remaining_mines);

    let mut suffix = vec![vec![0.0; remaining_mines + 1]; components.len() + 1];
    suffix[components.len()] = outside_poly;
    for index in (0..components.len()).rev() {
        suffix[index] =
            convolve_mine_counts(&component_polys[index], &suffix[index + 1], remaining_mines);
    }

    let total_weight = suffix[0][remaining_mines];
    if total_weight <= 0.0 || !total_weight.is_finite() {
        return None;
    }

    let mut prefix = vec![vec![0.0; remaining_mines + 1]; components.len() + 1];
    prefix[0][0] = 1.0;
    for index in 0..components.len() {
        prefix[index + 1] =
            convolve_mine_counts(&prefix[index], &component_polys[index], remaining_mines);
    }

    let mut certain_mine = None;
    let mut best_reveal: Option<(Cell, f64)> = None;
    for (component_index, stats) in components.iter().enumerate() {
        let other_weights = convolve_mine_counts(
            &prefix[component_index],
            &suffix[component_index + 1],
            remaining_mines,
        );
        for (var_index, cell) in stats.vars.iter().copied().enumerate() {
            let mut numerator = 0.0;
            for (mines, hits) in stats.mine_hits_by_mines[var_index]
                .iter()
                .copied()
                .enumerate()
            {
                if mines > remaining_mines || hits == 0 {
                    continue;
                }
                numerator += hits as f64 * other_weights[remaining_mines - mines];
            }
            let risk = (numerator / total_weight).clamp(0.0, 1.0);
            if risk <= f64::EPSILON {
                return Some(Move {
                    action: "reveal",
                    x: cell.row,
                    y: cell.col,
                    risk,
                    reason: "全局雷数加权后确定安全".to_string(),
                });
            }
            if (1.0 - risk) <= f64::EPSILON {
                certain_mine.get_or_insert(cell);
                continue;
            }
            if is_better_guess(board, (cell, risk), best_reveal) {
                best_reveal = Some((cell, risk));
            }
        }
    }

    if !outside.is_empty() {
        let component_weight = &prefix[components.len()];
        let mut expected_outside_mines = 0.0;
        for outside_mines in 0..=remaining_mines.min(outside.len()) {
            let Some(component_mines) = remaining_mines.checked_sub(outside_mines) else {
                continue;
            };
            expected_outside_mines += outside_mines as f64
                * component_weight[component_mines]
                * combination_as_f64(outside.len(), outside_mines);
        }
        let risk = (expected_outside_mines / (outside.len() as f64 * total_weight)).clamp(0.0, 1.0);
        let cell = choose_spread_out_cell(board, &outside);
        if risk <= f64::EPSILON {
            return Some(Move {
                action: "reveal",
                x: cell.row,
                y: cell.col,
                risk,
                reason: "全局雷数加权后确定约束外格子安全".to_string(),
            });
        }
        if (1.0 - risk) <= f64::EPSILON {
            certain_mine.get_or_insert(cell);
        } else if is_better_guess(board, (cell, risk), best_reveal) {
            best_reveal = Some((cell, risk));
        }
    }

    if let Some(cell) = certain_mine {
        return Some(Move {
            action: "flag",
            x: cell.row,
            y: cell.col,
            risk: 1.0,
            reason: "全局雷数加权后确定为雷".to_string(),
        });
    }

    best_reveal.map(|(cell, risk)| Move {
        action: "reveal",
        x: cell.row,
        y: cell.col,
        risk,
        reason: "按总雷数、边界组合和约束外组合加权后选择风险最低的位置".to_string(),
    })
}

fn component_solution_poly(stats: &ComponentStats, max_mines: usize) -> Vec<f64> {
    let mut poly = vec![0.0; max_mines + 1];
    for (mines, solutions) in stats.solutions_by_mines.iter().copied().enumerate() {
        if mines <= max_mines {
            poly[mines] = solutions as f64;
        }
    }
    poly
}

fn outside_solution_poly(outside_count: usize, max_mines: usize) -> Vec<f64> {
    let mut poly = vec![0.0; max_mines + 1];
    for (mines, slot) in poly
        .iter_mut()
        .enumerate()
        .take(max_mines.min(outside_count) + 1)
    {
        *slot = combination_as_f64(outside_count, mines);
    }
    poly
}

fn convolve_mine_counts(left: &[f64], right: &[f64], max_mines: usize) -> Vec<f64> {
    let mut result = vec![0.0; max_mines + 1];
    for left_mines in 0..=max_mines.min(left.len().saturating_sub(1)) {
        let left_weight = left[left_mines];
        if left_weight == 0.0 {
            continue;
        }
        for right_mines in 0..=(max_mines - left_mines).min(right.len().saturating_sub(1)) {
            let right_weight = right[right_mines];
            if right_weight != 0.0 {
                result[left_mines + right_mines] += left_weight * right_weight;
            }
        }
    }
    result
}

fn combination_as_f64(n: usize, k: usize) -> f64 {
    if k > n {
        return 0.0;
    }
    let k = k.min(n - k);
    let mut result = 1.0;
    for step in 1..=k {
        result *= (n - k + step) as f64 / step as f64;
    }
    result
}

fn is_better_guess(board: &Board, candidate: (Cell, f64), best: Option<(Cell, f64)>) -> bool {
    let Some((best_cell, best_risk)) = best else {
        return true;
    };
    candidate.1 + f64::EPSILON < best_risk
        || ((candidate.1 - best_risk).abs() <= f64::EPSILON
            && guess_quality_key(board, candidate.0) > guess_quality_key(board, best_cell))
}

fn guess_quality_key(board: &Board, cell: Cell) -> (i32, i32, i32, i32, i32, i32) {
    let neighbors = board.neighbors(cell);
    let numbered_neighbors = neighbors
        .iter()
        .filter(|neighbor| board.is_revealed(**neighbor) && board.number(**neighbor).is_some())
        .count() as i32;
    let hidden_neighbors = neighbors
        .iter()
        .filter(|neighbor| !board.is_revealed(**neighbor) && !board.is_flagged(**neighbor))
        .count() as i32;
    let nearest_number = numbered_cells(board)
        .iter()
        .map(|numbered| chebyshev_distance(cell, *numbered))
        .min()
        .unwrap_or(0);
    let edge_bonus = i32::from(
        cell.row == 0 || cell.col == 0 || cell.row == board.rows - 1 || cell.col == board.cols - 1,
    );
    (
        numbered_neighbors,
        hidden_neighbors,
        nearest_number,
        edge_bonus,
        -cell.row,
        -cell.col,
    )
}

fn best_unconstrained_guess(board: &Board, fallback_risk: f64) -> Option<Move> {
    let unknown = board.unknown_cells();
    if unknown.is_empty() {
        return None;
    }
    let risk = if fallback_risk > 0.0 {
        fallback_risk
    } else {
        let mines_left = (board.mine_count() - board.flagged_count()).max(0) as f64;
        (mines_left / unknown.len() as f64).clamp(0.0, 1.0)
    };
    let cell = choose_spread_out_cell(board, &unknown);
    Some(Move {
        action: "reveal",
        x: cell.row,
        y: cell.col,
        risk,
        reason: "当前没有可用数字约束，只能按全局雷率选择风险最低的未知格".to_string(),
    })
}

fn numbered_cells(board: &Board) -> Vec<Cell> {
    board
        .cells()
        .into_iter()
        .filter(|cell| board.is_revealed(*cell) && board.number(*cell).is_some())
        .collect()
}

fn flagged_cells(board: &Board) -> Vec<Cell> {
    board
        .cells()
        .into_iter()
        .filter(|cell| board.is_flagged(*cell))
        .collect()
}

fn remaining_mines(board: &Board) -> usize {
    (board.mine_count() - board.flagged_count())
        .max(0)
        .min(board.unknown_cells().len().min(i32::MAX as usize) as i32) as usize
}

fn constraints(board: &Board) -> Vec<Constraint> {
    let mut constraints = Vec::new();
    let mut seen = HashSet::new();
    for numbered in numbered_cells(board) {
        let value = board.number(numbered).unwrap_or(0);
        let mut vars = BTreeSet::new();
        let mut flagged = 0;
        for neighbor in board.neighbors(numbered) {
            if board.is_flagged(neighbor) {
                flagged += 1;
            } else if !board.is_revealed(neighbor) {
                vars.insert(neighbor);
            }
        }
        let mines = value - flagged;
        if mines < 0 || mines > vars.len() as i32 || vars.is_empty() {
            continue;
        }
        let key = (vars.clone(), mines);
        if seen.insert(key) {
            constraints.push(Constraint { vars, mines });
        }
    }
    constraints
}

fn enumerate_components(
    constraints: &[Constraint],
    max_component_mines: Option<usize>,
) -> Vec<ComponentStats> {
    let mut all_vars = BTreeSet::new();
    for constraint in constraints {
        all_vars.extend(constraint.vars.iter().copied());
    }
    let vars = all_vars.into_iter().collect::<Vec<_>>();
    let var_index = vars
        .iter()
        .copied()
        .enumerate()
        .map(|(index, cell)| (cell, index))
        .collect::<HashMap<_, _>>();
    let mut dsu = Dsu::new(vars.len());
    for constraint in constraints {
        let indices = constraint
            .vars
            .iter()
            .filter_map(|cell| var_index.get(cell).copied())
            .collect::<Vec<_>>();
        for window in indices.windows(2) {
            dsu.union(window[0], window[1]);
        }
    }

    let mut groups: HashMap<usize, Vec<usize>> = HashMap::new();
    for index in 0..vars.len() {
        groups.entry(dsu.find(index)).or_default().push(index);
    }

    let mut stats = Vec::new();
    for group_indices in groups.values() {
        let group_set = group_indices.iter().copied().collect::<HashSet<_>>();
        let group_vars = group_indices
            .iter()
            .map(|index| vars[*index])
            .collect::<Vec<_>>();
        let local_by_global = group_indices
            .iter()
            .copied()
            .enumerate()
            .map(|(local, global)| (global, local))
            .collect::<HashMap<_, _>>();
        let group_constraints = constraints
            .iter()
            .filter_map(|constraint| {
                let indices = constraint
                    .vars
                    .iter()
                    .filter_map(|cell| var_index.get(cell).copied())
                    .collect::<Vec<_>>();
                if indices.iter().all(|index| group_set.contains(index)) {
                    Some(IndexedConstraint {
                        vars: indices
                            .iter()
                            .filter_map(|index| local_by_global.get(index).copied())
                            .collect(),
                        mines: constraint.mines,
                    })
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        stats.push(enumerate_component(
            group_vars,
            &group_constraints,
            max_component_mines,
        ));
    }
    stats
}

#[derive(Debug, Clone)]
struct IndexedConstraint {
    vars: Vec<usize>,
    mines: i32,
}

fn enumerate_component(
    vars: Vec<Cell>,
    constraints: &[IndexedConstraint],
    max_mines: Option<usize>,
) -> ComponentStats {
    let var_count = vars.len();
    let mut var_to_constraints = vec![Vec::new(); vars.len()];
    for (constraint_index, constraint) in constraints.iter().enumerate() {
        for var in &constraint.vars {
            if let Some(list) = var_to_constraints.get_mut(*var) {
                list.push(constraint_index);
            }
        }
    }
    let mut order = (0..vars.len()).collect::<Vec<_>>();
    order.sort_by_key(|var| std::cmp::Reverse(var_to_constraints[*var].len()));
    let mut assignment = vec![false; vars.len()];
    let mut assigned_mines = vec![0; constraints.len()];
    let mut unassigned = constraints
        .iter()
        .map(|constraint| constraint.vars.len() as i32)
        .collect::<Vec<_>>();
    let mut stats = ComponentStats {
        vars,
        mine_hits: vec![0; assignment.len()],
        solutions_by_mines: vec![0; var_count + 1],
        mine_hits_by_mines: vec![vec![0; var_count + 1]; var_count],
        ..ComponentStats::default()
    };
    let max_mines = max_mines.unwrap_or(var_count).min(var_count);

    let mut state = BacktrackingState {
        order: &order,
        var_to_constraints: &var_to_constraints,
        constraints,
        assignment: &mut assignment,
        assigned_mines: &mut assigned_mines,
        unassigned: &mut unassigned,
        stats: &mut stats,
        assigned_total_mines: 0,
        max_mines,
    };
    enumerate_backtracking(0, &mut state);
    stats
}

struct BacktrackingState<'a> {
    order: &'a [usize],
    var_to_constraints: &'a [Vec<usize>],
    constraints: &'a [IndexedConstraint],
    assignment: &'a mut [bool],
    assigned_mines: &'a mut [i32],
    unassigned: &'a mut [i32],
    stats: &'a mut ComponentStats,
    assigned_total_mines: usize,
    max_mines: usize,
}

fn enumerate_backtracking(depth: usize, state: &mut BacktrackingState<'_>) {
    if state.stats.solutions >= ENUMERATION_ASSIGNMENT_LIMIT {
        state.stats.cutoff = true;
        return;
    }
    if depth == state.order.len() {
        if state
            .constraints
            .iter()
            .enumerate()
            .all(|(index, constraint)| state.assigned_mines[index] == constraint.mines)
        {
            state.stats.solutions += 1;
            let mines = state.assignment.iter().filter(|mine| **mine).count();
            state.stats.mines_total += mines;
            if let Some(slot) = state.stats.solutions_by_mines.get_mut(mines) {
                *slot += 1;
            }
            for (index, mine) in state.assignment.iter().enumerate() {
                if *mine {
                    state.stats.mine_hits[index] += 1;
                    if let Some(slot) = state
                        .stats
                        .mine_hits_by_mines
                        .get_mut(index)
                        .and_then(|items| items.get_mut(mines))
                    {
                        *slot += 1;
                    }
                }
            }
        }
        return;
    }

    let var = state.order[depth];
    for is_mine in [false, true] {
        state.assignment[var] = is_mine;
        for constraint_index in &state.var_to_constraints[var] {
            state.unassigned[*constraint_index] -= 1;
            if is_mine {
                state.assigned_mines[*constraint_index] += 1;
            }
        }
        if is_mine {
            state.assigned_total_mines += 1;
        }
        if state.assigned_total_mines <= state.max_mines
            && constraints_possible(state.constraints, state.assigned_mines, state.unassigned)
        {
            enumerate_backtracking(depth + 1, state);
        }
        if is_mine {
            state.assigned_total_mines -= 1;
        }
        for constraint_index in &state.var_to_constraints[var] {
            if is_mine {
                state.assigned_mines[*constraint_index] -= 1;
            }
            state.unassigned[*constraint_index] += 1;
        }
        if state.stats.cutoff {
            return;
        }
    }
    state.assignment[var] = false;
}

fn constraints_possible(
    constraints: &[IndexedConstraint],
    assigned_mines: &[i32],
    unassigned: &[i32],
) -> bool {
    constraints.iter().enumerate().all(|(index, constraint)| {
        assigned_mines[index] <= constraint.mines
            && assigned_mines[index] + unassigned[index] >= constraint.mines
    })
}

#[derive(Debug, Clone)]
struct Dsu {
    parent: Vec<usize>,
    rank: Vec<u8>,
}

impl Dsu {
    fn new(size: usize) -> Self {
        Self {
            parent: (0..size).collect(),
            rank: vec![0; size],
        }
    }

    fn find(&mut self, index: usize) -> usize {
        if self.parent[index] != index {
            self.parent[index] = self.find(self.parent[index]);
        }
        self.parent[index]
    }

    fn union(&mut self, left: usize, right: usize) {
        let left = self.find(left);
        let right = self.find(right);
        if left == right {
            return;
        }
        if self.rank[left] < self.rank[right] {
            self.parent[left] = right;
        } else if self.rank[left] > self.rank[right] {
            self.parent[right] = left;
        } else {
            self.parent[right] = left;
            self.rank[left] += 1;
        }
    }
}

fn choose_spread_out_cell(board: &Board, cells: &[Cell]) -> Cell {
    let numbered = numbered_cells(board);
    cells
        .iter()
        .copied()
        .max_by_key(|cell| {
            let nearest_number = numbered
                .iter()
                .map(|numbered| chebyshev_distance(*cell, *numbered))
                .min()
                .unwrap_or(0);
            let edge_bonus = if cell.row == 0
                || cell.col == 0
                || cell.row == board.rows - 1
                || cell.col == board.cols - 1
            {
                1
            } else {
                0
            };
            (nearest_number, edge_bonus, -cell.row, -cell.col)
        })
        .unwrap_or(Cell { row: 0, col: 0 })
}

fn chebyshev_distance(left: Cell, right: Cell) -> i32 {
    (left.row - right.row)
        .abs()
        .max((left.col - right.col).abs())
}

fn safe_len_as_i32(cells: &BTreeSet<Cell>) -> i32 {
    cells.len().min(i32::MAX as usize) as i32
}

fn checked_cell_count(rows: i32, cols: i32) -> Result<usize, String> {
    let rows = usize::try_from(rows).map_err(|_| "扫雷行数无效".to_string())?;
    let cols = usize::try_from(cols).map_err(|_| "扫雷列数无效".to_string())?;
    rows.checked_mul(cols)
        .filter(|count| *count > 0)
        .ok_or_else(|| "扫雷棋盘尺寸无效".to_string())
}

fn flatten_bool_matrix(
    rows: i32,
    cols: i32,
    matrix: &[Vec<bool>],
    field: &str,
) -> Result<Vec<bool>, String> {
    let rows_usize = usize::try_from(rows).map_err(|_| "扫雷行数无效".to_string())?;
    let cols_usize = usize::try_from(cols).map_err(|_| "扫雷列数无效".to_string())?;
    if matrix.len() != rows_usize {
        return Err(format!("扫雷 {} 行数不匹配", field));
    }
    let mut flattened = Vec::with_capacity(rows_usize * cols_usize);
    for row in matrix {
        if row.len() != cols_usize {
            return Err(format!("扫雷 {} 列数不匹配", field));
        }
        flattened.extend(row.iter().copied());
    }
    Ok(flattened)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_board(rows: i32, cols: i32, mines: i32) -> Board {
        Board::new(
            rows,
            cols,
            mines,
            &vec![vec![false; cols as usize]; rows as usize],
            &vec![vec![false; cols as usize]; rows as usize],
        )
        .unwrap()
    }

    #[test]
    fn first_move_reveals_top_left() {
        let board = empty_board(8, 8, 10);

        let next = next_move(&board).unwrap();

        assert_eq!(next.action, "reveal");
        assert_eq!((next.x, next.y), (0, 0));
        assert_eq!(next.risk, 0.0);
    }

    #[test]
    fn deterministic_rule_flags_last_hidden_neighbor() {
        let mut board = empty_board(3, 4, 2);
        for cell in [
            Cell { row: 0, col: 0 },
            Cell { row: 0, col: 1 },
            Cell { row: 0, col: 2 },
            Cell { row: 1, col: 0 },
            Cell { row: 1, col: 1 },
            Cell { row: 1, col: 2 },
            Cell { row: 2, col: 0 },
            Cell { row: 2, col: 1 },
        ] {
            board.reveal_number(cell, 1).unwrap();
        }

        let next = next_move(&board).unwrap();

        assert_eq!(next.action, "flag");
        assert_eq!((next.x, next.y), (2, 2));
    }

    #[test]
    fn deterministic_rule_chords_when_flags_satisfy_number() {
        let mut board = empty_board(2, 3, 1);
        board.reveal_number(Cell { row: 0, col: 0 }, 1).unwrap();
        board.apply_flag(Cell { row: 1, col: 0 }, true).unwrap();

        let next = next_move(&board).unwrap();

        assert_eq!(next.action, "chord");
        assert_eq!((next.x, next.y), (0, 0));
    }

    #[test]
    fn repair_rule_unflags_contradicting_manual_flag() {
        let mut board = empty_board(2, 2, 1);
        board.reveal_number(Cell { row: 0, col: 0 }, 0).unwrap();
        board.apply_flag(Cell { row: 0, col: 1 }, true).unwrap();

        let next = next_move(&board).unwrap();

        assert_eq!(next.action, "unflag");
        assert_eq!((next.x, next.y), (0, 1));
    }

    #[test]
    fn subset_rule_finds_safe_difference() {
        let mut board = empty_board(2, 4, 1);
        board.reveal_number(Cell { row: 0, col: 0 }, 1).unwrap();
        board.reveal_number(Cell { row: 0, col: 2 }, 1).unwrap();
        board.apply_flag(Cell { row: 1, col: 0 }, true).unwrap();

        let next = next_move(&board).unwrap();

        assert_eq!(next.action, "chord");
    }

    #[test]
    fn overlap_rule_finds_mines_from_non_subset_constraints() {
        let a = Cell { row: 0, col: 0 };
        let b = Cell { row: 0, col: 1 };
        let c = Cell { row: 0, col: 2 };
        let d = Cell { row: 0, col: 3 };
        let constraints = vec![
            Constraint {
                vars: BTreeSet::from([a, b, c]),
                mines: 1,
            },
            Constraint {
                vars: BTreeSet::from([b, c, d]),
                mines: 2,
            },
        ];

        let next = overlap_constraint_move(&constraints).unwrap();

        assert_eq!(next.action, "flag");
        assert_eq!((next.x, next.y), (d.row, d.col));
    }

    #[test]
    fn probability_move_uses_exact_frontier_when_no_safe_move_exists() {
        let mut board = empty_board(1, 3, 1);
        board.reveal_number(Cell { row: 0, col: 1 }, 1).unwrap();

        let next = next_move(&board).unwrap();

        assert_eq!(next.action, "reveal");
        assert!((next.risk - 0.5).abs() < 0.0001);
    }

    #[test]
    fn probability_move_uses_global_mine_count_across_component_solutions() {
        let revealed = vec![vec![true, true, true], vec![false, false, false]];
        let flagged = vec![vec![false; 3]; 2];
        let mut board = Board::new(2, 3, 1, &revealed, &flagged).unwrap();
        board.reveal_number(Cell { row: 0, col: 0 }, 1).unwrap();
        board.reveal_number(Cell { row: 0, col: 2 }, 1).unwrap();

        let next = next_move(&board).unwrap();

        assert_eq!(next.action, "reveal");
        assert_eq!((next.x, next.y), (1, 0));
        assert_eq!(next.risk, 0.0);
    }
}
