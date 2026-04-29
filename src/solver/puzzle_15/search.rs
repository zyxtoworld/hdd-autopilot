use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};

use super::board::{
    TileMove, blank_index, goal_board, heuristic, is_goal, legal_tile_moves,
    manhattan_distance_table, normalize_board, validate_solvable,
};

#[derive(Debug, Clone, PartialEq, Eq)]
struct Parent {
    previous: Vec<u8>,
    direction: TileMove,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct QueueItem {
    priority: i32,
    cost: i32,
    sequence: u64,
    blank: usize,
    last_direction: Option<TileMove>,
    board: Vec<u8>,
}

impl Ord for QueueItem {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .priority
            .cmp(&self.priority)
            .then_with(|| other.cost.cmp(&self.cost))
            .then_with(|| other.sequence.cmp(&self.sequence))
    }
}

impl PartialOrd for QueueItem {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Debug, Clone)]
struct SearchLimits {
    weight: i32,
    max_nodes: usize,
}

pub fn solve(board: &[i32], size: i32) -> Result<Vec<TileMove>, String> {
    let size = usize::try_from(size).map_err(|_| "棋盘尺寸无效".to_string())?;
    let board = normalize_board(board, size)?;
    validate_solvable(&board, size)?;
    if is_goal(&board) {
        return Ok(Vec::new());
    }

    for limits in search_limits(size) {
        if let Some(path) = weighted_a_star(&board, size, &limits)? {
            return Ok(path);
        }
    }
    Err("当前棋盘在搜索预算内没有找到解法".to_string())
}

fn weighted_a_star(
    start: &[u8],
    size: usize,
    limits: &SearchLimits,
) -> Result<Option<Vec<TileMove>>, String> {
    let goal = goal_board(size);
    let distance = manhattan_distance_table(size);
    let mut open = BinaryHeap::new();
    let mut best_cost = HashMap::<Vec<u8>, i32>::new();
    let mut parents = HashMap::<Vec<u8>, Parent>::new();
    let mut sequence = 0u64;

    let start_blank = blank_index(start)?;
    let start_heuristic = heuristic(start, &distance);
    open.push(QueueItem {
        priority: limits.weight * start_heuristic,
        cost: 0,
        sequence,
        blank: start_blank,
        last_direction: None,
        board: start.to_vec(),
    });
    best_cost.insert(start.to_vec(), 0);

    let mut visited = 0usize;
    while let Some(item) = open.pop() {
        if best_cost.get(&item.board).copied() != Some(item.cost) {
            continue;
        }
        if item.board == goal {
            return Ok(Some(reconstruct_path(item.board, &parents)));
        }
        visited += 1;
        if visited >= limits.max_nodes {
            return Ok(None);
        }

        for (direction, tile_index) in legal_tile_moves(size, item.blank) {
            if item.last_direction == Some(direction.reverse()) {
                continue;
            }
            let mut next = item.board.clone();
            next[item.blank] = next[tile_index];
            next[tile_index] = 0;
            let next_cost = item.cost + 1;
            if next_cost >= best_cost.get(&next).copied().unwrap_or(i32::MAX) {
                continue;
            }
            best_cost.insert(next.clone(), next_cost);
            parents.insert(
                next.clone(),
                Parent {
                    previous: item.board.clone(),
                    direction,
                },
            );
            sequence = sequence.saturating_add(1);
            let estimate = heuristic(&next, &distance);
            open.push(QueueItem {
                priority: next_cost + limits.weight * estimate,
                cost: next_cost,
                sequence,
                blank: tile_index,
                last_direction: Some(direction),
                board: next,
            });
        }
    }

    Ok(None)
}

fn reconstruct_path(mut board: Vec<u8>, parents: &HashMap<Vec<u8>, Parent>) -> Vec<TileMove> {
    let mut path = Vec::new();
    while let Some(parent) = parents.get(&board) {
        path.push(parent.direction);
        board = parent.previous.clone();
    }
    path.reverse();
    path
}

fn search_limits(size: usize) -> Vec<SearchLimits> {
    match size {
        0..=3 => vec![SearchLimits {
            weight: 1,
            max_nodes: 200_000,
        }],
        4 => vec![
            SearchLimits {
                weight: 3,
                max_nodes: 300_000,
            },
            SearchLimits {
                weight: 8,
                max_nodes: 500_000,
            },
        ],
        _ => vec![
            SearchLimits {
                weight: 12,
                max_nodes: 300_000,
            },
            SearchLimits {
                weight: 30,
                max_nodes: 800_000,
            },
        ],
    }
}
