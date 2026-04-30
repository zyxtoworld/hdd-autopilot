use std::collections::HashMap;
use std::io;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use crate::api::ApiError;
use crate::model::{
    AccountRunSummary, ConfigResponse, RoundResultSummary, SessionSnapshot, StartResponse,
    StepRequest, StepResponse,
};
use crate::ui;
use crate::workflows::common::{
    current_unix_ms, humanize_retryable_api_error, is_pending_round_status, is_retryable_api_error,
};

use super::auth::{with_auth_retry, with_auth_retry_api};
use super::snapshot::{
    fixed_click_queue, is_slot_full_error, is_solved, is_stale_click_error,
    snapshot_from_start_response, snapshot_from_step_response,
};
use super::{AccountRuntime, BatchState};

const STEP_RETRY_BACKOFF: std::time::Duration = std::time::Duration::from_millis(500);
const STEP_RETRY_LOG_EVERY: usize = 10;

#[derive(Debug, Clone, Copy)]
pub(super) struct RoundProgress {
    pub(super) current: i32,
    pub(super) total: i32,
}

pub(super) fn play_round(
    cancel_flag: &ui::CancelFlag,
    state: &Arc<Mutex<BatchState>>,
    runtime: &mut AccountRuntime,
    config: &ConfigResponse,
    start: &StartResponse,
    continued: bool,
    progress: RoundProgress,
) -> io::Result<RoundResultSummary> {
    let started = Instant::now();
    let mut snapshot = snapshot_from_start_response(start);
    let used_powerups = Vec::new();
    let click_queue = fixed_click_queue(&snapshot);

    if click_queue.is_empty() {
        if is_solved(&snapshot) {
            let remaining_after =
                remaining_plays(state, runtime, &snapshot.difficulty).unwrap_or_default();
            return Ok(RoundResultSummary {
                email: runtime.email().to_string(),
                difficulty: snapshot.difficulty.clone(),
                round_index: progress.current,
                round_total: progress.total,
                session_id: snapshot.session_id,
                continued,
                dry_run: false,
                status: "won".to_string(),
                reward: 0.0,
                balance_after: None,
                remaining_after,
                move_count: snapshot.move_count,
                used_powerups,
                duration_ms: started.elapsed().as_millis() as i64,
                when_unix_ms: current_unix_ms(),
                error_message: String::new(),
            });
        }
        return Ok(build_round_error(
            runtime.email(),
            &snapshot,
            continued,
            progress,
            &used_powerups,
            started,
            "当前对局没有可点击的牌，无法继续。",
        ));
    }

    for tile_id in click_queue {
        ui::check_cancel(cancel_flag)?;
        if is_solved(&snapshot) {
            let remaining_after =
                remaining_plays(state, runtime, &snapshot.difficulty).unwrap_or_default();
            return Ok(RoundResultSummary {
                email: runtime.email().to_string(),
                difficulty: snapshot.difficulty.clone(),
                round_index: progress.current,
                round_total: progress.total,
                session_id: snapshot.session_id,
                continued,
                dry_run: false,
                status: "won".to_string(),
                reward: 0.0,
                balance_after: None,
                remaining_after,
                move_count: snapshot.move_count,
                used_powerups,
                duration_ms: started.elapsed().as_millis() as i64,
                when_unix_ms: current_unix_ms(),
                error_message: String::new(),
            });
        }
        if config.min_interval_ms > 0 {
            ui::sleep_with_cancel(
                cancel_flag,
                std::time::Duration::from_millis(config.min_interval_ms as u64),
            )?;
        }
        let mut step_attempts = 0usize;
        let response = loop {
            ui::check_cancel(cancel_flag)?;
            step_attempts += 1;
            let response = with_auth_retry_api(state, runtime, |client, auth_token| {
                client.step(
                    auth_token,
                    StepRequest {
                        session_id: snapshot.session_id,
                        action: "click".to_string(),
                        tile_id,
                    },
                )
            });
            match response {
                Err(ApiError::HttpStatus {
                    status: 409,
                    message,
                }) if is_slot_full_error(&message) || is_stale_click_error(&message) => {
                    break Err(io::Error::other(message));
                }
                Err(ApiError::HttpStatus { .. }) => {
                    ui::sleep_with_cancel(cancel_flag, STEP_RETRY_BACKOFF)?;
                    continue;
                }
                Err(error) if is_retryable_step_transport_error(&error) => {
                    if step_attempts == 1 || step_attempts.is_multiple_of(STEP_RETRY_LOG_EVERY) {
                        state.lock().unwrap().log.line_fmt(format_args!(
                            "账号 {} 的羊了个羊第 {} 步请求暂时失败，继续等待接口返回成功后再推进：{}",
                            runtime.email(),
                            snapshot.move_count + 1,
                            humanize_retryable_api_error(&error)
                        ));
                    }
                    ui::sleep_with_cancel(cancel_flag, STEP_RETRY_BACKOFF)?;
                    continue;
                }
                other => break other.map_err(|error| io::Error::other(error.to_string())),
            }
        };
        match response {
            Ok(step) => {
                let email = runtime.email().to_string();
                let remaining_after =
                    remaining_plays(state, runtime, &snapshot.difficulty).unwrap_or(0);
                let result = build_round_from_step(
                    &email,
                    &snapshot,
                    &step,
                    StepRoundContext {
                        continued,
                        progress,
                        used_powerups: &used_powerups,
                        started,
                        remaining_after,
                    },
                );
                snapshot = snapshot_from_step_response(&snapshot, &step);
                if is_terminal_round_status(&step.status) {
                    return Ok(result);
                }
            }
            Err(error) => {
                if is_slot_full_error(&error.to_string()) {
                    return Ok(build_round_error(
                        runtime.email(),
                        &snapshot,
                        continued,
                        progress,
                        &used_powerups,
                        started,
                        &error.to_string(),
                    ));
                }
                if is_stale_click_error(&error.to_string()) {
                    continue;
                }
                return Ok(build_round_error(
                    runtime.email(),
                    &snapshot,
                    continued,
                    progress,
                    &used_powerups,
                    started,
                    &error.to_string(),
                ));
            }
        }
    }

    if is_solved(&snapshot) {
        let remaining_after =
            remaining_plays(state, runtime, &snapshot.difficulty).unwrap_or_default();
        return Ok(RoundResultSummary {
            email: runtime.email().to_string(),
            difficulty: snapshot.difficulty.clone(),
            round_index: progress.current,
            round_total: progress.total,
            session_id: snapshot.session_id,
            continued,
            dry_run: false,
            status: snapshot.status.clone(),
            reward: 0.0,
            balance_after: None,
            remaining_after,
            move_count: snapshot.move_count,
            used_powerups,
            duration_ms: started.elapsed().as_millis() as i64,
            when_unix_ms: current_unix_ms(),
            error_message: String::new(),
        });
    }

    Ok(build_round_error(
        runtime.email(),
        &snapshot,
        continued,
        progress,
        &used_powerups,
        started,
        "这一局的可点击步骤已经用完，但服务端仍显示未通关。",
    ))
}

