use std::collections::HashSet;
use std::io;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::model::{
    Puzzle2048ConfigResponse, Puzzle2048HistoryItem, Puzzle2048MoveResponse,
    Puzzle2048StartResponse,
};
use crate::solver::puzzle_2048::{self, DEFAULT_DIRECTIONS, Direction};
use crate::ui;
use crate::workflows::common::{
    AccountRuntime, BatchState, current_unix_ms, is_pending_round_status,
    is_state_conflict_api_error, retry_operation_with_step, sleep_min_interval,
    with_auth_retry_api_mutation_until_success, with_auth_retry_api_until_success,
};

use super::types::{PuzzleDifficultySummary, PuzzleRoundSummary, PuzzleSnapshot, RoundProgress};

const STATE_SYNC_RETRY_DELAY: Duration = Duration::from_millis(500);

pub(super) fn difficulty_order(config: &Puzzle2048ConfigResponse) -> Vec<String> {
    let mut ordered = Vec::new();
    for difficulty in crate::model::PUZZLE_2048_DIFFICULTY_ORDER {
        if config.difficulties.contains_key(*difficulty) {
            ordered.push((*difficulty).to_string());
        }
    }
    let seen = ordered.iter().cloned().collect::<HashSet<_>>();
    let mut extra = config
        .difficulties
        .keys()
        .filter(|difficulty| !seen.contains(*difficulty))
        .cloned()
        .collect::<Vec<_>>();
    extra.sort();
    ordered.extend(extra);
    ordered
}

pub(super) fn allowed_directions(config: &Puzzle2048ConfigResponse) -> Vec<Direction> {
    let parsed = config
        .directions
        .iter()
        .filter_map(|direction| parse_direction(direction))
        .collect::<Vec<_>>();
    if parsed.is_empty() {
        DEFAULT_DIRECTIONS.to_vec()
    } else {
        parsed
    }
}

pub(super) fn is_pending_item(item: &Puzzle2048HistoryItem) -> bool {
    if item.session_id <= 0 || item.board.is_empty() || item.won || item.game_over {
        return false;
    }
    let status = item.status.trim().to_ascii_lowercase();
    status.is_empty() || matches!(status.as_str(), "pending" | "running" | "active")
}

