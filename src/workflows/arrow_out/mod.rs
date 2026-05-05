use std::io;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::model::{
    ArrowOutClick, ArrowOutConfigResponse, ArrowOutFinishResponse, ArrowOutHistoryResponse,
    ArrowOutMeResponse, ArrowOutSession, AuthCache, AuthConfig,
};
use crate::runtime::resolve_data_file_path;
use crate::solver::arrow_out;
use crate::ui;
use crate::workflows::common::{
    AccountRewardSummary, AccountRuntime, BatchState, append_account_log_line, current_unix_ms,
    ensure_authenticated, format_amount, print_account_reward_summary,
    with_auth_retry_api_until_success,
};

pub const DONE_MESSAGE: &str = "自动箭头逃离已停止。";
const TASK_TITLE: &str = "自动箭头逃离";
const RESULT_WON: &str = "won";
const LIVE_STATUS_REFRESH_PREFIX: &str = "所有账号实时总收益";
const STATE_SYNC_RETRY_DELAY: Duration = Duration::from_millis(500);

#[derive(Debug, Clone)]
pub struct AccountRunOutput {
    pub account: AuthCache,
    pub total_reward: f64,
}

#[derive(Clone)]
struct LiveRewardTracker {
    title: String,
    log: ui::TaskLog,
    total_reward: Arc<Mutex<f64>>,
}

struct ActiveArrowOutSession {
    session: ArrowOutSession,
    server_now_ms: i64,
    observed_at: Instant,
}

impl LiveRewardTracker {
    fn new(title: &str, log: &ui::TaskLog) -> Self {
        let tracker = Self {
            title: title.to_string(),
            log: log.clone(),
            total_reward: Arc::new(Mutex::new(0.0)),
        };
        tracker.render();
        tracker
    }

    fn add(&self, amount: f64) {
        if amount != 0.0 {
            *self.total_reward.lock().unwrap() += amount;
        }
        self.render();
    }

    fn render(&self) {
        let total = *self.total_reward.lock().unwrap();
        self.log.status_fmt(format_args!(
            "【{}】{}：{}",
            self.title,
            LIVE_STATUS_REFRESH_PREFIX,
            format_amount(total)
        ));
    }
}

pub fn run_batch(
    config: AuthConfig,
    auth_cache_file: impl AsRef<Path>,
    cancel_flag: &ui::CancelFlag,
    log: &ui::TaskLog,
) -> io::Result<AuthConfig> {
    run_batch_with_title(config, auth_cache_file, cancel_flag, log, TASK_TITLE)
}

