use std::collections::{HashMap, HashSet, VecDeque};

use crate::model::{FlowfreeEndpoint, FlowfreePoint, FlowfreeSession};

const DIRS: [(i32, i32); 4] = [(-1, 0), (1, 0), (0, -1), (0, 1)];
const SEARCH_LIMIT: usize = 5_000_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlowfreeStep {
    pub action: String,
    pub color: i32,
    pub r: i32,
    pub c: i32,
}

#[derive(Debug, Clone)]
struct EndpointPlan {
    color: i32,
    start: FlowfreePoint,
    end: FlowfreePoint,
}

#[derive(Clone)]
struct State {
    grid: Vec<Vec<i32>>,
    paths: HashMap<i32, Vec<FlowfreePoint>>,
    complete: HashSet<i32>,
    calls: usize,
}

pub fn solve(session: &FlowfreeSession) -> Result<Vec<FlowfreeStep>, String> {
    let height = usize_from_i32(session.height, "flowfree height")?;
    let width = usize_from_i32(session.width, "flowfree width")?;
    let endpoints = normalized_endpoints(&session.endpoints, width, height)?;
    if endpoints.is_empty() {
        return Err("flowfree board has no endpoints".to_string());
    }

    let solved = solve_with_orientations(&endpoints, width, height)
        .ok_or_else(|| "flowfree has no reachable endpoint solution".to_string())?;

    let mut colors = endpoints
        .iter()
        .map(|endpoint| endpoint.color)
        .collect::<Vec<_>>();
    colors.sort_unstable();

    let mut steps = Vec::new();
    if needs_reset(session, &endpoints, width, height) {
        for color in &colors {
            steps.push(FlowfreeStep {
                action: "reset".to_string(),
                color: *color,
                r: 0,
                c: 0,
            });
        }
    }

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

fn normalized_endpoints(
    endpoints: &[FlowfreeEndpoint],
    width: usize,
    height: usize,
) -> Result<Vec<EndpointPlan>, String> {
    let mut plans = Vec::new();
    let mut colors = HashSet::new();
    for FlowfreeEndpoint(color, start, end) in endpoints {
        if *color <= 0 || !colors.insert(*color) {
            return Err("flowfree endpoints contain invalid colors".to_string());
        }
        point_index(*start, width, height)?;
        point_index(*end, width, height)?;
        if start == end {
            return Err("flowfree endpoint pair uses the same cell".to_string());
        }
        plans.push(EndpointPlan {
            color: *color,
            start: *start,
            end: *end,
        });
    }
    plans.sort_by_key(|endpoint| endpoint.color);
    Ok(plans)
}

fn solve_with_orientations(
    endpoints: &[EndpointPlan],
    width: usize,
    height: usize,
) -> Option<State> {
    let combinations = 1usize << endpoints.len();
    for mask in 0..combinations {
        let oriented = endpoints
            .iter()
            .enumerate()
            .map(|(index, endpoint)| {
                if (mask & (1usize << index)) == 0 {
                    endpoint.clone()
                } else {
                    EndpointPlan {
                        color: endpoint.color,
                        start: endpoint.end,
                        end: endpoint.start,
                    }
                }
            })
            .collect::<Vec<_>>();
        let mut state = initial_state(&oriented, width, height)?;
        if let Some(result) = search(&oriented, &mut state, width, height) {
            return Some(result);
        }
    }
    None
}

fn initial_state(endpoints: &[EndpointPlan], width: usize, height: usize) -> Option<State> {
    let mut state = State {
        grid: vec![vec![0; width]; height],
        paths: HashMap::new(),
        complete: HashSet::new(),
        calls: 0,
    };
    for endpoint in endpoints {
        let start = point_index(endpoint.start, width, height).ok()?;
        let end = point_index(endpoint.end, width, height).ok()?;
        if state.grid[endpoint.start[0] as usize][endpoint.start[1] as usize] != 0
            || state.grid[endpoint.end[0] as usize][endpoint.end[1] as usize] != 0
        {
            return None;
        }
        state.grid[start / width][start % width] = endpoint.color;
        state.grid[end / width][end % width] = endpoint.color;
        state.paths.insert(endpoint.color, vec![endpoint.start]);
    }
    Some(state)
}

fn search(
    endpoints: &[EndpointPlan],
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
        .all(|endpoint| state.complete.contains(&endpoint.color))
    {
        return Some(state.clone());
    }
    for endpoint in endpoints {
        if !state.complete.contains(&endpoint.color) && !reachable(endpoint, state, width, height) {
            return None;
        }
    }

    let (endpoint, moves) = choose_endpoint(endpoints, state, width, height)?;
    for (next, is_goal) in moves {
        let path = state.paths.get_mut(&endpoint.color)?;
        path.push(next);
        let previous = state.grid[next[0] as usize][next[1] as usize];
        if is_goal {
            state.complete.insert(endpoint.color);
        } else {
            state.grid[next[0] as usize][next[1] as usize] = endpoint.color;
        }

        if let Some(result) = search(endpoints, state, width, height) {
            return Some(result);
        }

        if is_goal {
            state.complete.remove(&endpoint.color);
        } else {
            state.grid[next[0] as usize][next[1] as usize] = previous;
        }
        state.paths.get_mut(&endpoint.color)?.pop();
    }
    None
}

fn choose_endpoint(
    endpoints: &[EndpointPlan],
    state: &State,
    width: usize,
    height: usize,
) -> Option<(EndpointPlan, Vec<(FlowfreePoint, bool)>)> {
    let mut best: Option<(EndpointPlan, Vec<(FlowfreePoint, bool)>)> = None;
    for endpoint in endpoints {
        if state.complete.contains(&endpoint.color) {
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
    best
}

fn legal_moves(
    endpoint: &EndpointPlan,
    state: &State,
    width: usize,
    height: usize,
) -> Vec<(FlowfreePoint, bool)> {
    let Some(path) = state.paths.get(&endpoint.color) else {
        return Vec::new();
    };
    let Some(&tip) = path.last() else {
        return Vec::new();
    };
    let mut moves = Vec::new();
    for (dr, dc) in DIRS {
        let next = [tip[0] + dr, tip[1] + dc];
        if point_index(next, width, height).is_err() {
            continue;
        }
        let is_goal = next == endpoint.end;
        if is_goal || state.grid[next[0] as usize][next[1] as usize] == 0 {
            moves.push((next, is_goal));
        }
    }
    moves.sort_by_key(|(point, is_goal)| {
        (
            manhattan(*point, endpoint.end),
            if *is_goal { 0 } else { 1 },
            point[0],
            point[1],
        )
    });
    moves
}

fn reachable(endpoint: &EndpointPlan, state: &State, width: usize, height: usize) -> bool {
    let Some(path) = state.paths.get(&endpoint.color) else {
        return false;
    };
    let Some(&start) = path.last() else {
        return false;
    };
    let mut seen = HashSet::new();
    let mut queue = VecDeque::from([start]);
    seen.insert(start);
    while let Some(point) = queue.pop_front() {
        if point == endpoint.end {
            return true;
        }
        for (dr, dc) in DIRS {
            let next = [point[0] + dr, point[1] + dc];
            if point_index(next, width, height).is_err() || !seen.insert(next) {
                continue;
            }
            if next == endpoint.end || state.grid[next[0] as usize][next[1] as usize] == 0 {
                queue.push_back(next);
            }
        }
    }
    false
}

fn needs_reset(
    session: &FlowfreeSession,
    endpoints: &[EndpointPlan],
    width: usize,
    height: usize,
) -> bool {
    if session.paths.iter().any(|path| !path.1.is_empty()) {
        return true;
    }
    let endpoint_cells = endpoints
        .iter()
        .flat_map(|endpoint| {
            [
                (endpoint.start, endpoint.color),
                (endpoint.end, endpoint.color),
            ]
        })
        .map(|(point, color)| (point[0], point[1], color))
        .collect::<HashSet<_>>();
    for r in 0..height {
        for c in 0..width {
            let value = session
                .cells
                .get(r)
                .and_then(|row| row.get(c))
                .copied()
                .unwrap_or(0);
            if value == 0 {
                continue;
            }
            if !endpoint_cells.contains(&(r as i32, c as i32, value)) {
                return true;
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
                FlowfreeEndpoint(1, [0, 0], [0, 4]),
                FlowfreeEndpoint(2, [4, 0], [4, 4]),
                FlowfreeEndpoint(3, [2, 1], [2, 3]),
                FlowfreeEndpoint(4, [1, 0], [3, 0]),
            ],
            cells: vec![
                vec![1, 0, 0, 0, 1],
                vec![4, 0, 0, 0, 0],
                vec![0, 3, 0, 3, 0],
                vec![4, 0, 0, 0, 0],
                vec![2, 0, 0, 0, 2],
            ],
            ..FlowfreeSession::default()
        };

        let steps = solve(&session).unwrap();

        assert!(!steps.is_empty());
        assert!(steps.iter().any(|step| step.action == "paint"));
        assert!(!steps.iter().any(|step| step.action == "reset"));
    }

    #[test]
    fn solver_resets_partial_paths_before_replay() {
        let session = FlowfreeSession {
            width: 3,
            height: 3,
            endpoints: vec![FlowfreeEndpoint(1, [0, 0], [0, 2])],
            cells: vec![vec![1, 1, 1], vec![0, 0, 0], vec![0, 0, 0]],
            paths: vec![crate::model::FlowfreePath(1, vec![[0, 0], [0, 1]])],
            ..FlowfreeSession::default()
        };

        let steps = solve(&session).unwrap();

        assert_eq!(
            steps.first().map(|step| step.action.as_str()),
            Some("reset")
        );
        assert!(steps.iter().any(|step| step.action == "paint"));
    }

    #[test]
    fn solver_rejects_crossed_corner_pairs() {
        let session = FlowfreeSession {
            width: 5,
            height: 5,
            endpoints: vec![
                FlowfreeEndpoint(1, [0, 0], [4, 4]),
                FlowfreeEndpoint(2, [0, 4], [4, 0]),
                FlowfreeEndpoint(3, [2, 2], [1, 1]),
            ],
            cells: vec![vec![0; 5]; 5],
            ..FlowfreeSession::default()
        };

        assert!(solve(&session).is_err());
    }
}