pub(super) fn play_round(
    cancel_flag: &ui::CancelFlag,
    state: &Arc<Mutex<BatchState>>,
    runtime: &mut AccountRuntime,
    config: &Puzzle2048ConfigResponse,
    start: PuzzleSnapshot,
    continued: bool,
    progress: RoundProgress,
) -> io::Result<PuzzleRoundSummary> {
    let started = Instant::now();
    let directions = allowed_directions(config);
    let four_ratio = config.four_ratio;
    let mut snapshot = start;
    let mut last_reward = 0.0;
    let mut consecutive_fail = 0;

    loop {
        ui::check_cancel(cancel_flag)?;
        if is_finished(&snapshot) {
            return Ok(build_round_summary(
                runtime.email(),
                &snapshot,
                continued,
                &progress,
                started,
                last_reward,
                String::new(),
            ));
        }
        let direction = puzzle_2048::choose_next_move(
            &snapshot.board,
            snapshot.target_tile,
            four_ratio,
            &directions,
        )
        .unwrap_or_else(|| directions.first().copied().unwrap_or(Direction::Up));
        let valid_dirs = puzzle_2048::legal_moves(&snapshot.board, &directions);
        sleep_min_interval(cancel_flag, config.min_interval_ms)?;
        let response = move_once(
            cancel_flag,
            state,
            runtime,
            &snapshot,
            snapshot.session_id,
            direction,
            snapshot.move_count + 1,
        );
        match response {
            Ok(step) => {
                if !step.ok {
                    let next_snapshot = snapshot_from_move_response(&snapshot, &step);
                    if is_finished(&next_snapshot) {
                        return Ok(build_round_summary(
                            runtime.email(),
                            &next_snapshot,
                            continued,
                            &progress,
                            started,
                            step.reward_amount,
                            String::new(),
                        ));
                    }
                    consecutive_fail += 1;
                    if consecutive_fail >= 3 {
                        return fail_round_without_abandon(
                            cancel_flag,
                            runtime,
                            &snapshot,
                            RoundFailure::new(continued, &progress, started, "stuck"),
                        );
                    }
                    continue;
                }
                consecutive_fail = 0;
                last_reward = step.reward_amount;
                snapshot = snapshot_from_move_response(&snapshot, &step);
                if !step.changed && !is_finished(&snapshot) {
                    state.lock().unwrap().log.line_fmt(format_args!(
                        "账号 {} 的{}难度对局 {} 本次 {} 没有改变棋盘，立即尝试备用方向。",
                        runtime.email(),
                        super::log::localized_difficulty(&snapshot.difficulty),
                        snapshot.session_id,
                        super::log::localized_direction(direction.as_api_str()),
                    ));
                    let alt = valid_dirs.into_iter().find(|item| *item != direction);
                    if let Some(alt) = alt {
                        sleep_min_interval(cancel_flag, config.min_interval_ms)?;
                        match move_once(
                            cancel_flag,
                            state,
                            runtime,
                            &snapshot,
                            snapshot.session_id,
                            alt,
                            snapshot.move_count + 1,
                        ) {
                            Ok(alt_step) => {
                                if !alt_step.ok {
                                    let next_snapshot =
                                        snapshot_from_move_response(&snapshot, &alt_step);
                                    if is_finished(&next_snapshot) {
                                        return Ok(build_round_summary(
                                            runtime.email(),
                                            &next_snapshot,
                                            continued,
                                            &progress,
                                            started,
                                            alt_step.reward_amount,
                                            String::new(),
                                        ));
                                    }
                                    return fail_round_without_abandon(
                                        cancel_flag,
                                        runtime,
                                        &snapshot,
                                        RoundFailure::new(
                                            continued,
                                            &progress,
                                            started,
                                            "sim_drift",
                                        ),
                                    );
                                }
                                last_reward = alt_step.reward_amount;
                                snapshot = snapshot_from_move_response(&snapshot, &alt_step);
                                continue;
                            }
                            Err(error) => {
                                if error.kind() == io::ErrorKind::TimedOut {
                                    return Err(error);
                                }
                                consecutive_fail += 1;
                                if consecutive_fail < 3 {
                                    continue;
                                }
                                return fail_round_without_abandon(
                                    cancel_flag,
                                    runtime,
                                    &snapshot,
                                    RoundFailure::new(
                                        continued,
                                        &progress,
                                        started,
                                        &format!("stuck: {}", error),
                                    ),
                                );
                            }
                        }
                    }
                    return fail_round_without_abandon(
                        cancel_flag,
                        runtime,
                        &snapshot,
                        RoundFailure::new(continued, &progress, started, "sim_drift"),
                    );
                }
            }
            Err(error) => {
                if error.kind() == io::ErrorKind::TimedOut {
                    return Err(error);
                }
                if is_game_already_finished_error(&error.to_string()) {
                    return Ok(build_round_summary(
                        runtime.email(),
                        &snapshot,
                        continued,
                        &progress,
                        started,
                        last_reward,
                        error.to_string(),
                    ));
                }
                consecutive_fail += 1;
                if consecutive_fail >= 3 {
                    return fail_round_without_abandon(
                        cancel_flag,
                        runtime,
                        &snapshot,
                        RoundFailure::new(
                            continued,
                            &progress,
                            started,
                            &format!("stuck: {}", error),
                        ),
                    );
                }
                continue;
            }
        }
    }
}

