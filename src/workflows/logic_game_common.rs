use std::io;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use crate::model::{
    AuthCache, AuthConfig, LogicGameActionResponse, LogicGameConfigResponse, LogicGameKind,
    LogicGameMeResponse, LogicGameSession, LogicGameStartResponse,
};
use crate::runtime::resolve_data_file_path;
use crate::solver::{flowfree, lightsout, maze, nonogram, sokoban};
use crate::ui;
use crate::workflows::common::{
    AccountRewardSummary, AccountRuntime, BatchState, ensure_authenticated, format_amount,
    format_duration_ms, is_pending_round_status, print_account_reward_summary,
    retry_operation_with_step, run_account_task_until_complete, with_auth_retry_api_until_success,
};

#[derive(Debug, Clone)]
pub(crate) struct AccountRunOutput {
    pub account: AuthCache,
    pub total_reward: f64,
}

#[derive(Debug, Clone, Default)]
struct RoundSummary {
    difficulty: String,
    session_id: i32,
    status: String,
    reward: f64,
    steps: i32,
    duration_ms: i64,
    error_message: String,
}

pub(crate) fn run_batch(
    config: AuthConfig,
    auth_cache_file: impl AsRef<Path>,
    cancel_flag: &ui::CancelFlag,
    log: &ui::TaskLog,
    kind: LogicGameKind,
) -> io::Result<AuthConfig> {
    if config.accounts.is_empty() {
        println!("当前还没有可用账号。");
        return Ok(config);
    }

    let state = Arc::new(Mutex::new(BatchState {
        config: config.clone(),
        auth_cache_file: Some(auth_cache_file.as_ref().to_path_buf()),
        result_log_dir: resolve_data_file_path(format!("log/{}", kind.slug())),
        log: log.clone(),
    }));

    let accounts = config.accounts.clone();
    let base_url = config.base_url.clone();
    log.line_fmt(format_args!(
        "开始自动{}，本次会处理 {} 个账号。",
        kind.title(),
        accounts.len()
    ));

    let mut reward_summaries = accounts
        .iter()
        .enumerate()
        .map(|(index, account)| AccountRewardSummary {
            index,
            email: account.email.trim().to_string(),
            total_reward: 0.0,
        })
        .collect::<Vec<_>>();
    let mut handles = Vec::with_capacity(accounts.len());
    for (index, account) in accounts.into_iter().enumerate() {
        ui::check_cancel(cancel_flag)?;
        let state = Arc::clone(&state);
        let cancel_flag = Arc::clone(cancel_flag);
        let base_url = base_url.clone();
        handles.push(std::thread::spawn(
            move || -> io::Result<AccountRewardSummary> {
                let mut runtime = AccountRuntime::new(&base_url, account);
                let email = runtime.email().to_string();
                let task_log = state.lock().unwrap().log.clone();
                let rounds = run_account_task_until_complete(
                    &cancel_flag,
                    &task_log,
                    &format!("自动{}", kind.title()),
                    &email,
                    || run_account(&cancel_flag, &state, &mut runtime, kind),
                )?;
                Ok(AccountRewardSummary {
                    index,
                    email,
                    total_reward: rounds.iter().map(|round| round.reward).sum(),
                })
            },
        ));
    }

    for handle in handles {
        match handle.join() {
            Ok(Ok(summary)) => {
                if let Some(slot) = reward_summaries.get_mut(summary.index) {
                    *slot = summary;
                }
            }
            Ok(Err(error)) if error.kind() == io::ErrorKind::Interrupted => return Err(error),
            Ok(Err(error)) => return Err(error),
            Err(_) => state.lock().unwrap().log.line_fmt(format_args!(
                "自动{}任务异常退出，请查看前面的账号日志定位原因。",
                kind.title()
            )),
        }
    }
    print_account_reward_summary(log, &format!("自动{}", kind.title()), &reward_summaries);

    Ok(state.lock().unwrap().config.clone())
}

pub(crate) fn run_account_for_free_play_with_log(
    config: &AuthConfig,
    account: AuthCache,
    cancel_flag: &ui::CancelFlag,
    log: &ui::TaskLog,
    kind: LogicGameKind,
) -> io::Result<AccountRunOutput> {
    let fallback_account = account.clone();
    let state = Arc::new(Mutex::new(BatchState {
        config: AuthConfig {
            base_url: config.base_url.clone(),
            accounts: vec![account.clone()],
        },
        auth_cache_file: None,
        result_log_dir: resolve_data_file_path(format!("log/{}", kind.slug())),
        log: log.clone(),
    }));
    let mut runtime = AccountRuntime::new(&config.base_url, account);
    let task_log = state.lock().unwrap().log.clone();
    let email = runtime.email().to_string();
    let rounds = run_account_task_until_complete(
        cancel_flag,
        &task_log,
        &format!("自动{}", kind.title()),
        &email,
        || run_account(cancel_flag, &state, &mut runtime, kind),
    )?;
    let total_reward = rounds.iter().map(|round| round.reward).sum();
    let updated_account = state
        .lock()
        .unwrap()
        .config
        .accounts
        .first()
        .cloned()
        .unwrap_or(fallback_account);
    Ok(AccountRunOutput {
        account: updated_account,
        total_reward,
    })
}

