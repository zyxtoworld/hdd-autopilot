use crate::model::NonogramSession;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NonogramStep {
    pub action: String,
    pub r: i32,
    pub c: i32,
}

pub fn solve(session: &NonogramSession) -> Result<Vec<NonogramStep>, String> {
    let width = usize_from_i32(session.width, "nonogram width")?;
    let height = usize_from_i32(session.height, "nonogram height")?;
    let solution = solve_grid(width, height, &session.row_clues, &session.col_clues)?;
    let mut steps = Vec::new();
    for (r, row) in solution.iter().enumerate() {
        for (c, &filled) in row.iter().enumerate() {
            if filled && session.cells.get(r).and_then(|line| line.get(c)).copied() != Some(1) {
                steps.push(NonogramStep {
                    action: "fill".to_string(),
                    r: r as i32,
                    c: c as i32,
                });
            }
        }
    }
    Ok(steps)
}

fn solve_grid(
    width: usize,
    height: usize,
    row_clues: &[Vec<i32>],
    col_clues: &[Vec<i32>],
) -> Result<Vec<Vec<bool>>, String> {
    if row_clues.len() != height || col_clues.len() != width {
        return Err("nonogram clue dimensions do not match board size".to_string());
    }
    let row_patterns = row_clues
        .iter()
        .map(|clue| line_patterns(width, clue))
        .collect::<Result<Vec<_>, _>>()?;
    let col_patterns = col_clues
        .iter()
        .map(|clue| line_patterns(height, clue))
        .collect::<Result<Vec<_>, _>>()?;
    let known = vec![vec![None; width]; height];
    search(known, &row_patterns, &col_patterns)
        .ok_or_else(|| "nonogram has no solution matching the clues".to_string())
}

type KnownGrid = Vec<Vec<Option<bool>>>;
type LinePatterns = Vec<Vec<Vec<bool>>>;

#[derive(Clone)]
struct Propagation {
    known: KnownGrid,
    rows: LinePatterns,
    cols: LinePatterns,
}

fn search(
    known: KnownGrid,
    row_patterns: &LinePatterns,
    col_patterns: &LinePatterns,
) -> Option<Vec<Vec<bool>>> {
    let propagated = propagate(known, row_patterns, col_patterns)?;
    if propagated
        .known
        .iter()
        .all(|row| row.iter().all(Option::is_some))
    {
        return Some(
            propagated
                .known
                .into_iter()
                .map(|row| {
                    row.into_iter()
                        .map(|value| value.unwrap_or(false))
                        .collect()
                })
                .collect(),
        );
    }

    let mut best: Option<(bool, usize, usize)> = None;
    for (index, candidates) in propagated.rows.iter().enumerate() {
        if candidates.len() > 1 && best.is_none_or(|(_, _, count)| candidates.len() < count) {
            best = Some((true, index, candidates.len()));
        }
    }
    for (index, candidates) in propagated.cols.iter().enumerate() {
        if candidates.len() > 1 && best.is_none_or(|(_, _, count)| candidates.len() < count) {
            best = Some((false, index, candidates.len()));
        }
    }

    let (is_row, index, _) = best?;
    let candidates = if is_row {
        propagated.rows[index].clone()
    } else {
        propagated.cols[index].clone()
    };
    for candidate in candidates {
        let mut next = propagated.known.clone();
        if is_row {
            for (c, &value) in candidate.iter().enumerate() {
                next[index][c] = Some(value);
            }
        } else {
            for (r, &value) in candidate.iter().enumerate() {
                next[r][index] = Some(value);
            }
        }
        if let Some(solution) = search(next, row_patterns, col_patterns) {
            return Some(solution);
        }
    }
    None
}

fn propagate(
    mut known: KnownGrid,
    row_patterns: &LinePatterns,
    col_patterns: &LinePatterns,
) -> Option<Propagation> {
    let height = row_patterns.len();
    let width = col_patterns.len();
    let mut rows = row_patterns.clone();
    let mut cols = col_patterns.clone();
    let mut changed = true;
    while changed {
        changed = false;
        for r in 0..height {
            rows[r].retain(|pattern| {
                pattern
                    .iter()
                    .enumerate()
                    .all(|(c, value)| known[r][c].is_none_or(|known| known == *value))
            });
            if rows[r].is_empty() {
                return None;
            }
            for c in 0..width {
                let value = rows[r][0][c];
                if rows[r].iter().all(|pattern| pattern[c] == value) && known[r][c] != Some(value) {
                    known[r][c] = Some(value);
                    changed = true;
                }
            }
        }
        for c in 0..width {
            cols[c].retain(|pattern| {
                pattern
                    .iter()
                    .enumerate()
                    .all(|(r, value)| known[r][c].is_none_or(|known| known == *value))
            });
            if cols[c].is_empty() {
                return None;
            }
            for r in 0..height {
                let value = cols[c][0][r];
                if cols[c].iter().all(|pattern| pattern[r] == value) && known[r][c] != Some(value) {
                    known[r][c] = Some(value);
                    changed = true;
                }
            }
        }
    }
    Some(Propagation { known, rows, cols })
}

fn line_patterns(length: usize, clues: &[i32]) -> Result<Vec<Vec<bool>>, String> {
    let clues = clues
        .iter()
        .copied()
        .filter(|value| *value > 0)
        .map(|value| usize::try_from(value).map_err(|_| "invalid nonogram clue".to_string()))
        .collect::<Result<Vec<_>, _>>()?;
    if clues.is_empty() {
        return Ok(vec![vec![false; length]]);
    }
    let mut patterns = Vec::new();
    build_line_patterns(length, &clues, 0, 0, Vec::new(), &mut patterns);
    if patterns.is_empty() {
        Err("a nonogram row or column has no valid pattern".to_string())
    } else {
        Ok(patterns)
    }
}

fn build_line_patterns(
    length: usize,
    clues: &[usize],
    clue_index: usize,
    pos: usize,
    current: Vec<bool>,
    out: &mut Vec<Vec<bool>>,
) {
    if clue_index == clues.len() {
        let mut line = current;
        line.resize(length, false);
        out.push(line);
        return;
    }
    let remaining =
        clues[clue_index + 1..].iter().sum::<usize>() + clues.len().saturating_sub(clue_index + 1);
    if clues[clue_index] + remaining > length {
        return;
    }
    let max_start = length - remaining - clues[clue_index];
    for start in pos..=max_start {
        let mut line = current.clone();
        line.resize(start, false);
        line.extend(std::iter::repeat_n(true, clues[clue_index]));
        if clue_index + 1 < clues.len() {
            line.push(false);
        }
        build_line_patterns(length, clues, clue_index + 1, line.len(), line, out);
    }
}

fn usize_from_i32(value: i32, label: &str) -> Result<usize, String> {
    usize::try_from(value)
        .ok()
        .filter(|value| *value > 0)
        .ok_or_else(|| format!("{label} is invalid"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn solver_solves_known_easy_board() {
        let session = NonogramSession {
            width: 5,
            height: 5,
            row_clues: vec![vec![1], vec![3, 1], vec![3], vec![3], vec![4]],
            col_clues: vec![vec![4], vec![4], vec![5], vec![1], vec![1]],
            cells: vec![vec![0; 5]; 5],
            ..NonogramSession::default()
        };

        let steps = solve(&session).unwrap();

        assert_eq!(steps.len(), 15);
        assert!(steps.iter().all(|step| step.action == "fill"));
    }
}