fn move_once(
    cancel_flag: &ui::CancelFlag,
    state: &Arc<Mutex<BatchState>>,
    runtime: &mut AccountRuntime,
    previous: &PuzzleSnapshot,
    session_id: i32,
    direction: Direction,
    step_number: i32,
) -> io::Result<Puzzle2048MoveResponse> {
    let operation = retry_operation_with_step("puzzle2048 move", step_number);
    let previous = previous.clone();
    with_auth_retry_api_mutation_until_success(
        cancel_flag,
        state,
        runtime,
        &operation,
        |client, auth_token| {
            client.move_puzzle_2048(auth_token, session_id, direction.as_api_str())
        },
        |cancel_flag, state, runtime| {
            recover_progressed_session(cancel_flag, state, runtime, &previous)
                .map(|item| item.map(|item| response_from_history_item(&previous, item, direction)))
        },
        is_state_conflict_api_error,
    )
}

fn recover_progressed_session(
    cancel_flag: &ui::CancelFlag,
    state: &Arc<Mutex<BatchState>>,
    runtime: &mut AccountRuntime,
    previous: &PuzzleSnapshot,
) -> io::Result<Option<Puzzle2048HistoryItem>> {
    ui::sleep_with_cancel(cancel_flag, STATE_SYNC_RETRY_DELAY)?;
    let me = with_auth_retry_api_until_success(
        cancel_flag,
        state,
        runtime,
        "puzzle2048 me",
        |client, auth_token| client.get_puzzle_2048_me(auth_token),
    )?;
    if let Some(item) = me.active_session {
        if is_progressed_item(previous, &item) {
            return Ok(Some(item));
        }
        if item.session_id == previous.session_id {
            return Ok(None);
        }
    }
    let history = with_auth_retry_api_until_success(
        cancel_flag,
        state,
        runtime,
        "puzzle2048 history",
        |client, auth_token| client.get_puzzle_2048_history(auth_token),
    )?;
    Ok(history
        .items
        .into_iter()
        .find(|item| is_progressed_item(previous, item)))
}

fn is_progressed_item(previous: &PuzzleSnapshot, item: &Puzzle2048HistoryItem) -> bool {
    item.session_id == previous.session_id
        && (!is_pending_item(item)
            || item.move_count > previous.move_count
            || item.board != previous.board)
}

fn response_from_history_item(
    previous: &PuzzleSnapshot,
    item: Puzzle2048HistoryItem,
    direction: Direction,
) -> Puzzle2048MoveResponse {
    let won = item.won || item.max_tile >= item.target_tile && item.target_tile > 0;
    let status = if item.status.trim().is_empty() {
        if won { "won" } else { "pending" }.to_string()
    } else {
        item.status.clone()
    };
    Puzzle2048MoveResponse {
        board: item.board.clone(),
        changed: item.move_count > previous.move_count || item.board != previous.board,
        direction: direction.as_api_str().to_string(),
        ended_at_ms: item.ended_at_ms,
        game_over: item.game_over,
        max_tile: item.max_tile,
        move_count: item.move_count,
        ok: true,
        resolution: status.clone(),
        reward_amount: item.reward_amount,
        score: item.score,
        server_seed: item.server_seed.clone(),
        spawned: item.last_spawn.clone(),
        status,
        won,
        ..Puzzle2048MoveResponse::default()
    }
}

struct RoundFailure<'a> {
    continued: bool,
    progress: &'a RoundProgress,
    started: Instant,
    tag: &'a str,
}

impl<'a> RoundFailure<'a> {
    fn new(continued: bool, progress: &'a RoundProgress, started: Instant, tag: &'a str) -> Self {
        Self {
            continued,
            progress,
            started,
            tag,
        }
    }
}

fn fail_round_without_abandon(
    cancel_flag: &ui::CancelFlag,
    runtime: &mut AccountRuntime,
    snapshot: &PuzzleSnapshot,
    failure: RoundFailure<'_>,
) -> io::Result<PuzzleRoundSummary> {
    ui::check_cancel(cancel_flag)?;
    let mut failed_snapshot = snapshot.clone();
    if is_pending_round_status(&failed_snapshot.status) {
        failed_snapshot.status = "failed".to_string();
    }
    failed_snapshot.game_over = true;
    Ok(build_round_summary(
        runtime.email(),
        &failed_snapshot,
        failure.continued,
        failure.progress,
        failure.started,
        0.0,
        user_facing_round_failure_reason(failure.tag),
    ))
}

