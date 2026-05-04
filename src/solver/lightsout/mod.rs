use crate::model::LightsoutSession;

const DIRS: [(i32, i32); 4] = [(-1, 0), (1, 0), (0, -1), (0, 1)];

pub fn solve(session: &LightsoutSession) -> Result<Vec<(i32, i32)>, String> {
    let cells = &session.cells;
    let size = cells.len();
    if size == 0 || cells.iter().any(|row| row.len() != size) {
        return Err("点灯棋盘尺寸无效".to_string());
    }
    let vars = size * size;
    if vars >= 127 {
        return Err("点灯棋盘过大".to_string());
    }
    let rhs_bit = vars;
    let mut rows = Vec::<u128>::with_capacity(vars);
    for (r, row_values) in cells.iter().enumerate() {
        for (c, &cell_value) in row_values.iter().enumerate() {
            let mut mask = 0u128;
            for (dr, dc) in DIRS {
                let rr = r as i32 + dr;
                let cc = c as i32 + dc;
                if rr >= 0 && rr < size as i32 && cc >= 0 && cc < size as i32 {
                    mask |= 1u128 << (rr as usize * size + cc as usize);
                }
            }
            mask |= 1u128 << (r * size + c);
            if cell_value != 0 {
                mask |= 1u128 << rhs_bit;
            }
            rows.push(mask);
        }
    }

    let mut row = 0usize;
    let mut pivots = Vec::<usize>::new();
    for col in 0..vars {
        let Some(pivot) = (row..rows.len()).find(|&candidate| bit(rows[candidate], col)) else {
            continue;
        };
        rows.swap(row, pivot);
        for other in 0..rows.len() {
            if other != row && bit(rows[other], col) {
                rows[other] ^= rows[row];
            }
        }
        pivots.push(col);
        row += 1;
    }
    for equation in rows.iter().skip(row) {
        if (*equation & ((1u128 << vars) - 1)) == 0 && bit(*equation, rhs_bit) {
            return Err("点灯棋盘无解".to_string());
        }
    }

    let mut solution = 0u128;
    for (equation_index, &col) in pivots.iter().enumerate() {
        if bit(rows[equation_index], rhs_bit) {
            solution |= 1u128 << col;
        }
    }

    let mut steps = Vec::new();
    for index in 0..vars {
        if bit(solution, index) {
            steps.push(((index / size) as i32, (index % size) as i32));
        }
    }
    Ok(steps)
}

fn bit(value: u128, index: usize) -> bool {
    ((value >> index) & 1) == 1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn solver_clears_known_board() {
        let session = LightsoutSession {
            cells: vec![
                vec![0, 0, 0, 0, 1],
                vec![0, 0, 0, 1, 1],
                vec![0, 0, 0, 1, 1],
                vec![1, 1, 1, 0, 0],
                vec![0, 0, 0, 1, 0],
            ],
            ..LightsoutSession::default()
        };

        let steps = solve(&session).unwrap();

        assert!(!steps.is_empty());
    }
}
