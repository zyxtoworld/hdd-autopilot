use std::collections::{HashSet, VecDeque};

use crate::model::{SokobanPoint, SokobanSession};

const DIRS: [(i32, i32, &str); 4] = [
    (-1, 0, "up"),
    (1, 0, "down"),
    (0, -1, "left"),
    (0, 1, "right"),
];

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct State {
    player: usize,
    boxes: Vec<usize>,
}

#[derive(Debug, Clone)]
struct Node {
    state: State,
    parent: Option<usize>,
    direction: Option<&'static str>,
}

pub fn solve(session: &SokobanSession) -> Result<Vec<String>, String> {
    let height = usize_from_i32(session.height, "推箱子高度")?;
    let width = usize_from_i32(session.width, "推箱子宽度")?;
    let walls = points_to_set(&session.walls, width, height)?;
    let targets = points_to_set(&session.targets, width, height)?;
    let mut boxes = session
        .boxes
        .iter()
        .map(|&point| point_index(point, width, height))
        .collect::<Result<Vec<_>, _>>()?;
    boxes.sort_unstable();
    let start = State {
        player: point_index(session.player, width, height)?,
        boxes,
    };
    let mut nodes = vec![Node {
        state: start.clone(),
        parent: None,
        direction: None,
    }];
    let mut queue = VecDeque::from([0usize]);
    let mut seen = HashSet::from([start]);
    const STATE_LIMIT: usize = 500_000;

    while let Some(node_index) = queue.pop_front() {
        if nodes[node_index]
            .state
            .boxes
            .iter()
            .all(|position| targets.contains(position))
        {
            return Ok(reconstruct_path(&nodes, node_index));
        }
        if nodes.len() > STATE_LIMIT {
            return Err("推箱子搜索状态过多".to_string());
        }
        let state = nodes[node_index].state.clone();
        let box_set = state.boxes.iter().copied().collect::<HashSet<_>>();
        for (dr, dc, direction) in DIRS {
            let Some(next_player) = offset_index(state.player, dr, dc, width, height) else {
                continue;
            };
            if walls.contains(&next_player) {
                continue;
            }
            let mut next_boxes = state.boxes.clone();
            if box_set.contains(&next_player) {
                let Some(pushed_box) = offset_index(next_player, dr, dc, width, height) else {
                    continue;
                };
                if walls.contains(&pushed_box) || box_set.contains(&pushed_box) {
                    continue;
                }
                let Some(slot) = next_boxes
                    .iter_mut()
                    .find(|position| **position == next_player)
                else {
                    continue;
                };
                *slot = pushed_box;
                next_boxes.sort_unstable();
                if has_dead_box(&next_boxes, &targets, &walls, width, height) {
                    continue;
                }
            }
            let next_state = State {
                player: next_player,
                boxes: next_boxes,
            };
            if seen.insert(next_state.clone()) {
                nodes.push(Node {
                    state: next_state,
                    parent: Some(node_index),
                    direction: Some(direction),
                });
                queue.push_back(nodes.len() - 1);
            }
        }
    }
    Err("推箱子没有找到通关路径".to_string())
}

fn reconstruct_path(nodes: &[Node], mut index: usize) -> Vec<String> {
    let mut directions = Vec::new();
    while let Some(parent) = nodes[index].parent {
        if let Some(direction) = nodes[index].direction {
            directions.push(direction.to_string());
        }
        index = parent;
    }
    directions.reverse();
    directions
}

fn has_dead_box(
    boxes: &[usize],
    targets: &HashSet<usize>,
    walls: &HashSet<usize>,
    width: usize,
    height: usize,
) -> bool {
    boxes.iter().copied().any(|position| {
        if targets.contains(&position) {
            return false;
        }
        let up = neighbor_blocked(position, -1, 0, walls, width, height);
        let down = neighbor_blocked(position, 1, 0, walls, width, height);
        let left = neighbor_blocked(position, 0, -1, walls, width, height);
        let right = neighbor_blocked(position, 0, 1, walls, width, height);
        (up || down) && (left || right)
    })
}

fn neighbor_blocked(
    position: usize,
    dr: i32,
    dc: i32,
    walls: &HashSet<usize>,
    width: usize,
    height: usize,
) -> bool {
    offset_index(position, dr, dc, width, height)
        .map(|index| walls.contains(&index))
        .unwrap_or(true)
}

fn usize_from_i32(value: i32, label: &str) -> Result<usize, String> {
    usize::try_from(value)
        .ok()
        .filter(|value| *value > 0)
        .ok_or_else(|| format!("{label}无效"))
}

fn points_to_set(
    points: &[SokobanPoint],
    width: usize,
    height: usize,
) -> Result<HashSet<usize>, String> {
    points
        .iter()
        .map(|&point| point_index(point, width, height))
        .collect()
}

fn point_index(point: SokobanPoint, width: usize, height: usize) -> Result<usize, String> {
    let r = usize::try_from(point[0]).map_err(|_| "坐标无效".to_string())?;
    let c = usize::try_from(point[1]).map_err(|_| "坐标无效".to_string())?;
    if r >= height || c >= width {
        return Err("坐标超出棋盘".to_string());
    }
    Ok(r * width + c)
}

fn offset_index(index: usize, dr: i32, dc: i32, width: usize, height: usize) -> Option<usize> {
    let r = i32::try_from(index / width).ok()? + dr;
    let c = i32::try_from(index % width).ok()? + dc;
    if r < 0 || c < 0 || r >= height as i32 || c >= width as i32 {
        return None;
    }
    Some(r as usize * width + c as usize)
}
