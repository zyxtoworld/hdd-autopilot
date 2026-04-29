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

use super::auth::{with_auth_retry, with_auth_retry_api};
use super::snapshot::{
    fixed_click_queue, is_slot_full_error, is_solved, is_stale_click_error,
    snapshot_from_start_response, snapshot_from_step_response,
};
use super::{AccountRuntime, BatchState, current_unix_ms};

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
            "固定 ID 队列为空，无法继续当前对局",
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
        let response = loop {
            ui::check_cancel(cancel_flag)?;
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
                    ui::sleep_with_cancel(cancel_flag, std::time::Duration::from_millis(500))?;
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
                    continued,
                    progress,
                    &used_powerups,
                    started,
                    &snapshot,
                    &step,
                    remaining_after,
                );
                snapshot = snapshot_from_step_response(&snapshot, &step);
                if step.status.trim().eq_ignore_ascii_case("won")
                    || step.status.trim().eq_ignore_ascii_case("abandoned")
                {
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
        "固定 ID 队列已耗尽，但本局仍未结束",
    ))
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

fn build_round_from_step(
    email: &str,
    continued: bool,
    progress: RoundProgress,
    used_powerups: &[String],
    started: Instant,
    snapshot: &SessionSnapshot,
    step: &StepResponse,
    remaining_after: i32,
) -> RoundResultSummary {
    RoundResultSummary {
        email: email.to_string(),
        difficulty: snapshot.difficulty.clone(),
        round_index: progress.current,
        round_total: progress.total,
        session_id: snapshot.session_id,
        continued,
        dry_run: false,
        status: step.status.clone(),
        reward: step.reward_amount,
        balance_after: Some(step.balance),
        remaining_after,
        move_count: step.move_count,
        used_powerups: used_powerups.to_vec(),
        duration_ms: started.elapsed().as_millis() as i64,
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
    match result.status.trim() {
        "won" => summary.won += 1,
        "abandoned" => summary.abandoned += 1,
        _ => summary.failed += 1,
    }
}