fn run_account(
    cancel_flag: &ui::CancelFlag,
    state: &Arc<Mutex<BatchState>>,
    runtime: &mut AccountRuntime,
    kind: LogicGameKind,
) -> io::Result<Vec<RoundSummary>> {
    ui::check_cancel(cancel_flag)?;
    ensure_authenticated(state, runtime)?;
    let config = with_auth_retry_api_until_success(
        cancel_flag,
        state,
        runtime,
        &format!("{} config", kind.slug()),
        |client, auth_token| client.get_logic_game_config(auth_token, kind),
    )?;
    let difficulties = difficulty_order(&config);
    state.lock().unwrap().log.line_fmt(format_args!(
        "账号 {} 已准备好：{}包含 {} 个难度，最高同时 {} 局，最小操作间隔 {}ms。",
        runtime.email(),
        kind.title(),
        difficulties.len(),
        config.max_active_sessions,
        config.min_interval_ms,
    ));

    let mut me = fetch_me(cancel_flag, state, runtime, kind)?;
    let mut remaining = me.daily_plays_remaining.clone();
    let mut rounds = Vec::new();
    if let Some(active) = me
        .active_session
        .as_ref()
        .filter(|session| is_pending_session(session))
        .cloned()
    {
        state.lock().unwrap().log.line_fmt(format_args!(
            "账号 {} 发现{}残局（{} 难度，对局 {}），先继续完成。",
            runtime.email(),
            kind.title(),
            active.difficulty,
            active.session_id
        ));
        let summary = play_round(cancel_flag, state, runtime, kind, &config, active, true)?;
        log_round(state, runtime.email(), kind, &summary);
        rounds.push(summary);
        me = fetch_me(cancel_flag, state, runtime, kind)?;
        remaining = me.daily_plays_remaining.clone();
    }

    for difficulty in difficulties {
        let mut left = remaining.get(&difficulty).copied().unwrap_or(0).max(0);
        while left > 0 {
            ui::check_cancel(cancel_flag)?;
            state.lock().unwrap().log.line_fmt(format_args!(
                "账号 {} 开始玩{}{}难度，今天还剩 {} 局。",
                runtime.email(),
                kind.title(),
                localized_difficulty(&difficulty),
                left
            ));
            let start = match start_new_round(cancel_flag, state, runtime, kind, &difficulty) {
                Ok(start) => start,
                Err(error) if is_daily_limit_error(&error.to_string()) => {
                    remaining.insert(difficulty.clone(), 0);
                    break;
                }
                Err(error) if is_active_session_error(&error.to_string()) => {
                    let refreshed = fetch_me(cancel_flag, state, runtime, kind)?;
                    let Some(active) = refreshed
                        .active_session
                        .as_ref()
                        .filter(|session| is_pending_session(session))
                        .cloned()
                    else {
                        return Err(error);
                    };
                    let summary =
                        play_round(cancel_flag, state, runtime, kind, &config, active, true)?;
                    log_round(state, runtime.email(), kind, &summary);
                    if summary.difficulty == difficulty {
                        left = left.saturating_sub(1);
                        remaining.insert(difficulty.clone(), left);
                    }
                    rounds.push(summary);
                    continue;
                }
                Err(error) => return Err(error),
            };
            let mut summary = play_round(
                cancel_flag,
                state,
                runtime,
                kind,
                &config,
                start.session,
                false,
            )?;
            if let Some(next_left) = start.daily_plays_remaining.get(&difficulty).copied() {
                left = next_left.max(0);
            } else {
                left = left.saturating_sub(1);
            }
            remaining.insert(difficulty.clone(), left);
            summary.difficulty = difficulty.clone();
            log_round(state, runtime.email(), kind, &summary);
            rounds.push(summary);
        }
    }

    state.lock().unwrap().log.line_fmt(format_args!(
        "账号 {} 的自动{}运行完成，总收益 {}。",
        runtime.email(),
        kind.title(),
        format_amount(rounds.iter().map(|round| round.reward).sum()),
    ));
    Ok(rounds)
}

