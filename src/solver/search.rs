use std::collections::HashSet;

use crate::model::{Powerups, SessionSnapshot, Tile};

use super::{Action, ActionPlan, BoundarySearchResult, SearchContext, SearchResult, SearchState};

pub(super) fn find_winning_click_with_forbidden(
    snapshot: &SessionSnapshot,
    forbidden: &HashSet<i32>,
    search_budget: usize,
) -> Option<i32> {
    let (actions, solved, _) = find_winning_path_with_forbidden(snapshot, search_budget, forbidden);
    if solved {
        actions.first().copied()
    } else {
        None
    }
}

pub(super) fn plan_click_only(
    snapshot: &SessionSnapshot,
    budget: usize,
) -> (ActionPlan, bool, bool) {
    let (path, solved, cutoff) = find_winning_path(snapshot, budget);
    if solved {
        return (
            ActionPlan {
                actions: path
                    .into_iter()
                    .map(|tile_id| Action {
                        kind: "click",
                        tile_id,
                    })
                    .collect(),
                completed: true,
            },
            true,
            false,
        );
    }
    (ActionPlan::default(), false, cutoff)
}

fn find_winning_path(snapshot: &SessionSnapshot, budget: usize) -> (Vec<i32>, bool, bool) {
    find_winning_path_with_forbidden(snapshot, budget, &HashSet::new())
}

fn find_winning_path_with_forbidden(
    snapshot: &SessionSnapshot,
    budget: usize,
    forbidden: &HashSet<i32>,
) -> (Vec<i32>, bool, bool) {
    let mut ctx = SearchContext {
        budget,
        ..SearchContext::default()
    };
    let state = SearchState {
        board: snapshot.tiles.clone(),
        slots: snapshot.slot_tiles.clone(),
        slot_limit: snapshot.slot_limit,
        forbidden: forbidden.clone(),
    };
    let result = find_winning_path_inner(&mut ctx, state);
    (
        result.actions,
        result.solved,
        result.cutoff || ctx.budget_hit,
    )
}

fn find_winning_path_inner(ctx: &mut SearchContext, state: SearchState) -> SearchResult {
    if state.board.is_empty() {
        return SearchResult {
            solved: state.slots.is_empty(),
            ..SearchResult::default()
        };
    }
    if ctx.visited >= ctx.budget {
        ctx.budget_hit = true;
        return SearchResult {
            cutoff: true,
            ..SearchResult::default()
        };
    }
    ctx.visited += 1;

    let key = state_key(&state);
    if ctx.failed.contains(&key) {
        return SearchResult::default();
    }

    let tiles = ordered_tiles_by_id_desc(&state.board, &state.forbidden);
    let mut cutoff = false;
    for tile in tiles {
        let Some(next) = apply_click(&state, tile.id) else {
            continue;
        };
        let result = find_winning_path_inner(ctx, next);
        if result.solved {
            let mut actions = Vec::with_capacity(result.actions.len() + 1);
            actions.push(tile.id);
            actions.extend(result.actions);
            return SearchResult {
                actions,
                solved: true,
                ..SearchResult::default()
            };
        }
        if result.cutoff {
            cutoff = true;
        }
    }

    if cutoff {
        return SearchResult {
            cutoff: true,
            ..SearchResult::default()
        };
    }
    ctx.failed.insert(key);
    SearchResult::default()
}

pub(super) fn plan_to_powerup_boundary(
    snapshot: &SessionSnapshot,
    budget: usize,
) -> (ActionPlan, bool, bool) {
    let mut ctx = SearchContext {
        budget,
        ..SearchContext::default()
    };
    let state = SearchState {
        board: snapshot.tiles.clone(),
        slots: snapshot.slot_tiles.clone(),
        slot_limit: snapshot.slot_limit,
        forbidden: HashSet::new(),
    };
    let result = find_plan_to_powerup_boundary(&mut ctx, &snapshot.powerups, state);
    if result.cutoff || ctx.budget_hit {
        return (ActionPlan::default(), false, true);
    }
    if result.solved || result.boundary {
        return (
            ActionPlan {
                actions: result.actions,
                completed: result.solved,
            },
            true,
            false,
        );
    }
    (ActionPlan::default(), false, false)
}

