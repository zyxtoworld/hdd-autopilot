use std::io;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::model::{
    SCRATCH_GAME_TYPE_ICON_MATCH, SCRATCH_GAME_TYPE_LUCKY_NUMBERS, SCRATCH_GAME_TYPE_PROGRESS_RUN,
    SCRATCH_GAME_TYPE_THREE_KIND, SCRATCH_GAME_TYPE_TREASURE_CHEST, ScratchHistoryItem,
    ScratchPlayResponse, ScratchRoundResult, scratch_reveal_ready_at,
};
use crate::ui;
use crate::workflows::common::current_unix_ms;
use rand::prelude::IndexedRandom;

use super::auth::with_auth_retry;
use super::log::{append_scratch_round_log, log_round_result};
use super::{AccountRuntime, BatchState, RunOptions};

const PLAY_ERROR_BACKOFF: Duration = Duration::from_secs(3);
const GAME_TYPES: &[&str] = &[
    SCRATCH_GAME_TYPE_LUCKY_NUMBERS,
    SCRATCH_GAME_TYPE_THREE_KIND,
    SCRATCH_GAME_TYPE_ICON_MATCH,
    SCRATCH_GAME_TYPE_TREASURE_CHEST,
    SCRATCH_GAME_TYPE_PROGRESS_RUN,
];

pub(super) enum RoundLoop {
    Continue,
    Done,
    Error(io::Error),
}

pub(super) fn run_one_round(
    cancel_flag: &ui::CancelFlag,
    state: &Arc<Mutex<BatchState>>,
    runtime: &mut AccountRuntime,
    options: &RunOptions,
    log_dir: &Path,
) -> io::Result<RoundLoop> {
    let round = runtime.rounds_played + 1;
    let result = run_round(cancel_flag, state, runtime, round, options)?;

    runtime.rounds_played += 1;
    add_round_totals(&result, &mut runtime.total_cost, &mut runtime.total_reward);
    log_round_result(
        &state.lock().unwrap().log,
        &result,
        runtime.total_cost,
        runtime.total_reward,
    );
    append_scratch_round_log(
        log_dir,
        runtime.email(),
        &result,
        runtime.total_cost,
        runtime.total_reward,
    )?;

    if !result.play_error_message.is_empty() {
        if is_daily_limit_reached(&result.play_error_message) {
            return Ok(RoundLoop::Done);
        }
        ui::sleep_with_cancel(cancel_flag, PLAY_ERROR_BACKOFF)?;
        return Ok(RoundLoop::Error(io::Error::other(
            result.play_error_message.clone(),
        )));
    }
    if !result.play_history_error_message.is_empty() {
        return Ok(RoundLoop::Error(io::Error::other(
            result.play_history_error_message.clone(),
        )));
    }
    if !result.reveal_error_message.is_empty() {
        return Ok(RoundLoop::Error(io::Error::other(
            result.reveal_error_message.clone(),
        )));
    }
    if !result.reveal_history_error_message.is_empty() {
        return Ok(RoundLoop::Error(io::Error::other(
            result.reveal_history_error_message.clone(),
        )));
    }
    Ok(RoundLoop::Continue)
}