fn is_retryable_step_transport_error(error: &ApiError) -> bool {
    is_retryable_api_error(error)
}

fn is_terminal_round_status(status: &str) -> bool {
    matches!(
        status.trim().to_ascii_lowercase().as_str(),
        "won" | "lost" | "failed" | "game_over" | "abandoned"
    )
}

pub(super) fn next_round_index_for_new_round(used_today: i32) -> i32 {
    used_today.max(0) + 1
}

pub(super) fn total_round_count(used_today: i32, remaining: i32) -> i32 {
    (used_today + remaining).max(0)
}

pub(super) fn normalize_round_total(current: i32, total: i32) -> i32 {
    total.max(current.max(1))
}

pub(super) fn remaining_plays(
    state: &Arc<Mutex<BatchState>>,
    runtime: &mut AccountRuntime,
    difficulty: &str,
) -> io::Result<i32> {
    let me = with_auth_retry(state, runtime, |client, auth_token| {
        client.get_tile_me(auth_token)
    })?;
    Ok(*me.daily_plays_remaining.get(difficulty).unwrap_or(&0))
}

pub(super) fn summarize_rounds_by_difficulty(
    email: &str,
    rounds: &[RoundResultSummary],
) -> HashMap<String, AccountRunSummary> {
    let mut stats = HashMap::new();
    for round in rounds {
        let entry = stats
            .entry(round.difficulty.clone())
            .or_insert_with(|| AccountRunSummary {
                email: email.to_string(),
                difficulty: round.difficulty.clone(),
                ..AccountRunSummary::default()
            });
        merge_round_into_summary(entry, round);
    }
    stats
}

struct StepRoundContext<'a> {
    continued: bool,
    progress: RoundProgress,
    used_powerups: &'a [String],
    started: Instant,
    remaining_after: i32,
}

fn build_round_from_step(
    email: &str,
    snapshot: &SessionSnapshot,
    step: &StepResponse,
    context: StepRoundContext<'_>,
) -> RoundResultSummary {
    RoundResultSummary {
        email: email.to_string(),
        difficulty: snapshot.difficulty.clone(),
        round_index: context.progress.current,
        round_total: context.progress.total,
        session_id: snapshot.session_id,
        continued: context.continued,
        dry_run: false,
        status: step.status.clone(),
        reward: step.reward_amount,
        balance_after: Some(step.balance),
        remaining_after: context.remaining_after,
        move_count: step.move_count,
        used_powerups: context.used_powerups.to_vec(),
        duration_ms: context.started.elapsed().as_millis() as i64,
        when_unix_ms: current_unix_ms(),
        error_message: String::new(),
    }
}

fn build_round_error(
    email: &str,
    snapshot: &SessionSnapshot,
    continued: bool,
    progress: RoundProgress,
    used_powerups: &[String],
    started: Instant,
    error_message: &str,
) -> RoundResultSummary {
    RoundResultSummary {
        email: email.to_string(),
        difficulty: snapshot.difficulty.clone(),
        round_index: progress.current,
        round_total: progress.total,
        session_id: snapshot.session_id,
        continued,
        dry_run: false,
        status: snapshot.status.clone(),
        reward: 0.0,
        balance_after: None,
        remaining_after: 0,
        move_count: snapshot.move_count,
        used_powerups: used_powerups.to_vec(),
        duration_ms: started.elapsed().as_millis() as i64,
        when_unix_ms: current_unix_ms(),
        error_message: error_message.to_string(),
    }
}

pub(super) fn merge_round_into_summary(
    summary: &mut AccountRunSummary,
    result: &RoundResultSummary,
) {
    summary.played += 1;
    summary.total_reward += result.reward;
    summary.remaining_after = result.remaining_after;
    summary.when_unix_ms = result.when_unix_ms;
    if let Some(balance) = result.balance_after {
        summary.balance_after = Some(balance);
    }
    if !result.error_message.trim().is_empty() {
        summary.failed += 1;
        summary.error_message = result.error_message.clone();
        return;
    }
    let status = result.status.trim().to_ascii_lowercase();
    match status.as_str() {
        "won" => summary.won += 1,
        "abandoned" => summary.abandoned += 1,
        _ if is_pending_round_status(&status) => {}
        _ => summary.failed += 1,
    }
}