fn fetch_me(
    cancel_flag: &ui::CancelFlag,
    state: &Arc<Mutex<BatchState>>,
    runtime: &mut AccountRuntime,
    kind: LogicGameKind,
) -> io::Result<LogicGameMeResponse> {
    with_auth_retry_api_until_success(
        cancel_flag,
        state,
        runtime,
        &format!("{} me", kind.slug()),
        |client, auth_token| client.get_logic_game_me(auth_token, kind),
    )
}

fn start_new_round(
    cancel_flag: &ui::CancelFlag,
    state: &Arc<Mutex<BatchState>>,
    runtime: &mut AccountRuntime,
    kind: LogicGameKind,
    difficulty: &str,
) -> io::Result<LogicGameStartResponse> {
    let result = with_auth_retry_api_until_success(
        cancel_flag,
        state,
        runtime,
        &format!("{} start", kind.slug()),
        |client, auth_token| client.start_logic_game(auth_token, kind, difficulty),
    );
    match result {
        Ok(start) if !start.ok => Err(io::Error::other(format!(
            "{}开局接口返回 ok=false",
            kind.title()
        ))),
        Ok(start) if start.session.session_id <= 0 => Err(io::Error::other(format!(
            "{}开局接口没有返回有效对局",
            kind.title()
        ))),
        Ok(start) => Ok(start),
        Err(error) => Err(error),
    }
}

fn play_round(
    cancel_flag: &ui::CancelFlag,
    state: &Arc<Mutex<BatchState>>,
    runtime: &mut AccountRuntime,
    kind: LogicGameKind,
    config: &LogicGameConfigResponse,
    start: LogicGameSession,
    continued: bool,
) -> io::Result<RoundSummary> {
    let started = Instant::now();
    let mut session = start;
    let steps = match solve_session(kind, &session) {
        Ok(steps) => steps,
        Err(error) => {
            return Ok(round_summary(
                &session,
                started,
                0,
                format!("求解失败：{error}"),
            ));
        }
    };
    let planned_steps = steps.len().min(i32::MAX as usize) as i32;
    if planned_steps == 0 && !is_finished(&session) {
        return Ok(round_summary(
            &session,
            started,
            0,
            "求解器没有给出操作步骤".to_string(),
        ));
    }

    for (index, step) in steps.iter().enumerate() {
        ui::check_cancel(cancel_flag)?;
        let min_interval = config.min_interval_ms.max(0) as u64;
        if min_interval > 0 {
            ui::sleep_with_cancel(cancel_flag, std::time::Duration::from_millis(min_interval))?;
        }
        let response = step_once(
            cancel_flag,
            state,
            runtime,
            kind,
            session.session_id,
            step,
            i32::try_from(index + 1).unwrap_or(i32::MAX),
        )?;
        if !response.ok {
            return Ok(round_summary(
                &session,
                started,
                planned_steps,
                "操作接口返回 ok=false".to_string(),
            ));
        }
        session = merge_session(&session, response);
        if is_finished(&session) {
            break;
        }
    }

    let mut summary = round_summary(&session, started, planned_steps, String::new());
    if !is_finished(&session) && summary.error_message.is_empty() {
        summary.error_message = "执行完求解步骤后服务端仍未结算通关".to_string();
    }
    if continued {
        summary.status = format!("{}（续玩）", summary.status);
    }
    Ok(summary)
}

fn solve_session(
    kind: LogicGameKind,
    session: &LogicGameSession,
) -> Result<Vec<crate::model::LogicGameStep>, String> {
    match kind {
        LogicGameKind::Sokoban => sokoban::solve(session),
        LogicGameKind::LightsOut => lightsout::solve(session),
        LogicGameKind::Maze => maze::solve(session),
        LogicGameKind::Nonogram => nonogram::solve(session),
        LogicGameKind::FlowFree => flowfree::solve(session),
    }
}

fn step_once(
    cancel_flag: &ui::CancelFlag,
    state: &Arc<Mutex<BatchState>>,
    runtime: &mut AccountRuntime,
    kind: LogicGameKind,
    session_id: i32,
    step: &crate::model::LogicGameStep,
    step_number: i32,
) -> io::Result<LogicGameActionResponse> {
    let operation = retry_operation_with_step(
        &format!("{} {}", kind.slug(), kind.action_name()),
        step_number,
    );
    with_auth_retry_api_until_success(
        cancel_flag,
        state,
        runtime,
        &operation,
        |client, auth_token| client.step_logic_game(auth_token, kind, session_id, step),
    )
}