pub fn run_batch_with_title(
    config: AuthConfig,
    auth_cache_file: impl AsRef<Path>,
    cancel_flag: &ui::CancelFlag,
    log: &ui::TaskLog,
    title: &str,
) -> io::Result<AuthConfig> {
    if config.accounts.is_empty() {
        println!("当前还没有可用账号。");
        return Ok(config);
    }

    let state = Arc::new(Mutex::new(BatchState {
        config: config.clone(),
        auth_cache_file: Some(auth_cache_file.as_ref().to_path_buf()),
        result_log_dir: resolve_data_file_path("log/arrow_out"),
        log: log.clone(),
    }));
    let tracker = LiveRewardTracker::new(title, log);
    let max_rounds = max_rounds_from_env();
    let accounts = config.accounts.clone();
    let base_url = config.base_url.clone();
    log.line_fmt(format_args!(
        "开始{}，本次会处理 {} 个账号；无次数限制玩法会持续运行，按 ESC 停止。",
        title,
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
        let tracker = tracker.clone();
        let base_url = base_url.clone();
        handles.push(std::thread::spawn(
            move || -> io::Result<AccountRewardSummary> {
                let mut runtime = AccountRuntime::new(&base_url, account);
                let email = runtime.email().to_string();
                let total_reward =
                    run_account_loop(&cancel_flag, &state, &mut runtime, &tracker, max_rounds)?;
                Ok(AccountRewardSummary {
                    index,
                    email,
                    total_reward,
                })
            },
        ));
    }

    let mut first_error = None;
    for handle in handles {
        match handle.join() {
            Ok(Ok(summary)) => {
                if let Some(slot) = reward_summaries.get_mut(summary.index) {
                    *slot = summary;
                }
            }
            Ok(Err(error)) if error.kind() == io::ErrorKind::Interrupted => {
                first_error = Some(error);
            }
            Ok(Err(error)) => return Err(error),
            Err(_) => {
                state.lock().unwrap().log.line_fmt(format_args!(
                    "{}任务异常退出，请查看前面的账号日志定位原因。",
                    TASK_TITLE
                ));
            }
        }
    }

    print_account_reward_summary(log, title, &reward_summaries);
    if let Some(error) = first_error {
        return Err(error);
    }
    Ok(state.lock().unwrap().config.clone())
}

fn run_account_loop(
    cancel_flag: &ui::CancelFlag,
    state: &Arc<Mutex<BatchState>>,
    runtime: &mut AccountRuntime,
    tracker: &LiveRewardTracker,
    max_rounds: Option<usize>,
) -> io::Result<f64> {
    ui::check_cancel(cancel_flag)?;
    ensure_authenticated(state, runtime)?;
    let config = fetch_config(cancel_flag, state, runtime)?;
    let mut total_reward = 0.0;
    let mut cleared = 0usize;
    state.lock().unwrap().log.line_fmt(format_args!(
        "账号 {} 已准备好：箭头逃离无限关卡，每关奖励 {}，最小点击间隔 {}ms，最多碰撞 {} 次。",
        runtime.email(),
        format_amount(config.reward_per_clear),
        config.min_interval_ms,
        config.max_collisions,
    ));

    loop {
        ui::check_cancel(cancel_flag)?;
        if max_rounds.is_some_and(|max_rounds| cleared >= max_rounds) {
            break;
        }
        let active_session = pending_or_new_session(cancel_flag, state, runtime)?;
        let session = &active_session.session;
        let click_ids = match arrow_out::solve(session) {
            Ok(click_ids) => click_ids,
            Err(error) => {
                state.lock().unwrap().log.line_fmt(format_args!(
                    "账号 {} 的箭头逃离第 {} 关求解失败：{}，放弃当前局后重新开局。",
                    runtime.email(),
                    session.stage + 1,
                    error
                ));
                let _ = abandon_session(cancel_flag, state, runtime, session.session_id)?;
                continue;
            }
        };
        if click_ids.len() > config.max_clicks.max(0) as usize {
            return Err(io::Error::other(format!(
                "箭头逃离第 {} 关需要 {} 次点击，超过接口上限 {}",
                session.stage + 1,
                click_ids.len(),
                config.max_clicks
            )));
        }
        let clicks = planned_clicks(&config, &session, click_ids);
        let click_count = clicks.len();
        sleep_before_finish(cancel_flag, &active_session, &clicks)?;
        let response = match finish_session(cancel_flag, state, runtime, session.session_id, clicks)
        {
            Ok(response) => response,
            Err(error) if is_state_conflict_error(&error) => {
                let email = runtime.email().to_string();
                state.lock().unwrap().log.line_fmt(format_args!(
                    "账号 {} 的箭头逃离第 {} 关结算状态冲突，重新读取服务端结果后继续。",
                    email,
                    session.stage + 1
                ));
                if let Some(session) =
                    recover_finished_session(cancel_flag, state, runtime, session.session_id)?
                {
                    if session_won(&session) {
                        record_successful_round(
                            state,
                            &email,
                            tracker,
                            &mut total_reward,
                            &mut cleared,
                            session.stage + 1,
                            click_count,
                            session.reward_amount,
                        )?;
                    } else {
                        append_account_log_line(
                            &state.lock().unwrap().result_log_dir,
                            &email,
                            &format!(
                                "{} 第 {} 关结算后不是通关状态：status={}\n",
                                current_unix_ms(),
                                session.stage + 1,
                                session.status
                            ),
                        )?;
                    }
                }
                continue;
            }
            Err(error) => return Err(error),
        };
        let reward = reward_from_finish(&response);
        if !response.ok || !finish_won(&response) {
            append_account_log_line(
                &state.lock().unwrap().result_log_dir,
                runtime.email(),
                &format!(
                    "{} 第 {} 关结算失败：status={} resolution={}\n",
                    current_unix_ms(),
                    session.stage + 1,
                    response.status,
                    response.resolution
                ),
            )?;
            continue;
        }
        record_successful_round(
            state,
            runtime.email(),
            tracker,
            &mut total_reward,
            &mut cleared,
            response.session.stage + 1,
            click_count,
            reward,
        )?;
    }
    Ok(total_reward)
}

fn fetch_config(
    cancel_flag: &ui::CancelFlag,
    state: &Arc<Mutex<BatchState>>,
    runtime: &mut AccountRuntime,
) -> io::Result<ArrowOutConfigResponse> {
    with_auth_retry_api_until_success(
        cancel_flag,
        state,
        runtime,
        "arrow-out config",
        |client, auth_token| client.get_arrow_out_config(auth_token),
    )
}

fn fetch_me(
    cancel_flag: &ui::CancelFlag,
    state: &Arc<Mutex<BatchState>>,
    runtime: &mut AccountRuntime,
) -> io::Result<ArrowOutMeResponse> {
    with_auth_retry_api_until_success(
        cancel_flag,
        state,
        runtime,
        "arrow-out me",
        |client, auth_token| client.get_arrow_out_me(auth_token),
    )
}

fn fetch_history(
    cancel_flag: &ui::CancelFlag,
    state: &Arc<Mutex<BatchState>>,
    runtime: &mut AccountRuntime,
) -> io::Result<ArrowOutHistoryResponse> {
    with_auth_retry_api_until_success(
        cancel_flag,
        state,
        runtime,
        "arrow-out history",
        |client, auth_token| client.get_arrow_out_history(auth_token),
    )
}

fn pending_or_new_session(
    cancel_flag: &ui::CancelFlag,
    state: &Arc<Mutex<BatchState>>,
    runtime: &mut AccountRuntime,
) -> io::Result<ActiveArrowOutSession> {
    let me = fetch_me(cancel_flag, state, runtime)?;
    let server_now_ms = me.server_now_ms;
    if let Some(session) = me.active_session
        && is_pending_session(&session)
    {
        return Ok(active_session(session, server_now_ms));
    }
    let response = match with_auth_retry_api_until_success(
        cancel_flag,
        state,
        runtime,
        "arrow-out start",
        |client, auth_token| client.start_arrow_out(auth_token),
    ) {
        Ok(response) => response,
        Err(error) if is_state_conflict_error(&error) => {
            let email = runtime.email().to_string();
            state.lock().unwrap().log.line_fmt(format_args!(
                "账号 {} 的箭头逃离开局状态冲突，重新读取当前残局。",
                email
            ));
            ui::sleep_with_cancel(cancel_flag, STATE_SYNC_RETRY_DELAY)?;
            let me = fetch_me(cancel_flag, state, runtime)?;
            let server_now_ms = me.server_now_ms;
            if let Some(session) = me.active_session
                && is_pending_session(&session)
            {
                return Ok(active_session(session, server_now_ms));
            }
            return Err(error);
        }
        Err(error) => return Err(error),
    };
    if !response.ok {
        return Err(io::Error::other("arrow-out start returned ok=false"));
    }
    Ok(active_session(response.session, response.server_now_ms))
}

fn finish_session(
    cancel_flag: &ui::CancelFlag,
    state: &Arc<Mutex<BatchState>>,
    runtime: &mut AccountRuntime,
    session_id: i32,
    clicks: Vec<ArrowOutClick>,
) -> io::Result<ArrowOutFinishResponse> {
    with_auth_retry_api_until_success(
        cancel_flag,
        state,
        runtime,
        "arrow-out finish",
        |client, auth_token| {
            client.finish_arrow_out(auth_token, session_id, clicks.clone(), RESULT_WON)
        },
    )
}

fn abandon_session(
    cancel_flag: &ui::CancelFlag,
    state: &Arc<Mutex<BatchState>>,
    runtime: &mut AccountRuntime,
    session_id: i32,
) -> io::Result<ArrowOutFinishResponse> {
    with_auth_retry_api_until_success(
        cancel_flag,
        state,
        runtime,
        "arrow-out abandon",
        |client, auth_token| client.abandon_arrow_out(auth_token, session_id),
    )
}

fn planned_clicks(
    config: &ArrowOutConfigResponse,
    session: &ArrowOutSession,
    click_ids: Vec<i32>,
) -> Vec<ArrowOutClick> {
    let min_gap = config.min_interval_ms.max(1);
    let click_count = click_ids.len().max(1) as i32;
    let elapsed_gap = (session.min_elapsed_ms.max(0) + click_count - 1) / click_count;
    let gap = min_gap.max(elapsed_gap).max(1);
    click_ids
        .into_iter()
        .enumerate()
        .map(|(index, arrow_id)| ArrowOutClick {
            arrow_id,
            t_ms: ((index as i32) + 1) * gap,
        })
        .collect()
}

fn sleep_before_finish(
    cancel_flag: &ui::CancelFlag,
    active_session: &ActiveArrowOutSession,
    clicks: &[ArrowOutClick],
) -> io::Result<()> {
    let session = &active_session.session;
    let last_click_ms = clicks.last().map(|click| click.t_ms).unwrap_or(0).max(0) as i64;
    let required = (session.min_elapsed_ms.max(0) as i64)
        .max(last_click_ms)
        .saturating_add(150);
    let elapsed = estimated_server_elapsed_ms(active_session);
    let wait_ms = required.saturating_sub(elapsed);
    if wait_ms > 0 {
        ui::sleep_with_cancel(
            cancel_flag,
            std::time::Duration::from_millis(wait_ms as u64),
        )?;
    }
    Ok(())
}

fn active_session(session: ArrowOutSession, server_now_ms: i64) -> ActiveArrowOutSession {
    ActiveArrowOutSession {
        session,
        server_now_ms: if server_now_ms > 0 {
            server_now_ms
        } else {
            current_unix_ms()
        },
        observed_at: Instant::now(),
    }
}

fn estimated_server_elapsed_ms(active_session: &ActiveArrowOutSession) -> i64 {
    let local_elapsed = active_session
        .observed_at
        .elapsed()
        .as_millis()
        .min(i64::MAX as u128) as i64;
    active_session
        .server_now_ms
        .saturating_add(local_elapsed)
        .saturating_sub(active_session.session.started_at_ms.max(0))
}

fn is_pending_session(session: &ArrowOutSession) -> bool {
    session.session_id > 0
        && session.ended_at_ms.is_none()
        && session.status.trim().eq_ignore_ascii_case("pending")
}

fn recover_finished_session(
    cancel_flag: &ui::CancelFlag,
    state: &Arc<Mutex<BatchState>>,
    runtime: &mut AccountRuntime,
    session_id: i32,
) -> io::Result<Option<ArrowOutSession>> {
    ui::sleep_with_cancel(cancel_flag, STATE_SYNC_RETRY_DELAY)?;
    let me = fetch_me(cancel_flag, state, runtime)?;
    if let Some(session) = me.active_session {
        if session.session_id == session_id && !is_pending_session(&session) {
            return Ok(Some(session));
        }
        if session.session_id == session_id {
            return Ok(None);
        }
    }
    let history = fetch_history(cancel_flag, state, runtime)?;
    Ok(history
        .items
        .into_iter()
        .find(|session| session.session_id == session_id))
}

fn finish_won(response: &ArrowOutFinishResponse) -> bool {
    response.won
        || response.status.trim().eq_ignore_ascii_case("won")
        || session_won(&response.session)
}

fn session_won(session: &ArrowOutSession) -> bool {
    session.won || session.status.trim().eq_ignore_ascii_case("won")
}

fn reward_from_finish(response: &ArrowOutFinishResponse) -> f64 {
    if response.reward_amount != 0.0 {
        response.reward_amount
    } else {
        response.session.reward_amount
    }
}

fn max_rounds_from_env() -> Option<usize> {
    std::env::var("HDD_AUTOPILOT_ARROW_OUT_MAX_ROUNDS")
        .ok()
        .and_then(|value| value.trim().parse::<usize>().ok())
        .filter(|value| *value > 0)
}

fn record_successful_round(
    state: &Arc<Mutex<BatchState>>,
    email: &str,
    tracker: &LiveRewardTracker,
    total_reward: &mut f64,
    cleared: &mut usize,
    stage: i32,
    click_count: usize,
    reward: f64,
) -> io::Result<()> {
    *total_reward += reward;
    tracker.add(reward);
    *cleared += 1;
    append_account_log_line(
        &state.lock().unwrap().result_log_dir,
        email,
        &format!(
            "{} 第 {} 关通关，点击 {} 次，收益 {}，累计 {}\n",
            current_unix_ms(),
            stage,
            click_count,
            format_amount(reward),
            format_amount(*total_reward)
        ),
    )?;
    if *cleared == 1 || (*cleared).is_multiple_of(10) {
        state.lock().unwrap().log.line_fmt(format_args!(
            "账号 {} 的箭头逃离已通关 {} 关，账号累计收益 {}。",
            email,
            *cleared,
            format_amount(*total_reward)
        ));
    }
    Ok(())
}

fn is_state_conflict_error(error: &io::Error) -> bool {
    let message = error.to_string();
    message.contains("状态码 409")
        || message.contains("请求状态冲突")
        || message.contains("未结束对局")
        || message.contains("状态还没同步")
        || message.contains("active session")
        || message.contains("max active")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn planned_clicks_respect_min_elapsed_and_interval() {
        let config = ArrowOutConfigResponse {
            min_interval_ms: 75,
            ..ArrowOutConfigResponse::default()
        };
        let session = ArrowOutSession {
            min_elapsed_ms: 1000,
            ..ArrowOutSession::default()
        };

        let clicks = planned_clicks(&config, &session, vec![3, 2, 1, 0]);

        assert_eq!(clicks[0].t_ms, 250);
        assert_eq!(clicks[3].t_ms, 1000);
        for pair in clicks.windows(2) {
            assert!(pair[1].t_ms - pair[0].t_ms >= 75);
        }
    }

    #[test]
    fn estimated_elapsed_uses_server_clock_snapshot() {
        let active_session = ActiveArrowOutSession {
            session: ArrowOutSession {
                started_at_ms: 10_000,
                ..ArrowOutSession::default()
            },
            server_now_ms: 10_900,
            observed_at: Instant::now(),
        };

        let elapsed = estimated_server_elapsed_ms(&active_session);

        assert!((900..5_000).contains(&elapsed));
    }

    #[test]
    fn live_tracker_accumulates_rewards() {
        let log = ui::TaskLog::stdout();
        let tracker = LiveRewardTracker::new("测试箭头逃离", &log);
        tracker.add(0.1);
        tracker.add(0.2);

        assert!((*tracker.total_reward.lock().unwrap() - 0.3).abs() < 0.000001);
    }

    #[test]
    fn state_conflict_detection_accepts_localized_409() {
        let error =
            io::Error::other("请求失败了（状态码 409）：请求状态冲突。可能已有未结束对局。");

        assert!(is_state_conflict_error(&error));
    }
}
