use std::collections::VecDeque;

use crate::model::{LogicGameSession, LogicGameStep, LogicPoint};

pub fn solve(session: &LogicGameSession) -> Result<Vec<LogicGameStep>, String> {
    let height = usize_from_i32(session.height, "迷宫高度")?;
    let width = usize_from_i32(session.width, "迷宫宽度")?;
    let start = point_index(session.player, width, height)?;
    let goal = point_index(session.exit, width, height)?;
    let mut graph = vec![Vec::<(usize, &'static str)>::new(); width * height];
    for edge in &session.open_edges {
        let a = point_index(edge[0], width, height)?;
        let b = point_index(edge[1], width, height)?;
        let dir_ab = direction_between(edge[0], edge[1])?;
        let dir_ba = direction_between(edge[1], edge[0])?;
        graph[a].push((b, dir_ab));
        graph[b].push((a, dir_ba));
    }

    let mut queue = VecDeque::from([start]);
    let mut prev = vec![None::<(usize, &'static str)>; width * height];
    prev[start] = Some((start, ""));
    while let Some(node) = queue.pop_front() {
        if node == goal {
            break;
        }
        for &(next, direction) in &graph[node] {
            if prev[next].is_none() {
                prev[next] = Some((node, direction));
                queue.push_back(next);
            }
        }
    }
    if prev[goal].is_none() {
        return Err("迷宫没有找到出口路径".to_string());
    }

    let mut directions = Vec::new();
    let mut cur = goal;
    while cur != start {
        let (parent, direction) = prev[cur].ok_or_else(|| "迷宫路径回溯失败".to_string())?;
        directions.push(direction.to_string());
        cur = parent;
    }
    directions.reverse();
    Ok(directions
        .into_iter()
        .map(|direction| LogicGameStep::Move { direction })
        .collect())
}

fn usize_from_i32(value: i32, label: &str) -> Result<usize, String> {
    usize::try_from(value)
        .ok()
        .filter(|value| *value > 0)
        .ok_or_else(|| format!("{label}无效"))
}

fn point_index(point: LogicPoint, width: usize, height: usize) -> Result<usize, String> {
    let r = usize::try_from(point[0]).map_err(|_| "坐标无效".to_string())?;
    let c = usize::try_from(point[1]).map_err(|_| "坐标无效".to_string())?;
    if r >= height || c >= width {
        return Err("坐标超出棋盘".to_string());
    }
    Ok(r * width + c)
}

fn direction_between(from: LogicPoint, to: LogicPoint) -> Result<&'static str, String> {
    match (to[0] - from[0], to[1] - from[1]) {
        (-1, 0) => Ok("up"),
        (1, 0) => Ok("down"),
        (0, -1) => Ok("left"),
        (0, 1) => Ok("right"),
        _ => Err("迷宫边不是相邻格".to_string()),
    }
}