pub(super) fn settle_pending_rounds(
    cancel_flag: &ui::CancelFlag,
    state: &Arc<Mutex<BatchState>>,
    runtime: &mut AccountRuntime,
    options: &RunOptions,
    log_dir: &Path,
) -> io::Result<()> {
    ui::check_cancel(cancel_flag)?;
    let history = with_auth_retry(cancel_flag, state, runtime, |client, auth_token| {
        client.get_scratch_history(auth_token)
    })?;
    let pending_items = pending_scratch_history_items(&history.items);
    if pending_items.is_empty() {
        return Ok(());
    }

    state.lock().unwrap().log.line_fmt(format_args!(
        "账号 {} 有 {} 个未开奖的旧对局，先补开奖再继续。",
        runtime.email(),
        pending_items.len()
    ));

    for item in pending_items {
        ui::check_cancel(cancel_flag)?;
        let started = Instant::now();
        let round = runtime.rounds_played + 1;
        let mut result = ScratchRoundResult {
            round,
            play_history_item: Some(item.clone()),
            ..ScratchRoundResult::default()
        };

        match with_auth_retry(cancel_flag, state, runtime, |client, auth_token| {
            client.reveal_scratch(auth_token, item.id, "")
        }) {
            Ok(reveal_resp) => {
                result.reveal_resp = Some(reveal_resp);
                match fetch_scratch_history_item_with_retry(
                    cancel_flag,
                    state,
                    runtime,
                    item.id,
                    options.history_retries,
                    options.history_wait,
                    true,
                ) {
                    Ok((Some(history_item), attempts)) => {
                        result.reveal_history_attempts = attempts as i32;
                        result.reveal_history_item = Some(history_item);
                    }
                    Ok((None, attempts)) => {
                        result.reveal_history_attempts = attempts as i32;
                        result.reveal_history_error_message =
                            format!("对局 {} 的开奖记录暂时还没有同步出来", item.id);
                    }
                    Err(error) => {
                        result.reveal_history_error_message = error.to_string();
                    }
                }
            }
            Err(error) => {
                result.reveal_error_message = error.to_string();
            }
        }

        result.duration_ms = started.elapsed().as_millis() as i64;
        runtime.rounds_played += 1;
        add_round_totals(&result, &mut runtime.total_cost, &mut runtime.total_reward);
        log_round_result(
            &state.lock().unwrap().log,
            &result,
            runtime.total_cost,
            runtime.total_reward,
        );
        append_scratch_round_log(
            log_dir,
            runtime.email(),
            &result,
            runtime.total_cost,
            runtime.total_reward,
        )?;

        if !result.reveal_error_message.is_empty() {
            return Err(io::Error::other(result.reveal_error_message));
        }
        if !result.reveal_history_error_message.is_empty() {
            return Err(io::Error::other(result.reveal_history_error_message));
        }
    }

    Ok(())
}

fn run_round(
    cancel_flag: &ui::CancelFlag,
    state: &Arc<Mutex<BatchState>>,
    runtime: &mut AccountRuntime,
    round: i32,
    options: &RunOptions,
) -> io::Result<ScratchRoundResult> {
    ui::check_cancel(cancel_flag)?;
    let started = Instant::now();
    let mut result = ScratchRoundResult {
        round,
        ..ScratchRoundResult::default()
    };

    let game_type = random_scratch_game_type();
    let play_resp = match with_auth_retry(cancel_flag, state, runtime, |client, auth_token| {
        client.play_scratch(auth_token, &game_type)
    }) {
        Ok(play_resp) => play_resp,
        Err(error) => {
            result.play_error_message = error.to_string();
            result.duration_ms = started.elapsed().as_millis() as i64;
            return Ok(result);
        }
    };
    result.play_resp = Some(play_resp.clone());

    match fetch_scratch_history_item_with_retry(
        cancel_flag,
        state,
        runtime,
        play_resp.play_id,
        options.history_retries,
        options.history_wait,
        false,
    ) {
        Ok((Some(history_item), attempts)) => {
            result.play_history_attempts = attempts as i32;
            result.play_history_item = Some(history_item);
        }
        Ok((None, attempts)) => {
            result.play_history_attempts = attempts as i32;
            result.play_history_error_message =
                format!("对局 {} 的开局记录暂时还没有同步出来", play_resp.play_id);
            result.duration_ms = started.elapsed().as_millis() as i64;
            return Ok(result);
        }
        Err(error) => {
            result.play_history_error_message = error.to_string();
            result.duration_ms = started.elapsed().as_millis() as i64;
            return Ok(result);
        }
    }

    if play_resp.reveal_token.trim().is_empty() {
        result.reveal_error_message = format!("对局 {} 的开奖令牌为空", play_resp.play_id);
        result.duration_ms = started.elapsed().as_millis() as i64;
        return Ok(result);
    }

    wait_until_reveal_ready(cancel_flag, runtime, &play_resp)?;
    let reveal_resp = match with_auth_retry(cancel_flag, state, runtime, |client, auth_token| {
        client.reveal_scratch(auth_token, play_resp.play_id, &play_resp.reveal_token)
    }) {
        Ok(reveal_resp) => reveal_resp,
        Err(error) => {
            result.reveal_error_message = error.to_string();
            result.duration_ms = started.elapsed().as_millis() as i64;
            return Ok(result);
        }
    };
    result.reveal_resp = Some(reveal_resp);

    match fetch_scratch_history_item_with_retry(
        cancel_flag,
        state,
        runtime,
        play_resp.play_id,
        options.history_retries,
        options.history_wait,
        true,
    ) {
        Ok((Some(history_item), attempts)) => {
            result.reveal_history_attempts = attempts as i32;
            result.reveal_history_item = Some(history_item);
        }
        Ok((None, attempts)) => {
            result.reveal_history_attempts = attempts as i32;
            result.reveal_history_error_message =
                format!("对局 {} 的开奖记录暂时还没有同步出来", play_resp.play_id);
        }
        Err(error) => {
            result.reveal_history_error_message = error.to_string();
        }
    }

    result.duration_ms = started.elapsed().as_millis() as i64;
    Ok(result)
}

