use std::io;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use crate::model::{NonogramConfigResponse, NonogramFinishResponse, NonogramMove, NonogramSession};
use crate::solver::nonogram;
use crate::ui;
use crate::workflows::common::{
    AccountRuntime, BatchState, ServerClockSnapshot, current_unix_ms, is_pending_round_status,
    with_auth_retry_api_until_success,
};

use super::types::{NonogramDifficultySummary, NonogramRoundSummary, RoundProgress};

pub(super) fn is_pending_session(session: &NonogramSession) -> bool {
    if session.session_id <= 0 || session.ended_at_ms.is_some() || session.won {
        return false;
    }
    is_pending_round_status(&session.status)
}

pub(super) fn play_round(
    cancel_flag: &ui::CancelFlag,
    state: &Arc<Mutex<BatchState>>,
    runtime: &mut AccountRuntime,
    config: &NonogramConfigResponse,
    start: NonogramSession,
    continued: bool,
    progress: RoundProgress,
    server_now_ms: i64,
) -> io::Result<NonogramRoundSummary> {
    let started = Instant::now();
    let session = start;
    let steps = match nonogram::solve(&session) {
        Ok(steps) => steps,
        Err(error) => {
            return Ok(build_round_summary(
                runtime.email(),
                &session,
                RoundBuildContext {
                    continued,
                    progress: &progress,
                    started,
                    planned_steps: 0,
                    actual_steps: 0,
                    error_message: format!("nonogram solve failed: {error}"),
                },
            ));
        }
    };
    let planned_steps = steps.len().min(i32::MAX as usize) as i32;
    if planned_steps == 0 && !is_finished(&session) {
        return Ok(build_round_summary(
            runtime.email(),
            &session,
            RoundBuildContext {
                continued,
                progress: &progress,
                started,
                planned_steps: 0,
                actual_steps: 0,
                error_message: "nonogram solver returned no moves".to_string(),
            },
        ));
    }

    play_round_with_finish(
        cancel_flag,
        state,
        runtime,
        config,
        session,
        continued,
        &progress,
        started,
        planned_steps,
        steps,
        server_now_ms,
    )
}

fn play_round_with_finish(
    cancel_flag: &ui::CancelFlag,
    state: &Arc<Mutex<BatchState>>,
    runtime: &mut AccountRuntime,
    config: &NonogramConfigResponse,
    mut session: NonogramSession,
    continued: bool,
    progress: &RoundProgress,
    started: Instant,
    planned_steps: i32,
    steps: Vec<nonogram::NonogramStep>,
    server_now_ms: i64,
) -> io::Result<NonogramRoundSummary> {
    sleep_before_finish(
        cancel_flag,
        ServerClockSnapshot::new(server_now_ms),
        session.started_at_ms,
        planned_steps,
        config.min_interval_ms,
    )?;
    ui::check_cancel(cancel_flag)?;
    let moves = steps
        .into_iter()
        .map(|step| NonogramMove(step.action, step.r, step.c))
        .collect::<Vec<_>>();
    let response = finish_once(
        cancel_flag,
        state,
        runtime,
        FinishAttempt {
            session_id: session.session_id,
            moves,
        },
    )?;
    let actual_steps = planned_steps;
    if !response.ok {
        return Ok(build_round_summary(
            runtime.email(),
            &session,
            RoundBuildContext {
                continued,
                progress,
                started,
                planned_steps,
                actual_steps,
                error_message: "nonogram finish returned ok=false".to_string(),
            },
        ));
    }
    session = merge_session(&session, response);

    let mut result = build_round_summary(
        runtime.email(),
        &session,
        RoundBuildContext {
            continued,
            progress,
            started,
            planned_steps,
            actual_steps,
            error_message: String::new(),
        },
    );
    if !is_finished(&session) {
        result.error_message = "nonogram finish did not settle the session".to_string();
    }
    Ok(result)
}

struct FinishAttempt {
    session_id: i32,
    moves: Vec<NonogramMove>,
}

fn finish_once(
    cancel_flag: &ui::CancelFlag,
    state: &Arc<Mutex<BatchState>>,
    runtime: &mut AccountRuntime,
    attempt: FinishAttempt,
) -> io::Result<NonogramFinishResponse> {
    with_auth_retry_api_until_success(
        cancel_flag,
        state,
        runtime,
        "nonogram finish",
        |client, auth_token| {
            client.finish_nonogram(auth_token, attempt.session_id, attempt.moves.clone())
        },
    )
}

