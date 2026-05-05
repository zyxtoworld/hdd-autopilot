use std::collections::{HashMap, HashSet};

use crate::model::{ArrowOutArrow, ArrowOutObstacle, ArrowOutPoint, ArrowOutSession};

const DIRS: [(&str, (i32, i32)); 4] = [
    ("up", (-1, 0)),
    ("down", (1, 0)),
    ("left", (0, -1)),
    ("right", (0, 1)),
];

const SHAPES_BASE: [(&str, &[ArrowOutPoint]); 8] = [
    ("I1", &[[0, 0]]),
    ("I2", &[[0, 0], [1, 0]]),
    ("I3", &[[0, 0], [1, 0], [2, 0]]),
    ("L2L", &[[0, 0], [1, 0], [1, -1]]),
    ("L2R", &[[0, 0], [1, 0], [1, 1]]),
    ("Z3L", &[[0, 0], [1, 0], [1, -1], [2, -1]]),
    ("Z3R", &[[0, 0], [1, 0], [1, 1], [2, 1]]),
    ("UL", &[[0, 0], [1, 0], [2, 0], [2, 1], [1, 1]]),
];

pub fn solve(session: &ArrowOutSession) -> Result<Vec<i32>, String> {
    let height = usize_from_i32(session.height, "arrow-out height")?;
    let width = usize_from_i32(session.width, "arrow-out width")?;
    let arrows = normalized_arrows(&session.arrows, width, height)?;
    if arrows.is_empty() {
        return Ok(Vec::new());
    }
    let obstacles = obstacle_set(&session.obstacles, width, height)?;
    let mut remaining = arrows.keys().copied().collect::<HashSet<_>>();
    let mut clicks = Vec::with_capacity(remaining.len());

    while !remaining.is_empty() {
        let occupied = occupied_cells(&arrows, &remaining);
        let mut clearable = remaining
            .iter()
            .filter_map(|id| {
                let arrow = arrows.get(id)?;
                if is_blocked(arrow, width, height, &occupied, &obstacles) {
                    return None;
                }
                Some((exit_distance(arrow, width, height), arrow.id))
            })
            .collect::<Vec<_>>();
        if clearable.is_empty() {
            return Err("arrow-out board is deadlocked".to_string());
        }
        clearable.sort_by_key(|(exit_distance, id)| (*exit_distance, *id));
        let id = clearable[0].1;
        remaining.remove(&id);
        clicks.push(id);
    }

    Ok(clicks)
}

fn normalized_arrows(
    arrows: &[ArrowOutArrow],
    width: usize,
    height: usize,
) -> Result<HashMap<i32, ArrowOutArrow>, String> {
    let mut result = HashMap::new();
    for arrow in arrows {
        if arrow.id < 0 || result.contains_key(&arrow.id) {
            return Err("arrow-out arrows contain invalid ids".to_string());
        }
        if direction_delta(&arrow.dir).is_none() {
            return Err("arrow-out arrow has invalid direction".to_string());
        }
        point_index([arrow.r, arrow.c], width, height)?;
        for cell in arrow_cells(arrow) {
            point_index(cell, width, height)?;
        }
        result.insert(arrow.id, arrow.clone());
    }
    Ok(result)
}

fn obstacle_set(
    obstacles: &[ArrowOutObstacle],
    width: usize,
    height: usize,
) -> Result<HashSet<ArrowOutPoint>, String> {
    let mut result = HashSet::new();
    for obstacle in obstacles {
        let point = [obstacle.r, obstacle.c];
        point_index(point, width, height)?;
        result.insert(point);
    }
    Ok(result)
}

fn occupied_cells(
    arrows: &HashMap<i32, ArrowOutArrow>,
    remaining: &HashSet<i32>,
) -> HashMap<ArrowOutPoint, i32> {
    let mut occupied = HashMap::new();
    for id in remaining {
        let Some(arrow) = arrows.get(id) else {
            continue;
        };
        for cell in arrow_cells(arrow) {
            occupied.entry(cell).or_insert(*id);
        }
    }
    occupied
}

fn is_blocked(
    arrow: &ArrowOutArrow,
    width: usize,
    height: usize,
    occupied: &HashMap<ArrowOutPoint, i32>,
    obstacles: &HashSet<ArrowOutPoint>,
) -> bool {
    exit_path_cells(arrow, width, height).any(|point| {
        obstacles.contains(&point)
            || occupied
                .get(&point)
                .is_some_and(|occupier| *occupier != arrow.id)
    })
}

fn exit_distance(arrow: &ArrowOutArrow, width: usize, height: usize) -> usize {
    exit_path_cells(arrow, width, height).count()
}

