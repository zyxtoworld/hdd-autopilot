use std::io;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use crate::model::{LightsoutClickResponse, LightsoutConfigResponse, LightsoutSession};
use crate::solver::lightsout;
use crate::ui;
use crate::workflows::common::{
    AccountRuntime, BatchState, current_unix_ms, is_pending_round_status,
    retry_operation_with_step, sleep_min_interval, with_auth_retry_api_until_success,
};

use super::types::{LightsoutDifficultySummary, LightsoutRoundSummary, RoundProgress};

pub(super) fn is_pending_session(session: &LightsoutSession) -> bool {
    if session.session_id <= 0 || session.ended_at_ms.is_some() || session.won {
        return false;
    }
    is_pending_round_status(&session.status)
}

pub(super) fn play_round(
    cancel_flag: &ui::CancelFlag,
    state: &Arc<Mutex<BatchState>>,
    runtime: &mut AccountRuntime,
    config: &LightsoutConfigResponse,
    start: LightsoutSession,
    continued: bool,
    progress: RoundProgress,
) -> io::Result<LightsoutRoundSummary> {
    let started = Instant::now();
    let mut session = start;
    let steps = match lightsout::solve(&session) {
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
                    error_message: format!("求解失败：{error}"),
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
                error_message: "求解器没有给出操作步骤".to_string(),
            },
        ));
    }

    let mut actual_steps = 0i32;
    for (r, c) in steps {
        sleep_min_interval(cancel_flag, config.min_interval_ms)?;
        let response = step_once(
            cancel_flag,
            state,
            runtime,
            StepAttempt {
                session_id: session.session_id,
                r,
                c,
                step_number: actual_steps + 1,
            },
        )?;
        actual_steps += 1;
        if !response.ok {
            return Ok(build_round_summary(
                runtime.email(),
                &session,
                RoundBuildContext {
                    continued,
                    progress: &progress,
                    started,
                    planned_steps,
                    actual_steps,
                    error_message: "操作接口返回 ok=false".to_string(),
                },
            ));
        }
        session = merge_session(&session, response);
        if is_finished(&session) {
            break;
        }
    }

    let mut result = build_round_summary(
        runtime.email(),
        &session,
        RoundBuildContext {
            continued,
            progress: &progress,
            started,
            planned_steps,
            actual_steps,
            error_message: String::new(),
        },
    );
    if !is_finished(&session) {
        result.error_message = "执行完求解步骤后服务端仍未结算通关".to_string();
    }
    Ok(result)
}

struct StepAttempt {
    session_id: i32,
    r: i32,
    c: i32,
    step_number: i32,
}

fn step_once(
    cancel_flag: &ui::CancelFlag,
    state: &Arc<Mutex<BatchState>>,
    runtime: &mut AccountRuntime,
    attempt: StepAttempt,
) -> io::Result<LightsoutClickResponse> {
    let operation = retry_operation_with_step("lightsout click", attempt.step_number);
    with_auth_retry_api_until_success(
        cancel_flag,
        state,
        runtime,
        &operation,
        |client, auth_token| {
            client.click_lightsout(auth_token, attempt.session_id, attempt.r, attempt.c)
        },
    )
}

fn merge_session(
    previous: &LightsoutSession,
    response: LightsoutClickResponse,
) -> LightsoutSession {
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
    summary: &mut LightsoutDifficultySummary,
    result: &LightsoutRoundSummary,
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
    session: &LightsoutSession,
    context: RoundBuildContext<'_>,
) -> LightsoutRoundSummary {
    LightsoutRoundSummary {
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

fn status_for_session(session: &LightsoutSession) -> String {
    if session.won {
        "won".to_string()
    } else if session.status.trim().is_empty() {
        "pending".to_string()
    } else {
        session.status.clone()
    }
}

fn is_finished(session: &LightsoutSession) -> bool {
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
    lower.contains("次数已经用完")
        || lower.contains("次数已用完")
        || lower.contains("今日次数")
        || lower.contains("daily limit")
        || lower.contains("no remaining plays")
        || lower.contains("remaining plays exhausted")
}

pub(super) fn is_active_session_error(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    lower.contains("未结束对局")
        || lower.contains("未结束的对局")
        || lower.contains("进行中")
        || lower.contains("active session")
        || lower.contains("max active")
}
