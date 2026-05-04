use std::io;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use crate::model::{
    MinesweeperClickResponse, MinesweeperConfigResponse, MinesweeperSession,
    MinesweeperStartResponse,
};
use crate::solver::minesweeper::{self, Board, Cell};
use crate::ui;
use crate::workflows::common::{
    AccountRuntime, BatchState, current_unix_ms, is_pending_round_status,
    retry_operation_with_step, with_auth_retry_api_until_success,
};

use super::log::localized_difficulty;
use super::types::RoundProgress;
use super::types::{MinesweeperDifficultySummary, MinesweeperRoundSummary, MinesweeperSnapshot};

pub(super) fn difficulty_order(config: &MinesweeperConfigResponse) -> Vec<String> {
    let mut ordered = config.difficulties.keys().cloned().collect::<Vec<_>>();
    ordered.sort_by(|left, right| {
        let left_reward = config.rewards.get(left).copied().unwrap_or(0.0);
        let right_reward = config.rewards.get(right).copied().unwrap_or(0.0);
        right_reward
            .partial_cmp(&left_reward)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| known_difficulty_priority(left).cmp(&known_difficulty_priority(right)))
            .then_with(|| left.cmp(right))
    });
    ordered
}

fn known_difficulty_priority(difficulty: &str) -> usize {
    crate::model::MINESWEEPER_DIFFICULTY_ORDER
        .iter()
        .position(|item| item.eq_ignore_ascii_case(difficulty))
        .unwrap_or(usize::MAX)
}

pub(super) fn is_pending_session(session: &MinesweeperSession) -> bool {
    if session.play_id <= 0 || session.ended_at_ms.is_some() {
        return false;
    }
    let status = session.status.trim().to_ascii_lowercase();
    let resolution = session.resolution.trim().to_ascii_lowercase();
    (status.is_empty() || is_pending_round_status(&status))
        && (resolution.is_empty() || is_pending_round_status(&resolution))
}

#[derive(Debug, Clone)]
pub(super) struct RoundPlayContext {
    pub(super) continued: bool,
    pub(super) progress: RoundProgress,
    pub(super) remaining_after: i32,
}

pub(super) fn play_round(
    cancel_flag: &ui::CancelFlag,
    state: &Arc<Mutex<BatchState>>,
    runtime: &mut AccountRuntime,
    config: &MinesweeperConfigResponse,
    start: MinesweeperSnapshot,
    context: RoundPlayContext,
) -> io::Result<MinesweeperRoundSummary> {
    let started = Instant::now();
    let RoundPlayContext {
        continued,
        progress,
        remaining_after,
    } = context;
    let mut snapshot = start;
    let max_steps = (snapshot.rows * snapshot.cols * 3).max(32);
    let mut executed_moves = 0;
    let mut safe_reveals = 0;
    let mut flags = 0;
    let mut chords = 0;
    let mut guesses = 0;
    let mut error_message = String::new();

    if snapshot.board.revealed_count() > snapshot.board.known_number_count()
        && snapshot.board.known_number_count() == 0
    {
        state.lock().unwrap().log.line_fmt(format_args!(
            "账号 {} 的扫雷{}难度旧残局没有返回已翻开的数字，只能按全局雷率继续；程序新开的局会完整使用数字求解。",
            runtime.email(),
            localized_difficulty(&snapshot.difficulty),
        ));
    }

    loop {
        ui::check_cancel(cancel_flag)?;
        if is_finished(&snapshot) {
            break;
        }
        if executed_moves >= max_steps {
            error_message = format!("超过最大操作步数 {}，停止本局防止异常循环", max_steps);
            break;
        }
        let Some(decision) = minesweeper::next_move(&snapshot.board) else {
            error_message = "求解器没有可执行的下一步，当前局保持未结算".to_string();
            break;
        };
        if config.min_interval_ms > 0 {
            ui::sleep_with_cancel(
                cancel_flag,
                std::time::Duration::from_millis(config.min_interval_ms as u64),
            )?;
        }
        let response = click_once(
            cancel_flag,
            state,
            runtime,
            MinesweeperClickAttempt {
                play_id: snapshot.play_id,
                action: decision.action,
                x: decision.x,
                y: decision.y,
                step_number: snapshot.trace_count + 1,
            },
        )?;
        executed_moves += 1;
        match decision.action {
            "flag" | "unflag" => flags += 1,
            "chord" => chords += 1,
            _ => {
                safe_reveals += 1;
                if decision.risk > 0.0 {
                    guesses += 1;
                }
            }
        }
        if !response.ok {
            error_message = "扫雷点击接口返回 ok=false".to_string();
            break;
        }
        snapshot = snapshot_from_click_response(&snapshot, &response)
            .map_err(|error| io::Error::other(format!("扫雷点击返回数据无效：{}", error)))?;
    }

    Ok(build_round_summary(
        runtime.email(),
        &snapshot,
        MinesweeperRoundBuildContext {
            continued,
            progress: &progress,
            remaining_after,
            started,
            executed_moves,
            safe_reveals,
            flags,
            chords,
            guesses,
            error_message,
        },
    ))
}

