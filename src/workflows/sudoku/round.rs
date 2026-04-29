use std::collections::{HashMap, HashSet};
use std::io;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use crate::model::{
    SudokuConfigResponse, SudokuFillResponse, SudokuHistoryResponse, SudokuSession,
    SudokuStartResponse,
};
use crate::solver::sudoku;
use crate::ui;

use super::auth::with_auth_retry_api;
use super::types::{RoundProgress, SudokuDifficultySummary, SudokuRoundSummary, SudokuSnapshot};
use super::{AccountRuntime, BatchState, current_unix_ms};

pub(super) fn difficulty_order(config: &SudokuConfigResponse) -> Vec<String> {
    let mut ordered = Vec::new();
    for difficulty in crate::model::SUDOKU_DIFFICULTY_ORDER {
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

pub(super) fn is_pending_session(session: &SudokuSession) -> bool {
    if session.session_id <= 0
        || session.givens.is_empty()
        || session.user_board.is_empty()
        || session.won
    {
        return false;
    }
    let status = session.status.trim().to_ascii_lowercase();
    status.is_empty() || matches!(status.as_str(), "pending" | "running" | "active")
}

pub(super) fn started_today(started_at_ms: i64, server_now_ms: i64) -> bool {
    if started_at_ms <= 0 || server_now_ms <= 0 {
        return false;
    }
    started_at_ms.div_euclid(86_400_000) == server_now_ms.div_euclid(86_400_000)
}

pub(super) fn used_today_by_difficulty(history: &SudokuHistoryResponse) -> HashMap<String, i32> {
    let mut used = HashMap::new();
    for item in &history.items {
        if item.difficulty.trim().is_empty()
            || !started_today(item.started_at_ms, history.server_now_ms)
        {
            continue;
        }
        *used.entry(item.difficulty.clone()).or_insert(0) += 1;
    }
    used
}

pub(super) fn remaining_for_difficulty(
    config: &SudokuConfigResponse,
    difficulty: &str,
    used_today: i32,
) -> i32 {
    config
        .difficulties
        .get(difficulty)
        .map(|item| (item.daily_plays - used_today).max(0))
        .unwrap_or(0)
}

pub(super) fn normalize_round_total(current: i32, total: i32) -> i32 {
    total.max(current.max(1))
}

pub(super) fn play_round(
    cancel_flag: &ui::CancelFlag,
    state: &Arc<Mutex<BatchState>>,
    runtime: &mut AccountRuntime,
    start: SudokuSnapshot,
    continued: bool,
    progress: RoundProgress,
) -> io::Result<SudokuRoundSummary> {
    let started = Instant::now();
    let mut snapshot = start;
    let fills = match sudoku::solve_fills(
        &snapshot.givens,
        &snapshot.user_board,
        snapshot.size,
        snapshot.box_size,
    ) {
        Ok(fills) => fills,
        Err(error) => {
            return Ok(build_round_summary(
                runtime.email(),
                &snapshot,
                continued,
                &progress,
                started,
                0,
                0,
                format!("求解失败：{}", error),
            ));
        }
    };
    let initial_wrong_fills = match wrong_editable_fills(&snapshot) {
        Ok(fills) => fills,
        Err(error) => {
            return Ok(build_round_summary(
                runtime.email(),
                &snapshot,
                continued,
                &progress,
                started,
                0,
                0,
                format!("求解失败：{}", error),
            ));
        }
    };
    let planned_fills = fills.len().min(i32::MAX as usize) as i32;
    let mut filled_cells = 0;
    let mut latest_conflict_count = 0;

    for fill in fills {
        ui::check_cancel(cancel_flag)?;
        let step = fill_once(
            state,
            runtime,
            snapshot.session_id,
            fill.row,
            fill.col,
            Some(fill.value),
        )?;
        if !step.ok {
            return Ok(build_round_summary(
                runtime.email(),
                &snapshot,
                continued,
                &progress,
                started,
                planned_fills,
                filled_cells,
                "填数接口返回 ok=false".to_string(),
            ));
        }
        latest_conflict_count = conflict_count(&step);
        snapshot = snapshot_from_fill_response(&snapshot, &step);
        filled_cells += 1;
    }

    let mut error_message = if snapshot.won || snapshot.complete {
        String::new()
    } else if latest_conflict_count > 0 {
        format!("填完后仍有 {} 个冲突位置", latest_conflict_count)
    } else {
        "填完后服务端仍未结算通关".to_string()
    };
    if !error_message.is_empty() && !initial_wrong_fills.is_empty() {
        match clear_and_refill_wrong_cells(
            state,
            runtime,
            &mut snapshot,
            &initial_wrong_fills,
            &mut filled_cells,
        ) {
            Ok(conflict_count) => {
                latest_conflict_count = conflict_count;
                error_message = if snapshot.won || snapshot.complete {
                    String::new()
                } else if latest_conflict_count > 0 {
                    format!("清理重填后仍有 {} 个冲突位置", latest_conflict_count)
                } else {
                    "清理重填后服务端仍未结算通关".to_string()
                };
            }
            Err(error) => {
                error_message = format!("清理重填失败：{}", error);
            }
        }
    }

    Ok(build_round_summary(
        runtime.email(),
        &snapshot,
        continued,
        &progress,
        started,
        planned_fills,
        filled_cells,
        error_message,
    ))
}

fn fill_once(
    state: &Arc<Mutex<BatchState>>,
    runtime: &mut AccountRuntime,
    session_id: i32,
    row: i32,
    col: i32,
    value: Option<i32>,
) -> io::Result<SudokuFillResponse> {
    with_auth_retry_api(state, runtime, |client, auth_token| {
        client.fill_sudoku(auth_token, session_id, row, col, value)
    })
    .map_err(|error| io::Error::other(error.to_string()))
}

pub(super) fn snapshot_from_start_response(
    start: &SudokuStartResponse,
    config: &SudokuConfigResponse,
) -> SudokuSnapshot {
    SudokuSnapshot {
        givens: start.givens.clone(),
        user_board: start.user_board.clone(),
        difficulty: start.difficulty.clone(),
        session_id: start.session_id,
        size: config.size,
        box_size: config.box_size,
        move_count: start.move_count,
        status: "pending".to_string(),
        reward_amount: 0.0,
        ..SudokuSnapshot::default()
    }
}

pub(super) fn snapshot_from_history_item(
    item: &SudokuSession,
    config: &SudokuConfigResponse,
) -> SudokuSnapshot {
    SudokuSnapshot {
        givens: item.givens.clone(),
        user_board: item.user_board.clone(),
        difficulty: item.difficulty.clone(),
        session_id: item.session_id,
        size: config.size,
        box_size: config.box_size,
        move_count: item.move_count,
        status: item.status.clone(),
        won: item.won,
        complete: item.won,
        reward_amount: item.reward_amount,
    }
}

fn snapshot_from_fill_response(
    previous: &SudokuSnapshot,
    response: &SudokuFillResponse,
) -> SudokuSnapshot {
    let session = &response.session;
    let has_session = session.session_id > 0;
    SudokuSnapshot {
        givens: if has_session && !session.givens.is_empty() {
            session.givens.clone()
        } else {
            previous.givens.clone()
        },
        user_board: if has_session && !session.user_board.is_empty() {
            session.user_board.clone()
        } else if !response.user_board.is_empty() {
            response.user_board.clone()
        } else {
            previous.user_board.clone()
        },
        difficulty: if has_session && !session.difficulty.trim().is_empty() {
            session.difficulty.clone()
        } else {
            previous.difficulty.clone()
        },
        session_id: if has_session {
            session.session_id
        } else {
            previous.session_id
        },
        size: previous.size,
        box_size: previous.box_size,
        move_count: prefer_i32(
            response.move_count,
            prefer_i32(session.move_count, previous.move_count),
        ),
        status: first_non_empty(&[
            response.status.as_str(),
            session.status.as_str(),
            response.resolution.as_str(),
            previous.status.as_str(),
        ]),
        won: response.won || session.won,
        complete: response.complete || response.won || session.won,
        reward_amount: prefer_f64(response.reward_amount, session.reward_amount),
    }
}

pub(super) fn merge_round_into_summary(
    summary: &mut SudokuDifficultySummary,
    result: &SudokuRoundSummary,
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
    if result.status.trim().eq_ignore_ascii_case("won") {
        summary.won += 1;
    } else {
        summary.failed += 1;
    }
}

fn build_round_summary(
    email: &str,
    snapshot: &SudokuSnapshot,
    continued: bool,
    progress: &RoundProgress,
    started: Instant,
    planned_fills: i32,
    filled_cells: i32,
    error_message: String,
) -> SudokuRoundSummary {
    SudokuRoundSummary {
        email: email.to_string(),
        difficulty: snapshot.difficulty.clone(),
        round_index: progress.current,
        round_total: progress.total,
        session_id: snapshot.session_id,
        continued,
        status: status_for_snapshot(snapshot),
        reward: snapshot.reward_amount,
        remaining_after: 0,
        move_count: snapshot.move_count,
        planned_fills,
        filled_cells,
        size: snapshot.size,
        box_size: snapshot.box_size,
        duration_ms: started.elapsed().as_millis() as i64,
        when_unix_ms: current_unix_ms(),
        error_message,
    }
}

fn status_for_snapshot(snapshot: &SudokuSnapshot) -> String {
    if snapshot.won || snapshot.complete {
        "won".to_string()
    } else if snapshot.status.trim().is_empty() {
        "pending".to_string()
    } else {
        snapshot.status.clone()
    }
}

fn prefer_i32(value: i32, fallback: i32) -> i32 {
    if value != 0 { value } else { fallback }
}

fn prefer_f64(value: f64, fallback: f64) -> f64 {
    if value != 0.0 { value } else { fallback }
}

fn first_non_empty(parts: &[&str]) -> String {
    parts
        .iter()
        .map(|part| part.trim())
        .find(|part| !part.is_empty())
        .unwrap_or("")
        .to_string()
}

fn wrong_editable_fills(snapshot: &SudokuSnapshot) -> Result<Vec<sudoku::CellFill>, String> {
    let solution = sudoku::solve(&snapshot.givens, snapshot.size, snapshot.box_size)?;
    let size = usize::try_from(snapshot.size).map_err(|_| "数独尺寸无效".to_string())?;
    let mut fills = Vec::new();
    for index in 0..solution.len() {
        if snapshot.givens[index] != 0 {
            continue;
        }
        let current = snapshot.user_board[index];
        if current == 0 || current == solution[index] {
            continue;
        }
        fills.push(sudoku::CellFill {
            row: (index / size) as i32,
            col: (index % size) as i32,
            value: solution[index],
        });
    }
    Ok(fills)
}

fn clear_and_refill_wrong_cells(
    state: &Arc<Mutex<BatchState>>,
    runtime: &mut AccountRuntime,
    snapshot: &mut SudokuSnapshot,
    wrong_fills: &[sudoku::CellFill],
    filled_cells: &mut i32,
) -> io::Result<usize> {
    let mut latest_conflict_count = 0;
    for fill in wrong_fills {
        let step = fill_once(
            state,
            runtime,
            snapshot.session_id,
            fill.row,
            fill.col,
            None,
        )?;
        if !step.ok {
            return Err(io::Error::other("清空错误格时接口返回 ok=false"));
        }
        latest_conflict_count = conflict_count(&step);
        *snapshot = snapshot_from_fill_response(snapshot, &step);
    }

    let fills = sudoku::solve_fills(
        &snapshot.givens,
        &snapshot.user_board,
        snapshot.size,
        snapshot.box_size,
    )
    .map_err(|error| io::Error::other(format!("重新求解失败：{}", error)))?;
    for fill in fills {
        let step = fill_once(
            state,
            runtime,
            snapshot.session_id,
            fill.row,
            fill.col,
            Some(fill.value),
        )?;
        if !step.ok {
            return Err(io::Error::other("重填正确值时接口返回 ok=false"));
        }
        latest_conflict_count = conflict_count(&step);
        *snapshot = snapshot_from_fill_response(snapshot, &step);
        *filled_cells += 1;
    }
    Ok(latest_conflict_count)
}

fn conflict_count(response: &SudokuFillResponse) -> usize {
    response
        .conflicts
        .len()
        .max(response.session.conflicts.len())
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
    use crate::model::SudokuDifficultyConfig;

    #[test]
    fn pending_session_requires_active_status_and_boards() {
        let session = SudokuSession {
            session_id: 1,
            givens: vec![1, 0, 0, 0],
            user_board: vec![1, 0, 0, 0],
            status: "pending".to_string(),
            ..SudokuSession::default()
        };
        let ended = SudokuSession {
            status: "won".to_string(),
            won: true,
            ..session.clone()
        };

        assert!(is_pending_session(&session));
        assert!(!is_pending_session(&ended));
    }

    #[test]
    fn difficulty_order_keeps_known_order_then_sorted_extras() {
        let mut config = SudokuConfigResponse::default();
        config
            .difficulties
            .insert("zzz".to_string(), SudokuDifficultyConfig::default());
        config.difficulties.insert(
            crate::model::SUDOKU_DIFFICULTY_HARD.to_string(),
            SudokuDifficultyConfig::default(),
        );
        config.difficulties.insert(
            crate::model::SUDOKU_DIFFICULTY_EASY.to_string(),
            SudokuDifficultyConfig::default(),
        );

        assert_eq!(difficulty_order(&config), vec!["easy", "hard", "zzz"]);
    }

    #[test]
    fn used_today_counts_only_current_day() {
        let history = SudokuHistoryResponse {
            server_now_ms: 86_400_000 * 10 + 100,
            items: vec![
                SudokuSession {
                    difficulty: "easy".to_string(),
                    started_at_ms: 86_400_000 * 10 + 50,
                    ..SudokuSession::default()
                },
                SudokuSession {
                    difficulty: "easy".to_string(),
                    started_at_ms: 86_400_000 * 9 + 50,
                    ..SudokuSession::default()
                },
            ],
        };

        assert_eq!(used_today_by_difficulty(&history)["easy"], 1);
    }
}