fn user_facing_round_failure_reason(tag: &str) -> String {
    if let Some(detail) = tag.strip_prefix("stuck:") {
        let detail = detail.trim();
        if detail.is_empty() {
            return "连续多次移动请求失败，这局按失败记录。".to_string();
        }
        return format!("连续多次移动请求失败，这局按失败记录：{}", detail);
    }
    match tag {
        "stuck" => "连续多次移动请求失败，这局按失败记录。".to_string(),
        "sim_drift" => "服务端棋盘和本地判断不一致，这局按失败记录。".to_string(),
        _ => tag.to_string(),
    }
}

pub(super) fn snapshot_from_start_response(start: &Puzzle2048StartResponse) -> PuzzleSnapshot {
    PuzzleSnapshot {
        board: start.board.clone(),
        difficulty: start.difficulty.clone(),
        game_over: false,
        max_tile: start.max_tile,
        move_count: start.move_count,
        score: start.score,
        session_id: start.session_id,
        size: start.size,
        status: "pending".to_string(),
        target_tile: start.target_tile,
        won: start.max_tile >= start.target_tile && start.target_tile > 0,
    }
}

pub(super) fn snapshot_from_history_item(item: &Puzzle2048HistoryItem) -> PuzzleSnapshot {
    PuzzleSnapshot {
        board: item.board.clone(),
        difficulty: item.difficulty.clone(),
        game_over: item.game_over,
        max_tile: item.max_tile,
        move_count: item.move_count,
        score: item.score,
        session_id: item.session_id,
        size: item.size,
        status: item.status.clone(),
        target_tile: item.target_tile,
        won: item.won,
    }
}

pub(super) fn merge_round_into_summary(
    summary: &mut PuzzleDifficultySummary,
    result: &PuzzleRoundSummary,
) {
    summary.played += 1;
    summary.total_reward += result.reward;
    summary.remaining_after = result.remaining_after;
    summary.when_unix_ms = result.when_unix_ms;
    if !result.error_message.trim().is_empty() {
        summary.failed += 1;
        summary.error_message = result.error_message.clone();
        return;
    }
    if result.status.trim().eq_ignore_ascii_case("won")
        || result.max_tile >= target_for_status(result)
    {
        summary.won += 1;
    } else if !is_pending_round_status(&result.status) {
        summary.failed += 1;
    }
}

pub(super) fn normalize_round_total(current: i32, total: i32) -> i32 {
    total.max(current.max(1))
}

fn snapshot_from_move_response(
    previous: &PuzzleSnapshot,
    response: &Puzzle2048MoveResponse,
) -> PuzzleSnapshot {
    PuzzleSnapshot {
        board: response.board.clone(),
        difficulty: previous.difficulty.clone(),
        game_over: response.game_over,
        max_tile: response.max_tile,
        move_count: response.move_count,
        score: response.score,
        session_id: previous.session_id,
        size: previous.size,
        status: if response.status.trim().is_empty() {
            response.resolution.clone()
        } else {
            response.status.clone()
        },
        target_tile: previous.target_tile,
        won: response.won,
    }
}

fn build_round_summary(
    email: &str,
    snapshot: &PuzzleSnapshot,
    continued: bool,
    progress: &RoundProgress,
    started: Instant,
    reward: f64,
    error_message: String,
) -> PuzzleRoundSummary {
    PuzzleRoundSummary {
        email: email.to_string(),
        difficulty: snapshot.difficulty.clone(),
        round_index: progress.current,
        round_total: progress.total,
        session_id: snapshot.session_id,
        continued,
        status: status_for_snapshot(snapshot),
        reward,
        remaining_after: 0,
        move_count: snapshot.move_count,
        max_tile: snapshot.max_tile,
        score: snapshot.score,
        duration_ms: started.elapsed().as_millis() as i64,
        when_unix_ms: current_unix_ms(),
        error_message,
    }
}