fn fetch_scratch_history_item_with_retry(
    cancel_flag: &ui::CancelFlag,
    state: &Arc<Mutex<BatchState>>,
    runtime: &mut AccountRuntime,
    play_id: i32,
    attempts: usize,
    wait: Duration,
    require_non_pending: bool,
) -> io::Result<(Option<ScratchHistoryItem>, usize)> {
    let attempts = attempts.max(1);
    for attempt in 1..=attempts {
        ui::check_cancel(cancel_flag)?;
        let history = with_auth_retry(cancel_flag, state, runtime, |client, auth_token| {
            client.get_scratch_history(auth_token)
        })?;
        let item = find_scratch_history_item(&history.items, play_id);
        let accepted = if require_non_pending {
            item.as_ref()
                .map(|item| !item.status.trim().eq_ignore_ascii_case("pending"))
                .unwrap_or(false)
        } else {
            item.is_some()
        };
        if accepted {
            return Ok((item, attempt));
        }
        if attempt < attempts && wait > Duration::ZERO {
            ui::sleep_with_cancel(cancel_flag, wait)?;
        }
    }
    Ok((None, attempts))
}

fn wait_until_reveal_ready(
    cancel_flag: &ui::CancelFlag,
    runtime: &mut AccountRuntime,
    play_resp: &ScratchPlayResponse,
) -> io::Result<()> {
    ui::check_cancel(cancel_flag)?;
    let target_ms = scratch_reveal_ready_at(play_resp);
    let now_ms = current_unix_ms();
    let until_server = Duration::from_millis(target_ms.saturating_sub(now_ms).max(0) as u64);
    let until_local = runtime
        .next_reveal_allowed_at
        .checked_duration_since(Instant::now())
        .unwrap_or(Duration::ZERO);
    let wait = until_server.max(until_local);
    ui::sleep_with_cancel(cancel_flag, wait)?;
    runtime.next_reveal_allowed_at = Instant::now();
    Ok(())
}

fn random_scratch_game_type() -> String {
    let mut rng = rand::rng();
    GAME_TYPES
        .choose(&mut rng)
        .copied()
        .unwrap_or(SCRATCH_GAME_TYPE_LUCKY_NUMBERS)
        .to_string()
}

fn find_scratch_history_item(
    items: &[ScratchHistoryItem],
    play_id: i32,
) -> Option<ScratchHistoryItem> {
    items.iter().find(|item| item.id == play_id).cloned()
}

fn pending_scratch_history_items(items: &[ScratchHistoryItem]) -> Vec<ScratchHistoryItem> {
    let mut pending = items
        .iter()
        .filter(|item| item.status.trim().eq_ignore_ascii_case("pending"))
        .cloned()
        .collect::<Vec<_>>();
    pending.sort_by_key(|item| item.id);
    pending
}

fn is_daily_limit_reached(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    lower.contains("daily limit reached") || message.contains("今天这个难度的次数已经用完了")
}

fn add_round_totals(result: &ScratchRoundResult, total_cost: &mut f64, total_reward: &mut f64) {
    if let Some(play_resp) = &result.play_resp {
        *total_cost += play_resp.cost_amount;
    } else if let Some(play_history_item) = &result.play_history_item {
        *total_cost += play_history_item.cost_amount.unwrap_or(0.0);
    }
    if let Some(reveal_history_item) = &result.reveal_history_item {
        *total_reward += reveal_history_item.reward_amount.unwrap_or(0.0);
    } else if let Some(reveal_resp) = &result.reveal_resp {
        *total_reward += reveal_resp.reward_amount;
    }
}