fn find_plan_to_powerup_boundary(
    ctx: &mut SearchContext,
    powerups: &Powerups,
    state: SearchState,
) -> BoundarySearchResult {
    if state.board.is_empty() {
        if state.slots.is_empty() {
            return BoundarySearchResult {
                solved: true,
                ..BoundarySearchResult::default()
            };
        }
        if let Some(action) = available_powerup_boundary(powerups, &state.slots) {
            return BoundarySearchResult {
                actions: vec![action],
                boundary: true,
                ..BoundarySearchResult::default()
            };
        }
        return BoundarySearchResult::default();
    }
    if ctx.visited >= ctx.budget {
        ctx.budget_hit = true;
        return BoundarySearchResult {
            cutoff: true,
            ..BoundarySearchResult::default()
        };
    }
    ctx.visited += 1;

    let key = state_key(&state);
    if ctx.failed.contains(&key) {
        return BoundarySearchResult::default();
    }

    let tiles = ordered_tiles_by_id_desc(&state.board, &state.forbidden);
    let mut cutoff = false;
    let mut boundary_plan = Vec::new();
    for tile in tiles {
        let Some(next) = apply_click(&state, tile.id) else {
            continue;
        };
        let result = find_plan_to_powerup_boundary(ctx, powerups, next);
        if result.solved {
            let mut actions = Vec::with_capacity(result.actions.len() + 1);
            actions.push(Action {
                kind: "click",
                tile_id: tile.id,
            });
            actions.extend(result.actions);
            return BoundarySearchResult {
                actions,
                solved: true,
                ..BoundarySearchResult::default()
            };
        }
        if result.cutoff {
            cutoff = true;
            continue;
        }
        if result.boundary && boundary_plan.is_empty() {
            boundary_plan.push(Action {
                kind: "click",
                tile_id: tile.id,
            });
            boundary_plan.extend(result.actions);
        }
    }

    if cutoff {
        return BoundarySearchResult {
            cutoff: true,
            ..BoundarySearchResult::default()
        };
    }
    if !boundary_plan.is_empty() {
        return BoundarySearchResult {
            actions: boundary_plan,
            boundary: true,
            ..BoundarySearchResult::default()
        };
    }
    if let Some(action) = available_powerup_boundary(powerups, &state.slots) {
        return BoundarySearchResult {
            actions: vec![action],
            boundary: true,
            ..BoundarySearchResult::default()
        };
    }
    ctx.failed.insert(key);
    BoundarySearchResult::default()
}

fn available_powerup_boundary(powerups: &Powerups, slots: &[Tile]) -> Option<Action> {
    if powerups.undo > 0 && !slots.is_empty() {
        return Some(Action {
            kind: "undo",
            tile_id: 0,
        });
    }
    if powerups.remove > 0 && slots.len() >= 3 {
        return Some(Action {
            kind: "remove",
            tile_id: 0,
        });
    }
    if powerups.shuffle > 0 {
        return Some(Action {
            kind: "shuffle",
            tile_id: 0,
        });
    }
    None
}

pub(super) fn ordered_tiles_by_id_desc(tiles: &[Tile], forbidden: &HashSet<i32>) -> Vec<Tile> {
    let mut ordered = tiles
        .iter()
        .filter(|tile| !forbidden.contains(&tile.id))
        .cloned()
        .collect::<Vec<_>>();
    ordered.sort_by_key(|tile| std::cmp::Reverse(tile.id));
    ordered
}

fn apply_click(state: &SearchState, tile_id: i32) -> Option<SearchState> {
    let mut board = Vec::with_capacity(state.board.len().saturating_sub(1));
    let mut clicked = None;
    for tile in &state.board {
        if tile.id == tile_id {
            clicked = Some(tile.clone());
            continue;
        }
        board.push(tile.clone());
    }
    let clicked = clicked?;
    if state.forbidden.contains(&tile_id) {
        return None;
    }

    let mut slots = state.slots.clone();
    slots.push(clicked.clone());
    let pattern_count = slots
        .iter()
        .filter(|tile| tile.pattern == clicked.pattern)
        .count();
    if pattern_count >= 3 {
        let mut removed = 0usize;
        slots.retain(|tile| {
            if tile.pattern == clicked.pattern && removed < 3 {
                removed += 1;
                return false;
            }
            true
        });
    }
    if slots.len() as i32 > state.slot_limit {
        return None;
    }
    Some(SearchState {
        board,
        slots,
        slot_limit: state.slot_limit,
        forbidden: state.forbidden.clone(),
    })
}

fn state_key(state: &SearchState) -> String {
    let mut board_ids = state.board.iter().map(|tile| tile.id).collect::<Vec<_>>();
    board_ids.sort_unstable();
    let mut slot_ids = state.slots.iter().map(|tile| tile.id).collect::<Vec<_>>();
    slot_ids.sort_unstable();
    let mut forbidden_ids = state.forbidden.iter().copied().collect::<Vec<_>>();
    forbidden_ids.sort_unstable();

    let board = join_i32s(&board_ids);
    let slots = join_i32s(&slot_ids);
    let forbidden = join_i32s(&forbidden_ids);
    format!("{board}|{slots}|{forbidden}")
}

fn join_i32s(values: &[i32]) -> String {
    values
        .iter()
        .map(i32::to_string)
        .collect::<Vec<_>>()
        .join(",")
}
