use std::collections::{HashMap, HashSet, VecDeque};

use crate::model::{FlowfreeEndpoint, FlowfreePoint, FlowfreeSession};

const DIRS: [(i32, i32); 4] = [(-1, 0), (1, 0), (0, -1), (0, 1)];
const FAST_SEARCH_LIMIT: usize = 100_000;
const FULL_SEARCH_LIMIT: usize = 20_000_000;
const PROVEN_UNSOLVABLE_ERROR: &str = "flowfree has no reachable endpoint solution";
const SEARCH_LIMIT_ERROR: &str = "flowfree solver search limit exceeded";

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
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct SearchKey {
    cells: Vec<i32>,
    tips: Vec<(i32, i32, i32, bool)>,
}

enum SearchOutcome {
    Solved(State),
    Exhausted,
    SearchLimitExceeded,
}

struct SearchBudget {
    calls: usize,
    limit: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct ReachabilityStats {
    reachable_cells: usize,
    distance_to_goal: usize,
}

pub fn solve(session: &FlowfreeSession) -> Result<Vec<FlowfreeStep>, String> {
    let height = usize_from_i32(session.height, "flowfree height")?;
    let width = usize_from_i32(session.width, "flowfree width")?;
    let endpoints = normalized_endpoints(&session.endpoints, width, height)?;
    if endpoints.is_empty() {
        return Err("flowfree board has no endpoints".to_string());
    }

    if has_alternating_boundary_endpoints(&endpoints, width, height) {
        return Err(PROVEN_UNSOLVABLE_ERROR.to_string());
    }

    let solved = match solve_with_budget(&endpoints, width, height, FAST_SEARCH_LIMIT) {
        SearchOutcome::Solved(state) => state,
        SearchOutcome::Exhausted | SearchOutcome::SearchLimitExceeded => {
            match solve_with_budget(&endpoints, width, height, FULL_SEARCH_LIMIT) {
                SearchOutcome::Solved(state) => state,
                SearchOutcome::Exhausted => return Err(PROVEN_UNSOLVABLE_ERROR.to_string()),
                SearchOutcome::SearchLimitExceeded => return Err(SEARCH_LIMIT_ERROR.to_string()),
            }
        }
    };

    steps_from_solution(session, &endpoints, width, height, solved)
}

pub fn is_proven_unsolvable_error(message: &str) -> bool {
    message.trim() == PROVEN_UNSOLVABLE_ERROR
}

fn steps_from_solution(
    session: &FlowfreeSession,
    endpoints: &[EndpointPlan],
    width: usize,
    height: usize,
    solved: State,
) -> Result<Vec<FlowfreeStep>, String> {
    validate_solution(&solved, endpoints, width, height)?;

    let mut colors = endpoints
        .iter()
        .map(|endpoint| endpoint.color)
        .collect::<Vec<_>>();
    colors.sort_unstable();

    let mut steps = Vec::new();
    if needs_reset(session, endpoints, width, height) {
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

fn validate_solution(
    solved: &State,
    endpoints: &[EndpointPlan],
    width: usize,
    height: usize,
) -> Result<(), String> {
    let mut occupied = HashMap::new();
    for endpoint in endpoints {
        let path = solved
            .paths
            .get(&endpoint.color)
            .ok_or_else(|| "flowfree solution is missing a color path".to_string())?;
        let path_endpoints_match = (path.first().copied() == Some(endpoint.start)
            && path.last().copied() == Some(endpoint.end))
            || (path.first().copied() == Some(endpoint.end)
                && path.last().copied() == Some(endpoint.start));
        if !path_endpoints_match {
            return Err("flowfree solution path does not connect its endpoints".to_string());
        }
        for point in path {
            point_index(*point, width, height)?;
            let cell = solved.grid[point[0] as usize][point[1] as usize];
            if cell != endpoint.color {
                return Err("flowfree solution path uses a cell owned by another color".to_string());
            }
            if occupied.insert(*point, endpoint.color).is_some() {
                return Err("flowfree solution path reuses a cell".to_string());
            }
        }
        for pair in path.windows(2) {
            if manhattan(pair[0], pair[1]) != 1 {
                return Err("flowfree solution path contains a non-adjacent step".to_string());
            }
        }
    }
    Ok(())
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

fn has_alternating_boundary_endpoints(
    endpoints: &[EndpointPlan],
    width: usize,
    height: usize,
) -> bool {
    for (left_index, left) in endpoints.iter().enumerate() {
        let Some((left_start, left_end)) = boundary_pair(left, width, height) else {
            continue;
        };
        for right in endpoints.iter().skip(left_index + 1) {
            let Some((right_start, right_end)) = boundary_pair(right, width, height) else {
                continue;
            };
            if boundary_pairs_alternate(left_start, left_end, right_start, right_end) {
                return true;
            }
        }
    }
    false
}

fn boundary_pair(endpoint: &EndpointPlan, width: usize, height: usize) -> Option<(usize, usize)> {
    Some((
        boundary_index(endpoint.start, width, height)?,
        boundary_index(endpoint.end, width, height)?,
    ))
}

fn boundary_pairs_alternate(
    left_start: usize,
    left_end: usize,
    right_start: usize,
    right_end: usize,
) -> bool {
    let (left_min, left_max) = if left_start <= left_end {
        (left_start, left_end)
    } else {
        (left_end, left_start)
    };
    let right_start_between = left_min < right_start && right_start < left_max;
    let right_end_between = left_min < right_end && right_end < left_max;
    right_start_between != right_end_between
}

fn boundary_index(point: FlowfreePoint, width: usize, height: usize) -> Option<usize> {
    if width < 2 || height < 2 {
        return None;
    }
    let r = usize::try_from(point[0]).ok()?;
    let c = usize::try_from(point[1]).ok()?;
    if r >= height || c >= width {
        return None;
    }
    if r == 0 {
        return Some(c);
    }
    if c == width - 1 {
        return Some((width - 1) + r);
    }
    if r == height - 1 {
        return Some((width - 1) + (height - 1) + (width - 1 - c));
    }
    if c == 0 {
        return Some((width - 1) + (height - 1) + (width - 1) + (height - 1 - r));
    }
    None
}

fn solve_with_budget(
    endpoints: &[EndpointPlan],
    width: usize,
    height: usize,
    search_limit: usize,
) -> SearchOutcome {
    let oriented = oriented_endpoints(endpoints, width, height);
    let mut budget = SearchBudget {
        calls: 0,
        limit: search_limit,
    };
    let Some(mut state) = initial_state(&oriented, width, height) else {
        return SearchOutcome::Exhausted;
    };
    if let Some(result) = greedy_solution(&oriented, state.clone(), width, height) {
        return SearchOutcome::Solved(result);
    }
    let mut exhausted = HashSet::new();
    match search(
        &oriented,
        &mut state,
        &mut budget,
        &mut exhausted,
        width,
        height,
    ) {
        SearchOutcome::Solved(result) => SearchOutcome::Solved(result),
        SearchOutcome::SearchLimitExceeded => SearchOutcome::SearchLimitExceeded,
        SearchOutcome::Exhausted => SearchOutcome::Exhausted,
    }
}

fn oriented_endpoints(
    endpoints: &[EndpointPlan],
    width: usize,
    height: usize,
) -> Vec<EndpointPlan> {
    let occupied = endpoints
        .iter()
        .flat_map(|endpoint| [endpoint.start, endpoint.end])
        .collect::<HashSet<_>>();
    endpoints
        .iter()
        .map(|endpoint| {
            let start_degree =
                endpoint_open_degree(endpoint.start, endpoint.end, &occupied, width, height);
            let end_degree =
                endpoint_open_degree(endpoint.end, endpoint.start, &occupied, width, height);
            let should_flip = (end_degree, endpoint.end[0], endpoint.end[1])
                < (start_degree, endpoint.start[0], endpoint.start[1]);
            if should_flip {
                EndpointPlan {
                    color: endpoint.color,
                    start: endpoint.end,
                    end: endpoint.start,
                }
            } else {
                endpoint.clone()
            }
        })
        .collect()
}

fn endpoint_open_degree(
    point: FlowfreePoint,
    mate: FlowfreePoint,
    occupied: &HashSet<FlowfreePoint>,
    width: usize,
    height: usize,
) -> usize {
    DIRS.iter()
        .filter(|(dr, dc)| {
            let next = [point[0] + dr, point[1] + dc];
            point_index(next, width, height).is_ok() && (next == mate || !occupied.contains(&next))
        })
        .count()
}

fn initial_state(endpoints: &[EndpointPlan], width: usize, height: usize) -> Option<State> {
    let mut state = State {
        grid: vec![vec![0; width]; height],
        paths: HashMap::new(),
        complete: HashSet::new(),
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

fn greedy_solution(
    endpoints: &[EndpointPlan],
    mut state: State,
    width: usize,
    height: usize,
) -> Option<State> {
    while !endpoints
        .iter()
        .all(|endpoint| state.complete.contains(&endpoint.color))
    {
        let mut best: Option<(usize, usize, usize, EndpointPlan, Vec<FlowfreePoint>)> = None;
        for endpoint in endpoints {
            if state.complete.contains(&endpoint.color) {
                continue;
            }
            let path = shortest_path(endpoint, &state, width, height)?;
            let legal_count = legal_moves(endpoint, &state, width, height).len();
            let stats = reachability_stats(endpoint, &state, width, height)?;
            let score = (
                legal_count,
                stats.reachable_cells,
                path.len(),
                endpoint.clone(),
                path,
            );
            if best.as_ref().is_none_or(|current| {
                (score.0, score.1, score.2, score.3.color)
                    < (current.0, current.1, current.2, current.3.color)
            }) {
                best = Some(score);
            }
        }
        let Some((_, _, _, endpoint, path)) = best else {
            return None;
        };
        apply_path(&endpoint, &mut state, &path)?;
    }
    Some(state)
}

fn apply_path(endpoint: &EndpointPlan, state: &mut State, path: &[FlowfreePoint]) -> Option<()> {
    if path.len() < 2 {
        return None;
    }
    let current_tip = state.paths.get(&endpoint.color)?.last().copied()?;
    if path.first().copied()? != current_tip || path.last().copied()? != endpoint.end {
        return None;
    }
    let mut previous = current_tip;
    for point in path.iter().skip(1) {
        if manhattan(previous, *point) != 1 {
            return None;
        }
        let is_goal = *point == endpoint.end;
        if is_goal {
            if state.grid[point[0] as usize][point[1] as usize] != endpoint.color {
                return None;
            }
            state.complete.insert(endpoint.color);
        } else {
            let cell = &mut state.grid[point[0] as usize][point[1] as usize];
            if *cell != 0 {
                return None;
            }
            *cell = endpoint.color;
        }
        state.paths.get_mut(&endpoint.color)?.push(*point);
        previous = *point;
    }
    Some(())
}

fn search(
    endpoints: &[EndpointPlan],
    state: &mut State,
    budget: &mut SearchBudget,
    exhausted: &mut HashSet<SearchKey>,
    width: usize,
    height: usize,
) -> SearchOutcome {
    budget.calls += 1;
    if budget.calls > budget.limit {
        return SearchOutcome::SearchLimitExceeded;
    }
    if endpoints
        .iter()
        .all(|endpoint| state.complete.contains(&endpoint.color))
    {
        return SearchOutcome::Solved(state.clone());
    }

    let key = search_key(endpoints, state, width, height);
    if exhausted.contains(&key) {
        return SearchOutcome::Exhausted;
    }

    let Some((endpoint, moves)) = choose_endpoint(endpoints, state, width, height) else {
        exhausted.insert(key);
        return SearchOutcome::Exhausted;
    };
    for (next, is_goal) in moves {
        let Some(path) = state.paths.get_mut(&endpoint.color) else {
            exhausted.insert(key);
            return SearchOutcome::Exhausted;
        };
        path.push(next);
        let previous = state.grid[next[0] as usize][next[1] as usize];
        if is_goal {
            state.complete.insert(endpoint.color);
        } else {
            state.grid[next[0] as usize][next[1] as usize] = endpoint.color;
        }

        match search(endpoints, state, budget, exhausted, width, height) {
            SearchOutcome::Solved(result) => return SearchOutcome::Solved(result),
            SearchOutcome::SearchLimitExceeded => return SearchOutcome::SearchLimitExceeded,
            SearchOutcome::Exhausted => {}
        }

        if is_goal {
            state.complete.remove(&endpoint.color);
        } else {
            state.grid[next[0] as usize][next[1] as usize] = previous;
        }
        let Some(path) = state.paths.get_mut(&endpoint.color) else {
            return SearchOutcome::Exhausted;
        };
        path.pop();
    }
    exhausted.insert(key);
    SearchOutcome::Exhausted
}

fn search_key(endpoints: &[EndpointPlan], state: &State, width: usize, height: usize) -> SearchKey {
    let mut cells = Vec::with_capacity(width * height);
    for row in &state.grid {
        cells.extend(row.iter().copied());
    }
    let mut tips = Vec::with_capacity(endpoints.len());
    for endpoint in endpoints {
        let tip = state
            .paths
            .get(&endpoint.color)
            .and_then(|path| path.last())
            .copied()
            .unwrap_or(endpoint.start);
        tips.push((
            endpoint.color,
            tip[0],
            tip[1],
            state.complete.contains(&endpoint.color),
        ));
    }
    SearchKey { cells, tips }
}

fn choose_endpoint(
    endpoints: &[EndpointPlan],
    state: &State,
    width: usize,
    height: usize,
) -> Option<(EndpointPlan, Vec<(FlowfreePoint, bool)>)> {
    let mut best: Option<(EndpointPlan, Vec<(FlowfreePoint, bool)>, ReachabilityStats)> = None;
    for endpoint in endpoints {
        if state.complete.contains(&endpoint.color) {
            continue;
        }
        let moves = legal_moves(endpoint, state, width, height);
        if moves.is_empty() {
            return None;
        }
        let stats = reachability_stats(endpoint, state, width, height)?;
        if best
            .as_ref()
            .is_none_or(|(best_endpoint, best_moves, best_stats)| {
                (
                    moves.len(),
                    stats.reachable_cells,
                    stats.distance_to_goal,
                    endpoint.color,
                ) < (
                    best_moves.len(),
                    best_stats.reachable_cells,
                    best_stats.distance_to_goal,
                    best_endpoint.color,
                )
            })
        {
            best = Some((endpoint.clone(), moves, stats));
        }
    }
    best.map(|(endpoint, moves, _)| (endpoint, moves))
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
        if can_enter_cell(endpoint, state, next) {
            moves.push((next, next == endpoint.end));
        }
    }
    moves.sort_by_key(|(point, is_goal)| {
        (
            shortest_distance_from(*point, endpoint.color, endpoint.end, state, width, height)
                .unwrap_or(usize::MAX),
            open_neighbor_count(*point, endpoint, state, width, height),
            if *is_goal { 0 } else { 1 },
            manhattan(*point, endpoint.end),
            point[0],
            point[1],
        )
    });
    moves
}

fn reachability_stats(
    endpoint: &EndpointPlan,
    state: &State,
    width: usize,
    height: usize,
) -> Option<ReachabilityStats> {
    let Some(path) = state.paths.get(&endpoint.color) else {
        return None;
    };
    let Some(&start) = path.last() else {
        return None;
    };
    let mut seen = HashSet::new();
    let mut queue = VecDeque::from([(start, 0usize)]);
    seen.insert(start);
    while let Some((point, distance)) = queue.pop_front() {
        if point == endpoint.end {
            return Some(ReachabilityStats {
                reachable_cells: seen.len(),
                distance_to_goal: distance,
            });
        }
        for (dr, dc) in DIRS {
            let next = [point[0] + dr, point[1] + dc];
            if point_index(next, width, height).is_err() || !seen.insert(next) {
                continue;
            }
            if can_enter_cell(endpoint, state, next) {
                queue.push_back((next, distance + 1));
            }
        }
    }
    None
}

fn shortest_distance_from(
    start: FlowfreePoint,
    color: i32,
    goal: FlowfreePoint,
    state: &State,
    width: usize,
    height: usize,
) -> Option<usize> {
    let mut seen = HashSet::new();
    let mut queue = VecDeque::from([(start, 0usize)]);
    seen.insert(start);
    while let Some((point, distance)) = queue.pop_front() {
        if point == goal {
            return Some(distance);
        }
        for (dr, dc) in DIRS {
            let next = [point[0] + dr, point[1] + dc];
            if point_index(next, width, height).is_err() || !seen.insert(next) {
                continue;
            }
            if can_enter_goal_or_empty(color, goal, state, next) {
                queue.push_back((next, distance + 1));
            }
        }
    }
    None
}

fn shortest_path(
    endpoint: &EndpointPlan,
    state: &State,
    width: usize,
    height: usize,
) -> Option<Vec<FlowfreePoint>> {
    let start = state.paths.get(&endpoint.color)?.last().copied()?;
    let mut seen = HashSet::new();
    let mut parents: HashMap<FlowfreePoint, FlowfreePoint> = HashMap::new();
    let mut queue = VecDeque::from([start]);
    seen.insert(start);
    while let Some(point) = queue.pop_front() {
        if point == endpoint.end {
            let mut result = vec![point];
            let mut cursor = point;
            while cursor != start {
                cursor = *parents.get(&cursor)?;
                result.push(cursor);
            }
            result.reverse();
            return Some(result);
        }
        for (dr, dc) in DIRS {
            let next = [point[0] + dr, point[1] + dc];
            if point_index(next, width, height).is_err() || !seen.insert(next) {
                continue;
            }
            if can_enter_cell(endpoint, state, next) {
                parents.insert(next, point);
                queue.push_back(next);
            }
        }
    }
    None
}

fn open_neighbor_count(
    point: FlowfreePoint,
    endpoint: &EndpointPlan,
    state: &State,
    width: usize,
    height: usize,
) -> usize {
    DIRS.iter()
        .filter(|(dr, dc)| {
            let next = [point[0] + dr, point[1] + dc];
            point_index(next, width, height).is_ok() && can_enter_cell(endpoint, state, next)
        })
        .count()
}

fn can_enter_cell(endpoint: &EndpointPlan, state: &State, point: FlowfreePoint) -> bool {
    can_enter_goal_or_empty(endpoint.color, endpoint.end, state, point)
}

fn can_enter_goal_or_empty(
    color: i32,
    goal: FlowfreePoint,
    state: &State,
    point: FlowfreePoint,
) -> bool {
    let cell = state.grid[point[0] as usize][point[1] as usize];
    if point == goal {
        return cell == color;
    }
    cell == 0
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
    use std::collections::{HashMap, HashSet};

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
    fn solver_outputs_adjacent_non_overlapping_same_color_paths() {
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
        assert_steps_connect_endpoints(&session, &steps);
    }

    #[test]
    fn solver_connects_9x9_six_color_board() {
        let session = FlowfreeSession {
            width: 9,
            height: 9,
            endpoints: vec![
                FlowfreeEndpoint(1, [0, 0], [0, 8]),
                FlowfreeEndpoint(2, [1, 0], [1, 8]),
                FlowfreeEndpoint(3, [2, 0], [2, 8]),
                FlowfreeEndpoint(4, [3, 0], [3, 8]),
                FlowfreeEndpoint(5, [4, 0], [4, 8]),
                FlowfreeEndpoint(6, [5, 0], [5, 8]),
            ],
            cells: vec![
                vec![1, 0, 0, 0, 0, 0, 0, 0, 1],
                vec![2, 0, 0, 0, 0, 0, 0, 0, 2],
                vec![3, 0, 0, 0, 0, 0, 0, 0, 3],
                vec![4, 0, 0, 0, 0, 0, 0, 0, 4],
                vec![5, 0, 0, 0, 0, 0, 0, 0, 5],
                vec![6, 0, 0, 0, 0, 0, 0, 0, 6],
                vec![0, 0, 0, 0, 0, 0, 0, 0, 0],
                vec![0, 0, 0, 0, 0, 0, 0, 0, 0],
                vec![0, 0, 0, 0, 0, 0, 0, 0, 0],
            ],
            ..FlowfreeSession::default()
        };

        let steps = solve(&session).unwrap();
        let paint_steps = steps
            .iter()
            .filter(|step| step.action == "paint")
            .collect::<Vec<_>>();

        assert_eq!(paint_steps.len(), 54);
        assert_eq!(
            paint_steps
                .iter()
                .map(|step| step.color)
                .collect::<HashSet<_>>()
                .len(),
            6
        );
        for color in 1..=6 {
            let path = paint_steps
                .iter()
                .filter(|step| step.color == color)
                .map(|step| [step.r, step.c])
                .collect::<Vec<_>>();
            assert_eq!(path.first().copied(), Some([color - 1, 0]));
            assert_eq!(path.last().copied(), Some([color - 1, 8]));
            for pair in path.windows(2) {
                assert_eq!(manhattan(pair[0], pair[1]), 1);
            }
        }
        assert_steps_connect_endpoints(&session, &steps);
    }

    #[test]
    fn solver_success_rate_9x9_six_color_generated_100_boards() {
        const TOTAL: usize = 100;
        let mut successes = 0usize;
        let mut failures = Vec::new();
        for index in 0..TOTAL {
            let session = generated_9x9_six_color_session(index as u64);
            match solve(&session) {
                Ok(steps) => {
                    assert_steps_connect_endpoints(&session, &steps);
                    successes += 1;
                }
                Err(error) => failures.push((index, error)),
            }
        }

        println!(
            "flowfree 9x9 six-color generated success rate: {successes}/{TOTAL} ({:.1}%)",
            successes as f64 * 100.0 / TOTAL as f64
        );
        assert!(failures.is_empty(), "failed generated boards: {failures:?}");
    }

    #[test]
    fn legal_moves_enter_only_same_color_goal() {
        let endpoint = EndpointPlan {
            color: 1,
            start: [0, 0],
            end: [0, 2],
        };
        let wrong_goal_state = State {
            grid: vec![vec![1, 1, 2]],
            paths: HashMap::from([(1, vec![[0, 0], [0, 1]])]),
            complete: HashSet::new(),
        };
        assert!(legal_moves(&endpoint, &wrong_goal_state, 3, 1).is_empty());

        let own_goal_state = State {
            grid: vec![vec![1, 1, 1]],
            paths: HashMap::from([(1, vec![[0, 0], [0, 1]])]),
            complete: HashSet::new(),
        };
        assert_eq!(
            legal_moves(&endpoint, &own_goal_state, 3, 1),
            vec![([0, 2], true)]
        );
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
    fn solver_keeps_current_endpoint_connection_semantics() {
        let session = FlowfreeSession {
            width: 3,
            height: 3,
            endpoints: vec![FlowfreeEndpoint(1, [0, 0], [0, 2])],
            cells: vec![vec![1, 0, 1], vec![0, 0, 0], vec![0, 0, 0]],
            ..FlowfreeSession::default()
        };

        let steps = solve(&session).unwrap();

        assert_eq!(
            steps
                .iter()
                .filter(|step| step.action == "paint")
                .collect::<Vec<_>>()
                .len(),
            3
        );
    }

    #[test]
    fn solver_rejects_crossed_normal_level() {
        let session = FlowfreeSession {
            width: 7,
            height: 7,
            endpoints: vec![
                FlowfreeEndpoint(1, [0, 0], [6, 6]),
                FlowfreeEndpoint(2, [0, 6], [6, 0]),
                FlowfreeEndpoint(3, [2, 2], [4, 4]),
                FlowfreeEndpoint(4, [2, 4], [4, 2]),
            ],
            cells: vec![
                vec![1, 0, 0, 0, 0, 0, 2],
                vec![0, 0, 0, 0, 0, 0, 0],
                vec![0, 0, 3, 0, 4, 0, 0],
                vec![0, 0, 0, 0, 0, 0, 0],
                vec![0, 0, 4, 0, 3, 0, 0],
                vec![0, 0, 0, 0, 0, 0, 0],
                vec![2, 0, 0, 0, 0, 0, 1],
            ],
            ..FlowfreeSession::default()
        };

        let error = solve(&session).unwrap_err();

        assert!(is_proven_unsolvable_error(&error));
        assert!(!is_proven_unsolvable_error(SEARCH_LIMIT_ERROR));
    }

    #[test]
    fn solver_rejects_crossed_hard_boundary_pairs() {
        let session = FlowfreeSession {
            width: 9,
            height: 9,
            endpoints: vec![
                FlowfreeEndpoint(1, [0, 0], [0, 8]),
                FlowfreeEndpoint(2, [8, 0], [8, 8]),
                FlowfreeEndpoint(3, [4, 0], [4, 8]),
                FlowfreeEndpoint(4, [0, 4], [8, 4]),
                FlowfreeEndpoint(5, [2, 2], [6, 6]),
                FlowfreeEndpoint(6, [2, 6], [6, 2]),
            ],
            cells: vec![vec![0; 9]; 9],
            ..FlowfreeSession::default()
        };

        let error = solve(&session).unwrap_err();

        assert!(is_proven_unsolvable_error(&error));
    }

    fn assert_steps_connect_endpoints(session: &FlowfreeSession, steps: &[FlowfreeStep]) {
        let endpoints = session
            .endpoints
            .iter()
            .map(|FlowfreeEndpoint(color, start, end)| (*color, (*start, *end)))
            .collect::<HashMap<_, _>>();
        let mut paths: HashMap<i32, Vec<FlowfreePoint>> = HashMap::new();
        for step in steps.iter().filter(|step| step.action == "paint") {
            paths.entry(step.color).or_default().push([step.r, step.c]);
        }

        let mut occupied = HashSet::new();
        for (color, (start, end)) in endpoints {
            let path = paths.get(&color).unwrap();
            let endpoints_match = (path.first().copied() == Some(start)
                && path.last().copied() == Some(end))
                || (path.first().copied() == Some(end) && path.last().copied() == Some(start));
            assert!(endpoints_match);
            for pair in path.windows(2) {
                assert_eq!(manhattan(pair[0], pair[1]), 1);
            }
            for point in path {
                assert!(occupied.insert(*point));
            }
        }
    }

    fn generated_9x9_six_color_session(seed: u64) -> FlowfreeSession {
        let mut rng = seed.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let horizontal = next_seed(&mut rng).is_multiple_of(2);
        let mut lanes = (0..9).collect::<Vec<i32>>();
        shuffle(&mut lanes, &mut rng);
        lanes.truncate(6);
        let mut colors = (1..=6).collect::<Vec<i32>>();
        shuffle(&mut colors, &mut rng);

        let mut cells = vec![vec![0; 9]; 9];
        let mut endpoints = Vec::new();
        for (index, color) in colors.into_iter().enumerate() {
            let lane = lanes[index];
            let (start, end) = if horizontal {
                ([lane, 0], [lane, 8])
            } else {
                ([0, lane], [8, lane])
            };
            cells[start[0] as usize][start[1] as usize] = color;
            cells[end[0] as usize][end[1] as usize] = color;
            endpoints.push(FlowfreeEndpoint(color, start, end));
        }

        FlowfreeSession {
            width: 9,
            height: 9,
            endpoints,
            cells,
            ..FlowfreeSession::default()
        }
    }

    fn shuffle(values: &mut [i32], seed: &mut u64) {
        for index in (1..values.len()).rev() {
            let swap_index = (next_seed(seed) as usize) % (index + 1);
            values.swap(index, swap_index);
        }
    }

    fn next_seed(seed: &mut u64) -> u64 {
        *seed = seed
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        *seed
    }
}