struct MinesweeperClickAttempt {
    play_id: i32,
    action: &'static str,
    x: i32,
    y: i32,
    step_number: i32,
}

fn click_once(
    cancel_flag: &ui::CancelFlag,
    state: &Arc<Mutex<BatchState>>,
    runtime: &mut AccountRuntime,
    attempt: MinesweeperClickAttempt,
) -> io::Result<MinesweeperClickResponse> {
    let operation = retry_operation_with_step("minesweeper click", attempt.step_number);
    with_auth_retry_api_until_success(
        cancel_flag,
        state,
        runtime,
        &operation,
        |client, auth_token| {
            client.click_minesweeper(
                auth_token,
                attempt.play_id,
                attempt.action,
                attempt.x,
                attempt.y,
            )
        },
    )
}

pub(super) fn snapshot_from_start_response(
    start: &MinesweeperStartResponse,
) -> Result<MinesweeperSnapshot, String> {
    snapshot_from_session(&start.session)
}

pub(super) fn snapshot_from_session(
    session: &MinesweeperSession,
) -> Result<MinesweeperSnapshot, String> {
    let board = Board::new(
        session.rows,
        session.cols,
        session.mine_count,
        &session.revealed,
        &session.flagged,
    )?;
    Ok(MinesweeperSnapshot {
        difficulty: session.difficulty.clone(),
        play_id: session.play_id,
        rows: session.rows,
        cols: session.cols,
        mine_count: session.mine_count,
        status: status_from_session(session),
        resolution: session.resolution.clone(),
        reward_amount: session.reward_amount,
        trace_count: session.trace_count,
        board,
    })
}

fn snapshot_from_click_response(
    previous: &MinesweeperSnapshot,
    response: &MinesweeperClickResponse,
) -> Result<MinesweeperSnapshot, String> {
    let session = &response.session;
    let mut board = previous.board.clone();
    if !session.revealed.is_empty() && !session.flagged.is_empty() {
        board.sync_masks(&session.revealed, &session.flagged)?;
    }
    apply_flagged_cells(&mut board, &response.delta.flagged_cells)?;
    board.apply_revealed_cells(&response.delta.revealed_cells)?;
    Ok(MinesweeperSnapshot {
        difficulty: first_non_empty(&[session.difficulty.as_str(), previous.difficulty.as_str()]),
        play_id: prefer_i32(session.play_id, previous.play_id),
        rows: prefer_i32(session.rows, previous.rows),
        cols: prefer_i32(session.cols, previous.cols),
        mine_count: prefer_i32(session.mine_count, previous.mine_count),
        status: status_from_click(previous, response),
        resolution: first_non_empty(&[
            session.resolution.as_str(),
            response.session.status.as_str(),
            previous.resolution.as_str(),
        ]),
        reward_amount: prefer_f64(session.reward_amount, previous.reward_amount),
        trace_count: prefer_i32(session.trace_count, previous.trace_count + 1),
        board,
    })
}

fn apply_flagged_cells(board: &mut Board, cells: &[serde_json::Value]) -> Result<(), String> {
    for value in cells {
        let Some(items) = value.as_array() else {
            continue;
        };
        if items.len() < 3 {
            continue;
        }
        let Some(row) = items[0]
            .as_i64()
            .and_then(|value| i32::try_from(value).ok())
        else {
            continue;
        };
        let Some(col) = items[1]
            .as_i64()
            .and_then(|value| i32::try_from(value).ok())
        else {
            continue;
        };
        let Some(flagged) = items[2].as_bool() else {
            continue;
        };
        board.apply_flag(Cell { row, col }, flagged)?;
    }
    Ok(())
}

pub(super) fn merge_round_into_summary(
    summary: &mut MinesweeperDifficultySummary,
    result: &MinesweeperRoundSummary,
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
    match result.status.trim().to_ascii_lowercase().as_str() {
        "won" => summary.won += 1,
        "lost" | "failed" | "game_over" => summary.lost += 1,
        _ if is_pending_round_status(&result.status) => {}
        _ => summary.failed += 1,
    }
}