fn sleep_before_finish(
    cancel_flag: &ui::CancelFlag,
    server_clock: ServerClockSnapshot,
    started_at_ms: i64,
    planned_steps: i32,
    min_interval_ms: i32,
) -> io::Result<()> {
    let wait_ms = finish_wait_ms(server_clock, started_at_ms, planned_steps, min_interval_ms);
    if wait_ms > 0 {
        ui::sleep_with_cancel(
            cancel_flag,
            std::time::Duration::from_millis(wait_ms as u64),
        )?;
    }
    Ok(())
}

fn finish_wait_ms(
    server_clock: ServerClockSnapshot,
    started_at_ms: i64,
    planned_steps: i32,
    min_interval_ms: i32,
) -> i64 {
    let min_move_delay = if min_interval_ms > 0 && planned_steps > 0 {
        min_interval_ms as i64 * planned_steps as i64
    } else {
        0
    };
    let elapsed = server_clock.elapsed_since_ms(started_at_ms);
    let required = min_move_delay.max(3_100);
    required.saturating_sub(elapsed)
}

fn merge_session(previous: &NonogramSession, response: NonogramFinishResponse) -> NonogramSession {
    let mut session = if response.session.session_id > 0 {
        response.session
    } else {
        previous.clone()
    };
    let merged_status = merged_response_status(
        response.won,
        &response.status,
        &response.resolution,
        &session.status,
    );
    if !merged_status.trim().is_empty() {
        session.status = merged_status;
    }
    session.won = session.won || response.won || session.status.trim().eq_ignore_ascii_case("won");
    if response.reward_amount != 0.0 {
        session.reward_amount = response.reward_amount;
    }
    session
}

fn merged_response_status(won: bool, status: &str, resolution: &str, fallback: &str) -> String {
    if won {
        return "won".to_string();
    }
    let status = status.trim();
    let resolution = resolution.trim();
    if is_terminal_status(resolution) && (status.is_empty() || is_pending_round_status(status)) {
        return resolution.to_string();
    }
    if !status.is_empty() {
        return status.to_string();
    }
    if !resolution.is_empty() {
        return resolution.to_string();
    }
    fallback.trim().to_string()
}

pub(super) fn merge_round_into_summary(
    summary: &mut NonogramDifficultySummary,
    result: &NonogramRoundSummary,
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
    } else if !is_pending_round_status(&result.status) {
        summary.failed += 1;
    }
}

struct RoundBuildContext<'a> {
    continued: bool,
    progress: &'a RoundProgress,
    started: Instant,
    planned_steps: i32,
    actual_steps: i32,
    error_message: String,
}

fn build_round_summary(
    email: &str,
    session: &NonogramSession,
    context: RoundBuildContext<'_>,
) -> NonogramRoundSummary {
    NonogramRoundSummary {
        email: email.to_string(),
        difficulty: session.difficulty.clone(),
        round_index: context.progress.current,
        round_total: context.progress.total,
        session_id: session.session_id,
        continued: context.continued,
        status: status_for_session(session),
        reward: session.reward_amount,
        remaining_after: 0,
        planned_steps: context.planned_steps,
        actual_steps: context.actual_steps,
        duration_ms: context.started.elapsed().as_millis() as i64,
        when_unix_ms: current_unix_ms(),
        error_message: context.error_message,
    }
}

fn status_for_session(session: &NonogramSession) -> String {
    if session.won {
        "won".to_string()
    } else if session.status.trim().is_empty() {
        "pending".to_string()
    } else {
        session.status.clone()
    }
}

fn is_finished(session: &NonogramSession) -> bool {
    session.won || is_terminal_status(&session.status)
}

fn is_terminal_status(status: &str) -> bool {
    matches!(
        status.trim().to_ascii_lowercase().as_str(),
        "won" | "lost" | "failed" | "game_over" | "abandoned"
    )
}

pub(super) fn is_daily_limit_error(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    lower.contains("daily limit")
        || lower.contains("no remaining plays")
        || lower.contains("remaining plays exhausted")
        || lower.contains("plays exhausted")
}

pub(super) fn is_active_session_error(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    lower.contains("active session")
        || lower.contains("finish your current")
        || lower.contains("current nonogram session")
        || lower.contains("max active")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finish_wait_uses_server_clock_snapshot() {
        let wait_ms = finish_wait_ms(ServerClockSnapshot::new(10_900), 10_000, 10, 75);

        assert!((2_000..=2_200).contains(&wait_ms));
    }
}