fn merge_session(
    previous: &LogicGameSession,
    response: LogicGameActionResponse,
) -> LogicGameSession {
    let mut session = if response.session.session_id > 0 {
        response.session
    } else {
        previous.clone()
    };
    if !response.status.trim().is_empty() {
        session.status = response.status;
    }
    if !response.resolution.trim().is_empty() && session.status.trim().is_empty() {
        session.status = response.resolution;
    }
    session.won = session.won || response.won;
    if response.reward_amount != 0.0 {
        session.reward_amount = response.reward_amount;
    }
    session
}

fn round_summary(
    session: &LogicGameSession,
    started: Instant,
    planned_steps: i32,
    error_message: String,
) -> RoundSummary {
    RoundSummary {
        difficulty: session.difficulty.clone(),
        session_id: session.session_id,
        status: status_for_session(session),
        reward: session.reward_amount,
        steps: planned_steps,
        duration_ms: started.elapsed().as_millis() as i64,
        error_message,
    }
}

fn log_round(
    state: &Arc<Mutex<BatchState>>,
    email: &str,
    kind: LogicGameKind,
    summary: &RoundSummary,
) {
    let result = if summary.error_message.trim().is_empty()
        && summary.status.to_ascii_lowercase().contains("won")
    {
        "成功"
    } else if summary.error_message.trim().is_empty() {
        "结束"
    } else {
        "失败"
    };
    let mut line = format!(
        "账号 {} 的{}{}难度第 {} 局{}：状态 {}，步骤 {}，收益 {}，耗时 {}。",
        email,
        kind.title(),
        localized_difficulty(&summary.difficulty),
        summary.session_id,
        result,
        summary.status,
        summary.steps,
        format_amount(summary.reward),
        format_duration_ms(summary.duration_ms),
    );
    if !summary.error_message.trim().is_empty() {
        line.push_str(&format!("原因：{}。", summary.error_message));
    }
    state.lock().unwrap().log.line_fmt(format_args!("{}", line));
}

fn difficulty_order(config: &LogicGameConfigResponse) -> Vec<String> {
    let mut ordered = Vec::new();
    for difficulty in ["easy", "normal", "hard"] {
        if config.difficulties.contains_key(difficulty) {
            ordered.push(difficulty.to_string());
        }
    }
    let mut extra = config
        .difficulties
        .keys()
        .filter(|difficulty| !ordered.contains(*difficulty))
        .cloned()
        .collect::<Vec<_>>();
    extra.sort();
    ordered.extend(extra);
    ordered
}

fn is_pending_session(session: &LogicGameSession) -> bool {
    if session.session_id <= 0 || session.won {
        return false;
    }
    is_pending_round_status(&session.status)
}

fn is_finished(session: &LogicGameSession) -> bool {
    session.won
        || matches!(
            session.status.trim().to_ascii_lowercase().as_str(),
            "won" | "lost" | "failed" | "game_over" | "abandoned"
        )
}

fn status_for_session(session: &LogicGameSession) -> String {
    if session.won {
        "won".to_string()
    } else if session.status.trim().is_empty() {
        "pending".to_string()
    } else {
        session.status.clone()
    }
}

fn localized_difficulty(difficulty: &str) -> String {
    match difficulty {
        "easy" => "简单".to_string(),
        "normal" => "普通".to_string(),
        "hard" => "困难".to_string(),
        other => other.to_string(),
    }
}

fn is_daily_limit_error(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    lower.contains("次数已经用完")
        || lower.contains("今日次数")
        || lower.contains("daily limit")
        || lower.contains("no remaining plays")
        || lower.contains("remaining plays exhausted")
}

fn is_active_session_error(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    lower.contains("未结束对局") || lower.contains("active session") || lower.contains("max active")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn difficulty_order_keeps_known_order_then_sorted_extras() {
        let mut config = LogicGameConfigResponse::default();
        config.difficulties.insert(
            "zzz".to_string(),
            crate::model::LogicGameDifficultyConfig {
                reward_amount: 9.0,
                ..Default::default()
            },
        );
        config.difficulties.insert(
            "easy".to_string(),
            crate::model::LogicGameDifficultyConfig {
                reward_amount: 0.3,
                ..Default::default()
            },
        );
        config.difficulties.insert(
            "hard".to_string(),
            crate::model::LogicGameDifficultyConfig {
                reward_amount: 3.0,
                ..Default::default()
            },
        );
        config.difficulties.insert(
            "normal".to_string(),
            crate::model::LogicGameDifficultyConfig {
                reward_amount: 1.0,
                ..Default::default()
            },
        );

        assert_eq!(difficulty_order(&config), ["easy", "normal", "hard", "zzz"]);
    }
}