fn build_round_summary(
    email: &str,
    snapshot: &MinesweeperSnapshot,
    context: MinesweeperRoundBuildContext<'_>,
) -> MinesweeperRoundSummary {
    MinesweeperRoundSummary {
        email: email.to_string(),
        difficulty: snapshot.difficulty.clone(),
        round_index: context.progress.current,
        round_total: context.progress.total,
        play_id: snapshot.play_id,
        continued: context.continued,
        status: status_for_snapshot(snapshot),
        reward: snapshot.reward_amount,
        remaining_after: context.remaining_after,
        rows: snapshot.rows,
        cols: snapshot.cols,
        mine_count: snapshot.mine_count,
        executed_moves: context.executed_moves,
        safe_reveals: context.safe_reveals,
        flags: context.flags,
        chords: context.chords,
        guesses: context.guesses,
        duration_ms: context.started.elapsed().as_millis() as i64,
        when_unix_ms: current_unix_ms(),
        error_message: context.error_message,
    }
}

struct MinesweeperRoundBuildContext<'a> {
    continued: bool,
    progress: &'a RoundProgress,
    remaining_after: i32,
    started: Instant,
    executed_moves: i32,
    safe_reveals: i32,
    flags: i32,
    chords: i32,
    guesses: i32,
    error_message: String,
}

fn status_from_session(session: &MinesweeperSession) -> String {
    first_non_empty(&[
        session.status.as_str(),
        session.resolution.as_str(),
        "pending",
    ])
}

fn status_from_click(
    previous: &MinesweeperSnapshot,
    response: &MinesweeperClickResponse,
) -> String {
    if response.delta.won {
        "won".to_string()
    } else if response.delta.lost || response.delta.hit_mine {
        "lost".to_string()
    } else {
        first_non_empty(&[
            response.session.status.as_str(),
            response.session.resolution.as_str(),
            previous.status.as_str(),
            "pending",
        ])
    }
}

fn status_for_snapshot(snapshot: &MinesweeperSnapshot) -> String {
    let status = first_non_empty(&[
        snapshot.status.as_str(),
        snapshot.resolution.as_str(),
        "pending",
    ]);
    if status.trim().eq_ignore_ascii_case("pending") && snapshot.board.complete_by_masks() {
        "won".to_string()
    } else {
        status
    }
}

fn is_finished(snapshot: &MinesweeperSnapshot) -> bool {
    snapshot.board.complete_by_masks() || is_terminal_status(&snapshot.status)
}

fn is_terminal_status(status: &str) -> bool {
    matches!(
        status.trim().to_ascii_lowercase().as_str(),
        "won" | "lost" | "failed" | "game_over" | "abandoned"
    )
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

pub(super) fn is_daily_limit_error(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    lower.contains("次数已经用完")
        || lower.contains("次数已用完")
        || lower.contains("今日次数")
        || lower.contains("daily limit")
        || lower.contains("no remaining plays")
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
    use crate::model::MinesweeperDifficultyConfig;

    #[test]
    fn difficulty_order_prefers_high_reward_known_order() {
        let mut config = MinesweeperConfigResponse::default();
        for difficulty in ["beginner", "expert", "intermediate"] {
            config.difficulties.insert(
                difficulty.to_string(),
                MinesweeperDifficultyConfig::default(),
            );
        }

        assert_eq!(
            difficulty_order(&config),
            vec!["expert", "intermediate", "beginner"]
        );
    }

    #[test]
    fn difficulty_order_uses_config_rewards_before_known_order() {
        let mut config = MinesweeperConfigResponse::default();
        for difficulty in ["beginner", "expert", "intermediate"] {
            config.difficulties.insert(
                difficulty.to_string(),
                MinesweeperDifficultyConfig::default(),
            );
        }
        config.rewards.insert("beginner".to_string(), 10.0);
        config.rewards.insert("expert".to_string(), 5.0);
        config.rewards.insert("intermediate".to_string(), 2.0);

        assert_eq!(
            difficulty_order(&config),
            vec!["beginner", "expert", "intermediate"]
        );
    }

    #[test]
    fn pending_session_requires_unfinished_status() {
        let pending = MinesweeperSession {
            play_id: 1,
            status: "pending".to_string(),
            ..MinesweeperSession::default()
        };
        let ended = MinesweeperSession {
            status: "won".to_string(),
            ended_at_ms: Some(1),
            ..pending.clone()
        };

        assert!(is_pending_session(&pending));
        assert!(!is_pending_session(&ended));
    }

    #[test]
    fn flagged_delta_updates_board_when_masks_are_sparse() {
        let empty = vec![vec![false; 2]; 2];
        let mut board = Board::new(2, 2, 1, &empty, &empty).unwrap();
        let cells = vec![serde_json::json!([1, 0, true])];

        apply_flagged_cells(&mut board, &cells).unwrap();

        assert!(board.is_flagged(Cell { row: 1, col: 0 }));
    }
}
