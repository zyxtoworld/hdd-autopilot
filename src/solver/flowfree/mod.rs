use std::collections::{HashMap, HashSet, VecDeque};

use crate::model::{FlowfreeEndpoint, FlowfreePoint, FlowfreeSession};

const DIRS: [(i32, i32); 4] = [(-1, 0), (1, 0), (0, -1), (0, 1)];
const SEARCH_LIMIT: usize = 2_000_000;

#[derive(Clone)]
struct State {
    grid: Vec<Vec<i32>>,
    paths: HashMap<i32, Vec<FlowfreePoint>>,
    complete: HashSet<i32>,
    calls: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlowfreeStep {
    pub action: String,
    pub color: i32,
    pub r: i32,
    pub c: i32,
}

pub fn solve(session: &FlowfreeSession) -> Result<Vec<FlowfreeStep>, String> {
    let height = usize_from_i32(session.height, "flowfree height")?;
    let width = usize_from_i32(session.width, "flowfree width")?;
    let endpoints = session.endpoints.clone();
    if endpoints.is_empty() {
        return Err("flowfree board has no endpoints".to_string());
    }

    let mut state = State {
        grid: vec![vec![0; width]; height],
        paths: HashMap::new(),
        complete: HashSet::new(),
        calls: 0,
    };
    for FlowfreeEndpoint(color, start, end) in &endpoints {
        point_index(*start, width, height)?;
        point_index(*end, width, height)?;
        state.grid[start[0] as usize][start[1] as usize] = *color;
        state.grid[end[0] as usize][end[1] as usize] = *color;
        state.paths.insert(*color, vec![*start]);
    }

    let solved = search(&endpoints, &mut state, width, height)
        .ok_or_else(|| "flowfree has no reachable endpoint solution".to_string())?;
    let mut colors = endpoints
        .iter()
        .map(|endpoint| endpoint.0)
        .collect::<Vec<_>>();
    colors.sort_unstable();

    let mut steps = Vec::new();
    for color in colors {
        let Some(path) = solved.paths.get(&color) else {
            continue;
        };
        for point in path {
            steps.push(FlowfreeStep {
                action: "paint".to_string(),
                color,
                r: point[0],
                c: point[1],
            });
        }
    }
    Ok(steps)
}

fn search(
    endpoints: &[FlowfreeEndpoint],
    state: &mut State,
    width: usize,
    height: usize,
) -> Option<State> {
    state.calls += 1;
    if state.calls > SEARCH_LIMIT {
        return None;
    }
    if endpoints
        .iter()
        .all(|endpoint| state.complete.contains(&endpoint.0))
    {
        return Some(state.clone());
    }
    for endpoint in endpoints {
        if !state.complete.contains(&endpoint.0) && !reachable(endpoint, state, width, height) {
            return None;
        }
    }

    let mut best: Option<(FlowfreeEndpoint, Vec<(FlowfreePoint, bool)>)> = None;
    for endpoint in endpoints {
        if state.complete.contains(&endpoint.0) {
            continue;
        }
        let moves = legal_moves(endpoint, state, width, height);
        if moves.is_empty() {
            return None;
        }
        if best
            .as_ref()
            .is_none_or(|(_, best_moves)| moves.len() < best_moves.len())
        {
            best = Some((endpoint.clone(), moves));
        }
    }

    let (endpoint, moves) = best?;
    for (next, is_goal) in moves {
        let path = state.paths.get_mut(&endpoint.0)?;
        path.push(next);
        let previous = state.grid[next[0] as usize][next[1] as usize];
        if is_goal {
            state.complete.insert(endpoint.0);
        } else {
            state.grid[next[0] as usize][next[1] as usize] = endpoint.0;
        }

        if let Some(result) = search(endpoints, state, width, height) {
            return Some(result);
        }

        if is_goal {
            state.complete.remove(&endpoint.0);
        } else {
            state.grid[next[0] as usize][next[1] as usize] = previous;
        }
        state.paths.get_mut(&endpoint.0)?.pop();
    }
    None
}

fn legal_moves(
    endpoint: &FlowfreeEndpoint,
    state: &State,
    width: usize,
    height: usize,
) -> Vec<(FlowfreePoint, bool)> {
    let Some(path) = state.paths.get(&endpoint.0) else {
        return Vec::new();
    };
    let Some(&tip) = path.last() else {
        return Vec::new();
    };
    let goal = endpoint.2;
    let mut moves = Vec::new();
    for (dr, dc) in DIRS {
        let next = [tip[0] + dr, tip[1] + dc];
        if point_index(next, width, height).is_err() {
            continue;
        }
        let is_goal = next == goal;
        if is_goal || state.grid[next[0] as usize][next[1] as usize] == 0 {
            moves.push((next, is_goal));
        }
    }
    moves.sort_by_key(|(point, _)| manhattan(*point, goal));
    moves
}

fn reachable(endpoint: &FlowfreeEndpoint, state: &State, width: usize, height: usize) -> bool {
    let Some(path) = state.paths.get(&endpoint.0) else {
        return false;
    };
    let Some(&start) = path.last() else {
        return false;
    };
    let goal = endpoint.2;
    let mut seen = HashSet::new();
    let mut queue = VecDeque::from([start]);
    seen.insert(start);
    while let Some(point) = queue.pop_front() {
        if point == goal {
            return true;
        }
        for (dr, dc) in DIRS {
            let next = [point[0] + dr, point[1] + dc];
            if point_index(next, width, height).is_err() || !seen.insert(next) {
                continue;
            }
            if next == goal || state.grid[next[0] as usize][next[1] as usize] == 0 {
                queue.push_back(next);
            }
        }
    }
    false
}

fn manhattan(a: FlowfreePoint, b: FlowfreePoint) -> i32 {
    (a[0] - b[0]).abs() + (a[1] - b[1]).abs()
}

fn usize_from_i32(value: i32, label: &str) -> Result<usize, String> {
    usize::try_from(value)
        .ok()
        .filter(|value| *value > 0)
        .ok_or_else(|| format!("{label} is invalid"))
}

fn point_index(point: FlowfreePoint, width: usize, height: usize) -> Result<usize, String> {
    let r = usize::try_from(point[0]).map_err(|_| "invalid coordinate".to_string())?;
    let c = usize::try_from(point[1]).map_err(|_| "invalid coordinate".to_string())?;
    if r >= height || c >= width {
        return Err("coordinate is outside the board".to_string());
    }
    Ok(r * width + c)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn solver_connects_known_easy_board() {
        let session = FlowfreeSession {
            width: 5,
            height: 5,
            endpoints: vec![
                FlowfreeEndpoint(1, [0, 0], [4, 0]),
                FlowfreeEndpoint(2, [0, 4], [4, 4]),
                FlowfreeEndpoint(3, [2, 1], [2, 3]),
                FlowfreeEndpoint(4, [1, 2], [3, 2]),
            ],
            ..FlowfreeSession::default()
        };

        let steps = solve(&session).unwrap();

        assert!(!steps.is_empty());
    }
}
