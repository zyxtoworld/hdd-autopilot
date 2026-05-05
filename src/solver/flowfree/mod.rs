use std::collections::{HashMap, HashSet, VecDeque};

use crate::model::{FlowfreeEndpoint, FlowfreePoint, FlowfreeSession};

const DIRS: [(i32, i32); 4] = [(-1, 0), (1, 0), (0, -1), (0, 1)];
const FAST_SEARCH_LIMIT: usize = 500_000;
const FULL_SEARCH_LIMIT: usize = 50_000_000;
const PROVEN_UNSOLVABLE_ERROR: &str = "flowfree has no reachable endpoint solution";
const SEARCH_LIMIT_ERROR: &str = "flowfree solver search limit exceeded";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlowfreeStep {
    pub action: String,
    pub color: i32,
    pub r: i32,
    pub c: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Board {
    width: usize,
    height: usize,
    cells: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct EndpointPlan {
    color: i32,
    start: usize,
    end: usize,
}

#[derive(Clone)]
struct State {
    grid: Vec<i32>,
    paths: HashMap<i32, Vec<usize>>,
    complete: HashSet<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct SearchKey {
    grid: Vec<i32>,
    tips: Vec<(i32, usize, bool)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct ReachabilityStats {
    reachable_cells: usize,
    distance_to_goal: usize,
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

type EndpointSearchCandidate = (EndpointPlan, Vec<(usize, bool)>, ReachabilityStats);

pub fn solve(session: &FlowfreeSession) -> Result<Vec<FlowfreeStep>, String> {
    let board = Board::new(session.width, session.height)?;
    let endpoints = normalized_endpoints(&session.endpoints, &board)?;
    if endpoints.is_empty() {
        return Err("flowfree board has no endpoints".to_string());
    }
    if has_forced_outer_boundary_crossing(&endpoints, &board) {
        return Err(PROVEN_UNSOLVABLE_ERROR.to_string());
    }

    let solved = match solve_with_budget(&endpoints, &board, FAST_SEARCH_LIMIT) {
        SearchOutcome::Solved(state) => state,
        SearchOutcome::Exhausted | SearchOutcome::SearchLimitExceeded => {
            match solve_with_budget(&endpoints, &board, FULL_SEARCH_LIMIT) {
                SearchOutcome::Solved(state) => state,
                SearchOutcome::Exhausted => return Err(PROVEN_UNSOLVABLE_ERROR.to_string()),
                SearchOutcome::SearchLimitExceeded => return Err(SEARCH_LIMIT_ERROR.to_string()),
            }
        }
    };

    steps_from_solution(session, &endpoints, &board, solved)
}

pub fn is_proven_unsolvable_error(message: &str) -> bool {
    message.trim() == PROVEN_UNSOLVABLE_ERROR
}

impl Board {
    fn new(width: i32, height: i32) -> Result<Self, String> {
        let width = usize_from_i32(width, "flowfree width")?;
        let height = usize_from_i32(height, "flowfree height")?;
        let cells = width
            .checked_mul(height)
            .ok_or_else(|| "flowfree board is too large".to_string())?;
        Ok(Self {
            width,
            height,
            cells,
        })
    }

    fn index(&self, point: FlowfreePoint) -> Result<usize, String> {
        let r = usize::try_from(point[0]).map_err(|_| "invalid coordinate".to_string())?;
        let c = usize::try_from(point[1]).map_err(|_| "invalid coordinate".to_string())?;
        if r >= self.height || c >= self.width {
            return Err("coordinate is outside the board".to_string());
        }
        Ok(r * self.width + c)
    }

    fn point(&self, index: usize) -> FlowfreePoint {
        [(index / self.width) as i32, (index % self.width) as i32]
    }

    fn neighbors(&self, index: usize) -> Vec<usize> {
        let point = self.point(index);
        DIRS.iter()
            .filter_map(|(dr, dc)| {
                let next = [point[0] + dr, point[1] + dc];
                self.index(next).ok()
            })
            .collect()
    }
}

fn normalized_endpoints(
    endpoints: &[FlowfreeEndpoint],
    board: &Board,
) -> Result<Vec<EndpointPlan>, String> {
    let mut plans = Vec::new();
    let mut colors = HashSet::new();
    let mut endpoint_cells = HashSet::new();
    for FlowfreeEndpoint(color, start, end) in endpoints {
        if *color <= 0 || !colors.insert(*color) {
            return Err("flowfree endpoints contain invalid colors".to_string());
        }
        let start = board.index(*start)?;
        let end = board.index(*end)?;
        if start == end {
            return Err("flowfree endpoint pair uses the same cell".to_string());
        }
        if !endpoint_cells.insert(start) || !endpoint_cells.insert(end) {
            return Err("flowfree endpoint cells overlap".to_string());
        }
        plans.push(EndpointPlan {
            color: *color,
            start,
            end,
        });
    }
    plans.sort_by_key(|endpoint| endpoint.color);
    Ok(oriented_endpoints(&plans, board))
}

fn oriented_endpoints(endpoints: &[EndpointPlan], board: &Board) -> Vec<EndpointPlan> {
    let occupied = endpoints
        .iter()
        .flat_map(|endpoint| [endpoint.start, endpoint.end])
        .collect::<HashSet<_>>();
    endpoints
        .iter()
        .map(|endpoint| {
            let start_degree = endpoint_open_degree(endpoint.start, endpoint.end, &occupied, board);
            let end_degree = endpoint_open_degree(endpoint.end, endpoint.start, &occupied, board);
            if end_degree < start_degree {
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
    point: usize,
    mate: usize,
    occupied: &HashSet<usize>,
    board: &Board,
) -> usize {
    board
        .neighbors(point)
        .into_iter()
        .filter(|next| *next == mate || !occupied.contains(next))
        .count()
}

fn solve_with_budget(
    endpoints: &[EndpointPlan],
    board: &Board,
    search_limit: usize,
) -> SearchOutcome {
    let mut budget = SearchBudget {
        calls: 0,
        limit: search_limit,
    };
    let Some(mut state) = initial_state(endpoints, board) else {
        return SearchOutcome::Exhausted;
    };
    if let Some(result) = greedy_solution(endpoints, state.clone(), board) {
        return SearchOutcome::Solved(result);
    }

    let mut exhausted = HashSet::new();
    search(endpoints, &mut state, &mut budget, &mut exhausted, board)
}

fn initial_state(endpoints: &[EndpointPlan], board: &Board) -> Option<State> {
    let mut state = State {
        grid: vec![0; board.cells],
        paths: HashMap::new(),
        complete: HashSet::new(),
    };
    for endpoint in endpoints {
        if state.grid[endpoint.start] != 0 || state.grid[endpoint.end] != 0 {
            return None;
        }
        state.grid[endpoint.start] = endpoint.color;
        state.grid[endpoint.end] = endpoint.color;
        state.paths.insert(endpoint.color, vec![endpoint.start]);
    }
    Some(state)
}

fn greedy_solution(endpoints: &[EndpointPlan], mut state: State, board: &Board) -> Option<State> {
    while !endpoints
        .iter()
        .all(|endpoint| state.complete.contains(&endpoint.color))
    {
        let mut best: Option<(usize, usize, usize, EndpointPlan, Vec<usize>)> = None;
        for endpoint in endpoints {
            if state.complete.contains(&endpoint.color) {
                continue;
            }
            let path = shortest_path(endpoint, &state, board)?;
            let legal_count = legal_moves(endpoint, &state, board).len();
            let stats = reachability_stats(endpoint, &state, board)?;
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
        let (_, _, _, endpoint, path) = best?;
        apply_path(&endpoint, &mut state, &path, board)?;
    }
    Some(state)
}

fn apply_path(
    endpoint: &EndpointPlan,
    state: &mut State,
    path: &[usize],
    board: &Board,
) -> Option<()> {
    if path.len() < 2 {
        return None;
    }
    let current_tip = state.paths.get(&endpoint.color)?.last().copied()?;
    if path.first().copied()? != current_tip || path.last().copied()? != endpoint.end {
        return None;
    }
    let mut previous = current_tip;
    for index in path.iter().skip(1).copied() {
        if manhattan(board.point(previous), board.point(index)) != 1 {
            return None;
        }
        let is_goal = index == endpoint.end;
        if is_goal {
            if state.grid[index] != endpoint.color {
                return None;
            }
            state.complete.insert(endpoint.color);
        } else {
            if state.grid[index] != 0 {
                return None;
            }
            state.grid[index] = endpoint.color;
        }
        state.paths.get_mut(&endpoint.color)?.push(index);
        previous = index;
    }
    Some(())
}

fn search(
    endpoints: &[EndpointPlan],
    state: &mut State,
    budget: &mut SearchBudget,
    exhausted: &mut HashSet<SearchKey>,
    board: &Board,
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
    if !all_incomplete_pairs_reachable(endpoints, state, board) {
        return SearchOutcome::Exhausted;
    }

    let key = search_key(endpoints, state);
    if exhausted.contains(&key) {
        return SearchOutcome::Exhausted;
    }

    let Some((endpoint, moves)) = choose_endpoint(endpoints, state, board) else {
        exhausted.insert(key);
        return SearchOutcome::Exhausted;
    };
    for (next, is_goal) in moves {
        let Some(path) = state.paths.get_mut(&endpoint.color) else {
            exhausted.insert(key);
            return SearchOutcome::Exhausted;
        };
        path.push(next);
        let previous = state.grid[next];
        if is_goal {
            state.complete.insert(endpoint.color);
        } else {
            state.grid[next] = endpoint.color;
        }

        match search(endpoints, state, budget, exhausted, board) {
            SearchOutcome::Solved(result) => return SearchOutcome::Solved(result),
            SearchOutcome::SearchLimitExceeded => return SearchOutcome::SearchLimitExceeded,
            SearchOutcome::Exhausted => {}
        }

        if is_goal {
            state.complete.remove(&endpoint.color);
        } else {
            state.grid[next] = previous;
        }
        let Some(path) = state.paths.get_mut(&endpoint.color) else {
            return SearchOutcome::Exhausted;
        };
        path.pop();
    }

    exhausted.insert(key);
    SearchOutcome::Exhausted
}

fn search_key(endpoints: &[EndpointPlan], state: &State) -> SearchKey {
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
            tip,
            state.complete.contains(&endpoint.color),
        ));
    }
    SearchKey {
        grid: state.grid.clone(),
        tips,
    }
}

fn choose_endpoint(
    endpoints: &[EndpointPlan],
    state: &State,
    board: &Board,
) -> Option<(EndpointPlan, Vec<(usize, bool)>)> {
    let mut best: Option<EndpointSearchCandidate> = None;
    for endpoint in endpoints {
        if state.complete.contains(&endpoint.color) {
            continue;
        }
        let moves = legal_moves(endpoint, state, board);
        if moves.is_empty() {
            return None;
        }
        let stats = reachability_stats(endpoint, state, board)?;
        if best
            .as_ref()
            .is_none_or(|(best_endpoint, best_moves, best_stats)| {
                (
                    moves.len(),
                    stats.distance_to_goal,
                    stats.reachable_cells,
                    endpoint.color,
                ) < (
                    best_moves.len(),
                    best_stats.distance_to_goal,
                    best_stats.reachable_cells,
                    best_endpoint.color,
                )
            })
        {
            best = Some((endpoint.clone(), moves, stats));
        }
    }
    best.map(|(endpoint, moves, _)| (endpoint, moves))
}

fn legal_moves(endpoint: &EndpointPlan, state: &State, board: &Board) -> Vec<(usize, bool)> {
    let Some(path) = state.paths.get(&endpoint.color) else {
        return Vec::new();
    };
    let Some(&tip) = path.last() else {
        return Vec::new();
    };
    let mut moves = board
        .neighbors(tip)
        .into_iter()
        .filter(|next| can_enter_cell(endpoint, state, *next))
        .map(|next| (next, next == endpoint.end))
        .collect::<Vec<_>>();
    moves.sort_by_key(|(next, is_goal)| {
        (
            shortest_distance_from(*next, endpoint.color, endpoint.end, state, board)
                .unwrap_or(usize::MAX),
            if *is_goal { 0 } else { 1 },
            open_neighbor_count(*next, endpoint, state, board),
            bend_score(endpoint, state, *next, board),
            *next,
        )
    });
    moves
}

fn all_incomplete_pairs_reachable(
    endpoints: &[EndpointPlan],
    state: &State,
    board: &Board,
) -> bool {
    endpoints
        .iter()
        .filter(|endpoint| !state.complete.contains(&endpoint.color))
        .all(|endpoint| reachability_stats(endpoint, state, board).is_some())
}

fn reachability_stats(
    endpoint: &EndpointPlan,
    state: &State,
    board: &Board,
) -> Option<ReachabilityStats> {
    let path = state.paths.get(&endpoint.color)?;
    let &start = path.last()?;
    let mut seen = HashSet::new();
    let mut queue = VecDeque::from([(start, 0usize)]);
    seen.insert(start);
    while let Some((index, distance)) = queue.pop_front() {
        if index == endpoint.end {
            return Some(ReachabilityStats {
                reachable_cells: seen.len(),
                distance_to_goal: distance,
            });
        }
        for next in board.neighbors(index) {
            if !seen.insert(next) {
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
    start: usize,
    color: i32,
    goal: usize,
    state: &State,
    board: &Board,
) -> Option<usize> {
    let mut seen = HashSet::new();
    let mut queue = VecDeque::from([(start, 0usize)]);
    seen.insert(start);
    while let Some((index, distance)) = queue.pop_front() {
        if index == goal {
            return Some(distance);
        }
        for next in board.neighbors(index) {
            if !seen.insert(next) {
                continue;
            }
            if can_enter_goal_or_empty(color, goal, state, next) {
                queue.push_back((next, distance + 1));
            }
        }
    }
    None
}

fn shortest_path(endpoint: &EndpointPlan, state: &State, board: &Board) -> Option<Vec<usize>> {
    let start = state.paths.get(&endpoint.color)?.last().copied()?;
    let mut seen = HashSet::new();
    let mut parents: HashMap<usize, usize> = HashMap::new();
    let mut queue = VecDeque::from([start]);
    seen.insert(start);
    while let Some(index) = queue.pop_front() {
        if index == endpoint.end {
            let mut result = vec![index];
            let mut cursor = index;
            while cursor != start {
                cursor = *parents.get(&cursor)?;
                result.push(cursor);
            }
            result.reverse();
            return Some(result);
        }
        for next in board.neighbors(index) {
            if !seen.insert(next) {
                continue;
            }
            if can_enter_cell(endpoint, state, next) {
                parents.insert(next, index);
                queue.push_back(next);
            }
        }
    }
    None
}

fn open_neighbor_count(
    index: usize,
    endpoint: &EndpointPlan,
    state: &State,
    board: &Board,
) -> usize {
    board
        .neighbors(index)
        .into_iter()
        .filter(|next| can_enter_cell(endpoint, state, *next))
        .count()
}

fn bend_score(endpoint: &EndpointPlan, state: &State, next: usize, board: &Board) -> usize {
    let Some(path) = state.paths.get(&endpoint.color) else {
        return 0;
    };
    if path.len() < 2 {
        return 0;
    }
    let previous = path[path.len() - 2];
    let tip = path[path.len() - 1];
    let prev_point = board.point(previous);
    let tip_point = board.point(tip);
    let next_point = board.point(next);
    usize::from(
        (tip_point[0] - prev_point[0], tip_point[1] - prev_point[1])
            != (next_point[0] - tip_point[0], next_point[1] - tip_point[1]),
    )
}

fn can_enter_cell(endpoint: &EndpointPlan, state: &State, index: usize) -> bool {
    can_enter_goal_or_empty(endpoint.color, endpoint.end, state, index)
}

fn can_enter_goal_or_empty(color: i32, goal: usize, state: &State, index: usize) -> bool {
    if index == goal {
        return state.grid[index] == color;
    }
    state.grid[index] == 0
}

fn steps_from_solution(
    session: &FlowfreeSession,
    endpoints: &[EndpointPlan],
    board: &Board,
    solved: State,
) -> Result<Vec<FlowfreeStep>, String> {
    validate_solution(&solved, endpoints, board)?;

    let mut colors = endpoints
        .iter()
        .map(|endpoint| endpoint.color)
        .collect::<Vec<_>>();
    colors.sort_unstable();

    let mut steps = Vec::new();
    if needs_reset(session, endpoints, board) {
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
        for index in path {
            let point = board.point(*index);
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
    board: &Board,
) -> Result<(), String> {
    let mut occupied = HashMap::new();
    for endpoint in endpoints {
        let path = solved
            .paths
            .get(&endpoint.color)
            .ok_or_else(|| "flowfree solution is missing a color path".to_string())?;
        if path.len() < 2 {
            return Err("flowfree solution path is too short".to_string());
        }
        let first = path.first().copied();
        let last = path.last().copied();
        let path_endpoints_match = (first == Some(endpoint.start) && last == Some(endpoint.end))
            || (first == Some(endpoint.end) && last == Some(endpoint.start));
        if !path_endpoints_match {
            return Err("flowfree solution path does not connect its endpoints".to_string());
        }
        for index in path {
            if solved.grid[*index] != endpoint.color {
                return Err("flowfree solution path uses a cell owned by another color".to_string());
            }
            if occupied.insert(*index, endpoint.color).is_some() {
                return Err("flowfree solution path reuses a cell".to_string());
            }
        }
        for pair in path.windows(2) {
            if manhattan(board.point(pair[0]), board.point(pair[1])) != 1 {
                return Err("flowfree solution path contains a non-adjacent step".to_string());
            }
        }
    }
    Ok(())
}

fn needs_reset(session: &FlowfreeSession, endpoints: &[EndpointPlan], board: &Board) -> bool {
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
        .collect::<HashSet<_>>();
    for r in 0..board.height {
        for c in 0..board.width {
            let value = session
                .cells
                .get(r)
                .and_then(|row| row.get(c))
                .copied()
                .unwrap_or(0);
            if value == 0 {
                continue;
            }
            let index = r * board.width + c;
            if !endpoint_cells.contains(&(index, value)) {
                return true;
            }
        }
    }
    false
}

fn has_forced_outer_boundary_crossing(endpoints: &[EndpointPlan], board: &Board) -> bool {
    for (left_index, left) in endpoints.iter().enumerate() {
        let Some((left_start, left_end)) = boundary_pair(left, board) else {
            continue;
        };
        for right in endpoints.iter().skip(left_index + 1) {
            let Some((right_start, right_end)) = boundary_pair(right, board) else {
                continue;
            };
            if boundary_pairs_alternate(left_start, left_end, right_start, right_end) {
                return true;
            }
        }
    }
    false
}

fn boundary_pair(endpoint: &EndpointPlan, board: &Board) -> Option<(usize, usize)> {
    Some((
        boundary_index(endpoint.start, board)?,
        boundary_index(endpoint.end, board)?,
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

fn boundary_index(index: usize, board: &Board) -> Option<usize> {
    if board.width < 2 || board.height < 2 {
        return None;
    }
    let r = index / board.width;
    let c = index % board.width;
    if r == 0 {
        return Some(c);
    }
    if c == board.width - 1 {
        return Some((board.width - 1) + r);
    }
    if r == board.height - 1 {
        return Some((board.width - 1) + (board.height - 1) + (board.width - 1 - c));
    }
    if c == 0 {
        return Some(
            (board.width - 1) + (board.height - 1) + (board.width - 1) + (board.height - 1 - r),
        );
    }
    None
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::FlowfreePath;

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
        assert_steps_connect_endpoints(&session, &steps);
    }

    #[test]
    fn solver_does_not_require_filling_the_whole_board() {
        let session = FlowfreeSession {
            width: 3,
            height: 3,
            endpoints: vec![FlowfreeEndpoint(1, [0, 0], [0, 2])],
            cells: vec![vec![1, 0, 1], vec![0, 0, 0], vec![0, 0, 0]],
            ..FlowfreeSession::default()
        };

        let steps = solve(&session).unwrap();

        assert_eq!(
            steps.iter().filter(|step| step.action == "paint").count(),
            3
        );
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

        assert_steps_connect_endpoints(&session, &steps);
    }

    #[test]
    fn solver_success_rate_9x9_six_color_generated_100_boards() {
        const TOTAL: usize = 100;
        let mut failures = Vec::new();
        for index in 0..TOTAL {
            let session = generated_9x9_six_color_session(index as u64);
            if let Err(error) = solve(&session).inspect(|steps| {
                assert_steps_connect_endpoints(&session, steps);
            }) {
                failures.push((index, error));
            }
        }

        assert!(failures.is_empty(), "failed generated boards: {failures:?}");
    }

    #[test]
    fn solver_resets_partial_paths_before_replay() {
        let session = FlowfreeSession {
            width: 3,
            height: 3,
            endpoints: vec![FlowfreeEndpoint(1, [0, 0], [0, 2])],
            cells: vec![vec![1, 1, 1], vec![0, 0, 0], vec![0, 0, 0]],
            paths: vec![FlowfreePath(1, vec![[0, 0], [0, 1]])],
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
    fn solver_proves_outer_boundary_crossing_unsolvable() {
        let session = FlowfreeSession {
            width: 9,
            height: 9,
            endpoints: vec![
                FlowfreeEndpoint(1, [0, 0], [8, 8]),
                FlowfreeEndpoint(2, [0, 8], [8, 0]),
                FlowfreeEndpoint(3, [2, 2], [6, 6]),
                FlowfreeEndpoint(4, [2, 6], [6, 2]),
                FlowfreeEndpoint(5, [4, 4], [0, 4]),
            ],
            cells: vec![vec![0; 9]; 9],
            ..FlowfreeSession::default()
        };

        let error = solve(&session).unwrap_err();

        assert!(is_proven_unsolvable_error(&error));
        assert!(!is_proven_unsolvable_error(SEARCH_LIMIT_ERROR));
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