fn is_finished(snapshot: &PuzzleSnapshot) -> bool {
    snapshot.won
        || snapshot.max_tile >= snapshot.target_tile && snapshot.target_tile > 0
        || snapshot.game_over
        || matches!(
            snapshot.status.trim().to_ascii_lowercase().as_str(),
            "won" | "lost" | "failed" | "abandoned"
        )
}

fn status_for_snapshot(snapshot: &PuzzleSnapshot) -> String {
    if snapshot.won || snapshot.max_tile >= snapshot.target_tile && snapshot.target_tile > 0 {
        "won".to_string()
    } else if snapshot.game_over {
        "game_over".to_string()
    } else if snapshot.status.trim().is_empty() {
        "pending".to_string()
    } else {
        snapshot.status.clone()
    }
}

fn parse_direction(direction: &str) -> Option<Direction> {
    match direction.trim().to_ascii_lowercase().as_str() {
        "up" => Some(Direction::Up),
        "down" => Some(Direction::Down),
        "left" => Some(Direction::Left),
        "right" => Some(Direction::Right),
        _ => None,
    }
}

fn target_for_status(result: &PuzzleRoundSummary) -> i32 {
    match result.difficulty.trim().to_ascii_lowercase().as_str() {
        crate::model::PUZZLE_2048_DIFFICULTY_MINI => 512,
        crate::model::PUZZLE_2048_DIFFICULTY_CLASSIC => 2048,
        crate::model::PUZZLE_2048_DIFFICULTY_JUMBO => 4096,
        _ => i32::MAX,
    }
}

fn is_game_already_finished_error(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    lower.contains("已经结束")
        || lower.contains("已结束")
        || lower.contains("game over")
        || lower.contains("already ended")
}

pub(super) fn is_daily_limit_error(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    lower.contains("次数已经用完")
        || lower.contains("次数已用完")
        || lower.contains("今日次数")
        || lower.contains("daily limit")
}

pub(super) fn is_active_session_error(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    lower.contains("未结束对局")
        || lower.contains("未结束的对局")
        || lower.contains("进行中")
        || lower.contains("active session")
        || lower.contains("max active")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Puzzle2048DifficultyConfig;

    #[test]
    fn pending_item_requires_active_status_and_board() {
        let item = Puzzle2048HistoryItem {
            session_id: 1,
            board: vec![vec![2, 0, 0], vec![0, 0, 0], vec![0, 0, 0]],
            status: "pending".to_string(),
            ..Puzzle2048HistoryItem::default()
        };
        let ended = Puzzle2048HistoryItem {
            status: "abandoned".to_string(),
            ..item.clone()
        };

        assert!(is_pending_item(&item));
        assert!(!is_pending_item(&ended));
    }

    #[test]
    fn difficulty_order_keeps_known_order_then_sorted_extras() {
        let mut config = Puzzle2048ConfigResponse::default();
        config
            .difficulties
            .insert("zzz".to_string(), Puzzle2048DifficultyConfig::default());
        config.difficulties.insert(
            crate::model::PUZZLE_2048_DIFFICULTY_JUMBO.to_string(),
            Puzzle2048DifficultyConfig::default(),
        );
        config.difficulties.insert(
            crate::model::PUZZLE_2048_DIFFICULTY_MINI.to_string(),
            Puzzle2048DifficultyConfig::default(),
        );

        assert_eq!(difficulty_order(&config), vec!["mini", "jumbo", "zzz"]);
    }

    #[test]
    fn pending_status_is_ignored_not_failed() {
        let mut summary = PuzzleDifficultySummary::default();
        let result = PuzzleRoundSummary {
            status: "pending".to_string(),
            max_tile: 128,
            difficulty: crate::model::PUZZLE_2048_DIFFICULTY_MINI.to_string(),
            ..PuzzleRoundSummary::default()
        };

        merge_round_into_summary(&mut summary, &result);

        assert_eq!(summary.won, 0);
        assert_eq!(summary.failed, 0);
    }
}
