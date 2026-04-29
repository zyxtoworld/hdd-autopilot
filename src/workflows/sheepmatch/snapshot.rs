use crate::model::{HistoryItem, SessionSnapshot, StartResponse, StepResponse, Tile};

pub(super) fn snapshot_from_start_response(start: &StartResponse) -> SessionSnapshot {
    SessionSnapshot {
        difficulty: normalize_start_difficulty(start),
        session_id: start.session_id,
        slot_limit: normalize_slot_limit(start.slot_limit, 0),
        powerups: start.powerups.clone(),
        status: normalize_round_status(&start.status),
        tiles: clone_tiles_without_ids(&start.tiles, &start.slots),
        slot_tiles: clone_slot_tiles(&start.slot_tiles, &start.slots, &start.tiles),
        move_count: start.move_count,
    }
}

pub(super) fn history_item_to_start_response(item: &HistoryItem) -> StartResponse {
    StartResponse {
        difficulty: item.difficulty.clone(),
        move_count: item.move_count,
        powerups: item.powerups.clone(),
        session_id: item.session_id,
        slot_limit: item.slot_limit,
        slots: item.slots.clone(),
        slot_tiles: item.slot_tiles.clone(),
        status: item.status.clone(),
        tiles: item.tiles.clone(),
        ..StartResponse::default()
    }
}

pub(super) fn snapshot_from_step_response(
    previous: &SessionSnapshot,
    step: &StepResponse,
) -> SessionSnapshot {
    let mut next = previous.clone();
    next.move_count = step.move_count;
    next.status = if step.status.trim().is_empty() {
        previous.status.clone()
    } else {
        step.status.clone()
    };
    if step.session_id != 0 {
        next.session_id = step.session_id;
    }
    if step.slot_limit > 0 {
        next.slot_limit = step.slot_limit;
    }
    if let Some(powerups) = &step.powerups {
        next.powerups = powerups.clone();
    }
    if step.slots.is_some() {
        next.slot_tiles = resolve_slot_tiles(previous, step);
    }
    if let Some(tiles) = &step.tiles {
        next.tiles = clone_tiles_without_ids(tiles, &collect_tile_ids(&next.slot_tiles));
    }
    if !step.removed.is_empty() {
        next.tiles = remove_tiles_by_id(&next.tiles, &step.removed);
    }
    next
}

fn resolve_slot_tiles(previous: &SessionSnapshot, step: &StepResponse) -> Vec<Tile> {
    let Some(slots) = &step.slots else {
        return previous.slot_tiles.clone();
    };
    let mut lookup = std::collections::HashMap::<i32, Tile>::new();
    let step_tiles = step.tiles.as_deref().unwrap_or(&[]);
    for tile in previous
        .slot_tiles
        .iter()
        .chain(previous.tiles.iter())
        .chain(step_tiles.iter())
    {
        lookup.insert(tile.id, tile.clone());
    }
    slots
        .iter()
        .filter_map(|id| lookup.get(id).cloned())
        .collect()
}

fn normalize_start_difficulty(start: &StartResponse) -> String {
    start.difficulty.trim().to_string()
}

fn normalize_round_status(status: &str) -> String {
    let trimmed = status.trim();
    if trimmed.is_empty() {
        "pending".to_string()
    } else {
        trimmed.to_string()
    }
}

fn normalize_slot_limit(slot_limit: i32, fallback: i32) -> i32 {
    if slot_limit > 0 { slot_limit } else { fallback }
}

fn clone_tiles_without_ids(tiles: &[Tile], excluded: &[i32]) -> Vec<Tile> {
    let excluded: std::collections::HashSet<i32> = excluded.iter().copied().collect();
    tiles
        .iter()
        .filter(|tile| !excluded.contains(&tile.id))
        .cloned()
        .collect()
}

fn clone_slot_tiles(explicit: &[Tile], slots: &[i32], board: &[Tile]) -> Vec<Tile> {
    if !explicit.is_empty() {
        return explicit.to_vec();
    }
    let lookup = board
        .iter()
        .map(|tile| (tile.id, tile.clone()))
        .collect::<std::collections::HashMap<_, _>>();
    slots
        .iter()
        .filter_map(|id| lookup.get(id).cloned())
        .collect()
}

pub(super) fn collect_tile_ids(tiles: &[Tile]) -> Vec<i32> {
    tiles.iter().map(|tile| tile.id).collect()
}

pub(super) fn fixed_click_queue(snapshot: &SessionSnapshot) -> Vec<i32> {
    let mut ids = collect_tile_ids(&snapshot.tiles);
    ids.sort_unstable_by(|a, b| b.cmp(a));
    ids
}

pub(super) fn is_stale_click_error(message: &str) -> bool {
    let message = message.trim().to_ascii_lowercase();
    message.contains("目标方块已不在棋盘上")
        || message.contains("tile not on board")
        || message.contains("tile is covered")
        || message.contains("session not found")
        || message.contains("invalid action")
}

fn remove_tiles_by_id(tiles: &[Tile], ids: &[i32]) -> Vec<Tile> {
    let removed: std::collections::HashSet<i32> = ids.iter().copied().collect();
    tiles
        .iter()
        .filter(|tile| !removed.contains(&tile.id))
        .cloned()
        .collect()
}

pub(super) fn is_solved(snapshot: &SessionSnapshot) -> bool {
    snapshot.status == "won" || (snapshot.tiles.is_empty() && snapshot.slot_tiles.is_empty())
}

pub(super) fn is_slot_full_error(message: &str) -> bool {
    let message = message.trim().to_ascii_lowercase();
    message.contains("槽位已满") || message.contains("slot full")
}