fn exit_path_cells(
    arrow: &ArrowOutArrow,
    width: usize,
    height: usize,
) -> impl Iterator<Item = ArrowOutPoint> {
    let (dr, dc) = direction_delta(&arrow.dir).unwrap_or((0, 0));
    let mut current = [arrow.r + dr, arrow.c + dc];
    std::iter::from_fn(move || {
        if point_index(current, width, height).is_err() {
            return None;
        }
        let point = current;
        current = [current[0] + dr, current[1] + dc];
        Some(point)
    })
}

fn arrow_cells(arrow: &ArrowOutArrow) -> Vec<ArrowOutPoint> {
    if !arrow.body.is_empty() {
        return arrow
            .body
            .iter()
            .map(|offset| [arrow.r + offset[0], arrow.c + offset[1]])
            .collect();
    }
    shape_cells(&arrow.shape, &arrow.dir)
        .into_iter()
        .map(|offset| [arrow.r + offset[0], arrow.c + offset[1]])
        .collect()
}

fn shape_cells(shape: &str, direction: &str) -> Vec<ArrowOutPoint> {
    let base = SHAPES_BASE
        .iter()
        .find(|(name, _)| *name == shape)
        .map(|(_, cells)| *cells)
        .unwrap_or(SHAPES_BASE[0].1);
    base.iter()
        .map(|point| rotate_cell(*point, direction))
        .collect()
}

fn rotate_cell(point: ArrowOutPoint, direction: &str) -> ArrowOutPoint {
    let [dr, dc] = point;
    match direction {
        "up" => [dr, dc],
        "right" => [dc, -dr],
        "down" => [-dr, -dc],
        "left" => [-dc, dr],
        _ => [dr, dc],
    }
}

fn direction_delta(direction: &str) -> Option<(i32, i32)> {
    DIRS.iter()
        .find(|(name, _)| *name == direction)
        .map(|(_, delta)| *delta)
}

fn point_index(point: ArrowOutPoint, width: usize, height: usize) -> Result<usize, String> {
    let r = usize::try_from(point[0]).map_err(|_| "invalid coordinate".to_string())?;
    let c = usize::try_from(point[1]).map_err(|_| "invalid coordinate".to_string())?;
    if r >= height || c >= width {
        return Err("coordinate is outside the board".to_string());
    }
    Ok(r * width + c)
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
    fn solver_clears_unblocked_arrows_in_dependency_order() {
        let session = ArrowOutSession {
            width: 3,
            height: 1,
            arrows: vec![
                ArrowOutArrow {
                    id: 1,
                    r: 0,
                    c: 0,
                    dir: "right".to_string(),
                    body: vec![[0, 0]],
                    ..ArrowOutArrow::default()
                },
                ArrowOutArrow {
                    id: 2,
                    r: 0,
                    c: 2,
                    dir: "right".to_string(),
                    body: vec![[0, 0]],
                    ..ArrowOutArrow::default()
                },
            ],
            ..ArrowOutSession::default()
        };

        assert_eq!(solve(&session).unwrap(), vec![2, 1]);
    }

    #[test]
    fn solver_rejects_deadlocked_board() {
        let session = ArrowOutSession {
            width: 3,
            height: 1,
            arrows: vec![
                ArrowOutArrow {
                    id: 1,
                    r: 0,
                    c: 0,
                    dir: "right".to_string(),
                    body: vec![[0, 0]],
                    ..ArrowOutArrow::default()
                },
                ArrowOutArrow {
                    id: 2,
                    r: 0,
                    c: 2,
                    dir: "left".to_string(),
                    body: vec![[0, 0]],
                    ..ArrowOutArrow::default()
                },
            ],
            ..ArrowOutSession::default()
        };

        assert!(solve(&session).is_err());
    }

    #[test]
    fn solver_uses_arrow_body_as_blocker() {
        let session = ArrowOutSession {
            width: 4,
            height: 2,
            arrows: vec![
                ArrowOutArrow {
                    id: 1,
                    r: 0,
                    c: 0,
                    dir: "right".to_string(),
                    body: vec![[0, 0]],
                    ..ArrowOutArrow::default()
                },
                ArrowOutArrow {
                    id: 2,
                    r: 1,
                    c: 2,
                    dir: "down".to_string(),
                    body: vec![[0, 0], [-1, 0]],
                    ..ArrowOutArrow::default()
                },
            ],
            ..ArrowOutSession::default()
        };

        assert_eq!(solve(&session).unwrap(), vec![2, 1]);
    }
}
