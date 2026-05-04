pub mod flowfree;
pub mod lightsout;
pub mod maze;
pub mod memory;
pub mod minesweeper;
pub mod nonogram;
pub mod puzzle_15;
pub mod puzzle_2048;
mod search;
pub mod sokoban;
pub mod sudoku;

use std::collections::HashSet;

use crate::model::{SessionSnapshot, Tile};

use self::search::{
    find_winning_click_with_forbidden, ordered_tiles_by_id_desc, plan_click_only,
    plan_to_powerup_boundary,
};

const DEFAULT_BUDGET: usize = 2_000;
const DEFAULT_ATTEMPTS: usize = 2;
const SEARCH_BUDGET: usize = 50_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Action {
    pub kind: &'static str,
    pub tile_id: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Decision {
    pub action: Option<Action>,
    pub done: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ActionPlan {
    pub actions: Vec<Action>,
    pub completed: bool,
}

#[derive(Debug, Clone, Default)]
struct SearchContext {
    visited: usize,
    budget: usize,
    budget_hit: bool,
    failed: HashSet<String>,
}

#[derive(Debug, Clone, Default)]
struct SearchResult {
    actions: Vec<i32>,
    solved: bool,
    cutoff: bool,
}

#[derive(Debug, Clone, Default)]
struct BoundarySearchResult {
    actions: Vec<Action>,
    solved: bool,
    boundary: bool,
    cutoff: bool,
}

#[derive(Debug, Clone, Default)]
struct SearchState {
    board: Vec<Tile>,
    slots: Vec<Tile>,
    slot_limit: i32,
    forbidden: HashSet<i32>,
}

pub fn plan_to_tool_boundary(snapshot: &SessionSnapshot) -> Result<ActionPlan, String> {
    plan_to_tool_boundary_with_budget(snapshot, DEFAULT_BUDGET, DEFAULT_ATTEMPTS)
}

pub fn plan_to_tool_boundary_with_budget(
    snapshot: &SessionSnapshot,
    initial_budget: usize,
    attempts: usize,
) -> Result<ActionPlan, String> {
    let mut budget = initial_budget.max(DEFAULT_BUDGET);
    let attempts = attempts.max(1);
    for _ in 0..attempts {
        let (plan, solved, cutoff) = plan_click_only(snapshot, budget);
        if solved {
            return Ok(plan);
        }
        if !cutoff {
            let (boundary_plan, found, boundary_cutoff) =
                plan_to_powerup_boundary(snapshot, budget);
            if boundary_cutoff {
                budget = budget.saturating_mul(2);
                continue;
            }
            if found && !boundary_plan.actions.is_empty() {
                return Ok(boundary_plan);
            }
            break;
        }
        budget = budget.saturating_mul(2);
    }

    let decision = next(snapshot);
    if decision.done {
        return Err("当前整局无法生成可执行计划".to_string());
    }
    if let Some(action) = decision.action {
        return Ok(ActionPlan {
            actions: vec![action],
            completed: false,
        });
    }
    Err("当前整局无法生成可执行计划".to_string())
}

pub fn next(snapshot: &SessionSnapshot) -> Decision {
    next_with_forbidden(snapshot, &HashSet::new())
}

pub fn next_with_forbidden(snapshot: &SessionSnapshot, forbidden: &HashSet<i32>) -> Decision {
    if is_solved_snapshot(snapshot) {
        return Decision {
            action: None,
            done: true,
        };
    }

    if let Some(tile_id) = find_winning_click_with_forbidden(snapshot, forbidden, SEARCH_BUDGET) {
        return Decision {
            action: Some(Action {
                kind: "click",
                tile_id,
            }),
            done: false,
        };
    }

    next_greedy_with_forbidden(snapshot, forbidden)
}

pub fn next_greedy_with_forbidden(
    snapshot: &SessionSnapshot,
    forbidden: &HashSet<i32>,
) -> Decision {
    if is_solved_snapshot(snapshot) {
        return Decision {
            action: None,
            done: true,
        };
    }

    let ordered = ordered_tiles_by_id_desc(&snapshot.tiles, forbidden);
    if let Some(tile) = ordered.first() {
        return Decision {
            action: Some(Action {
                kind: "click",
                tile_id: tile.id,
            }),
            done: false,
        };
    }

    fallback(snapshot)
}

fn fallback(snapshot: &SessionSnapshot) -> Decision {
    if snapshot.powerups.undo > 0 && !snapshot.slot_tiles.is_empty() {
        return Decision {
            action: Some(Action {
                kind: "undo",
                tile_id: 0,
            }),
            done: false,
        };
    }
    if snapshot.powerups.remove > 0 && snapshot.slot_tiles.len() >= 3 {
        return Decision {
            action: Some(Action {
                kind: "remove",
                tile_id: 0,
            }),
            done: false,
        };
    }
    if snapshot.powerups.shuffle > 0 {
        return Decision {
            action: Some(Action {
                kind: "shuffle",
                tile_id: 0,
            }),
            done: false,
        };
    }
    Decision {
        action: None,
        done: true,
    }
}

fn is_solved_snapshot(snapshot: &SessionSnapshot) -> bool {
    snapshot.status == "won" || (snapshot.tiles.is_empty() && snapshot.slot_tiles.is_empty())
}
