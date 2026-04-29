mod auth;
mod log;
mod round;
mod types;

use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::api::ApiClient;
use crate::model::{
    AuthCache, AuthConfig, SudokuConfigResponse, SudokuHistoryResponse, SudokuStartResponse,
};
use crate::runtime::resolve_data_file_path;
use crate::storage::{save_cache, upsert_account};
use crate::ui;

use self::auth::{ensure_authenticated, with_auth_retry};
use self::log::{
    append_account_summary, append_difficulty_summary, append_round_result, append_run_header,
    localized_difficulty, localized_difficulty_list, log_round_result,
};
use self::round::{
    difficulty_order, is_active_session_error, is_daily_limit_error, is_pending_session,
    merge_round_into_summary, normalize_round_total, play_round, remaining_for_difficulty,
    snapshot_from_history_item, snapshot_from_start_response, used_today_by_difficulty,
};
use self::types::{RoundProgress, SudokuDifficultySummary, SudokuRoundSummary};

pub const DONE_MESSAGE: &str = "自动数独处理完成。";

#[derive(Debug, Clone)]
pub struct AccountRunOutput {
    pub account: AuthCache,
}

#[derive(Debug)]
pub struct BatchState {
    pub config: AuthConfig,
    pub auth_cache_file: Option<PathBuf>,
    pub result_log_dir: PathBuf,
    pub log: ui::TaskLog,
}

impl BatchState {
    pub fn save_account(&mut self, account: AuthCache) -> io::Result<()> {
        self.config = upsert_account(self.config.clone(), account);
        if let Some(path) = &self.auth_cache_file {
            save_cache(path, self.config.clone())
        } else {
            Ok(())
        }
    }
}

#[derive(Debug)]
struct AccountRuntime {
    api_client: ApiClient,
    account: AuthCache,
    auth_token: String,
}

impl AccountRuntime {
    fn email(&self) -> &str {
        self.account.email.trim()
    }
}

fn current_unix_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(i64::MAX as u128) as i64)
        .unwrap_or(0)
}

pub fn run_batch(
    config: AuthConfig,
    auth_cache_file: impl AsRef<Path>,
    cancel_flag: &ui::CancelFlag,
    log: &ui::TaskLog,
) -> io::Result<AuthConfig> {
    if config.accounts.is_empty() {
        println!("当前还没有可用账号。");
        return Ok(config);
    }

    let state = Arc::new(Mutex::new(BatchState {
        config: config.clone(),
        auth_cache_file: Some(auth_cache_file.as_ref().to_path_buf()),
        result_log_dir: resolve_data_file_path("log/sudoku"),
        log: log.clone(),
    }));

    let accounts = config.accounts.clone();
    let base_url = config.base_url.clone();
    log.line_fmt(format_args!(
        "开始自动数独，本次会处理 {} 个账号。",
        accounts.len()
    ));

    let mut handles = Vec::with_capacity(accounts.len());
    for account in accounts {
        ui::check_cancel(cancel_flag)?;
        let state = Arc::clone(&state);
        let cancel_flag = Arc::clone(cancel_flag);
        let base_url = base_url.clone();
        handles.push(std::thread::spawn(move || {
            let mut runtime = AccountRuntime {
                api_client: ApiClient::new(&base_url),
                account,
                auth_token: String::new(),
            };
            let email = runtime.email().to_string();
            match run_account(&cancel_flag, &state, &mut runtime) {
                Ok(_) => Ok(()),
                Err(error) if error.kind() == io::ErrorKind::Interrupted => Err(error),
                Err(error) => {
                    state
                        .lock()
                        .unwrap()
                        .log
                        .line_fmt(format_args!("账号 {} 自动数独运行失败：{}", email, error));
                    Ok(())
                }
            }
        }));
    }

    for handle in handles {
        match handle.join() {
            Ok(Ok(())) => {}
            Ok(Err(error)) if error.kind() == io::ErrorKind::Interrupted => return Err(error),
            Ok(Err(error)) => return Err(error),
            Err(_) => state
                .lock()
                .unwrap()
                .log
                .line("自动数独线程提前结束：后台线程发生了未处理异常。"),
        }
    }

    Ok(state.lock().unwrap().config.clone())
}

pub fn run_account_for_free_play_with_log(
    config: &AuthConfig,
    account: AuthCache,
    cancel_flag: &ui::CancelFlag,
    log: &ui::TaskLog,
) -> io::Result<AccountRunOutput> {
    let fallback_account = account.clone();
    let state = Arc::new(Mutex::new(BatchState {
        config: AuthConfig {
            base_url: config.base_url.clone(),
            accounts: vec![account.clone()],
        },
        auth_cache_file: None,
        result_log_dir: resolve_data_file_path("log/sudoku"),
        log: log.clone(),
    }));
    let mut runtime = AccountRuntime {
        api_client: ApiClient::new(&config.base_url),
        account,
        auth_token: String::new(),
    };
    let _summaries = run_account(cancel_flag, &state, &mut runtime)?;
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
    })
}

fn run_account(
    cancel_flag: &ui::CancelFlag,
    state: &Arc<Mutex<BatchState>>,
    runtime: &mut AccountRuntime,
) -> io::Result<Vec<SudokuDifficultySummary>> {
    ui::check_cancel(cancel_flag)?;
    ensure_authenticated(state, runtime)?;
    append_run_header(
        &state.lock().unwrap().result_log_dir,
        runtime.email(),
        current_unix_ms(),
    )?;

    let config = with_auth_retry(state, runtime, |client, auth_token| {
        client.get_sudoku_config(auth_token)
    })?;
    let difficulties = difficulty_order(&config);
    state.lock().unwrap().log.line_fmt(format_args!(
        "账号 {} 已准备好：数独包含 {} 难度，最多可同时进行 {} 局，最小操作间隔 {}ms。",
        runtime.email(),
        localized_difficulty_list(&difficulties),
        config.max_active_sessions,
        config.min_interval_ms,
    ));

    let history = fetch_history(state, runtime)?;
    let mut used_today = used_today_by_difficulty(&history);
    let drained = drain_pending_sessions(cancel_flag, state, runtime, &config, &history)?;
    let mut summaries = summarize_rounds_by_difficulty(runtime.email(), &drained);
    let mut all_summaries = Vec::new();

    for difficulty in difficulties {
        ui::check_cancel(cancel_flag)?;
        let mut summary =
            summaries
                .remove(&difficulty)
                .unwrap_or_else(|| SudokuDifficultySummary {
                    email: runtime.email().to_string(),
                    difficulty: difficulty.clone(),
                    ..SudokuDifficultySummary::default()
                });
        let used = *used_today.get(&difficulty).unwrap_or(&0);
        let remaining = remaining_for_difficulty(&config, &difficulty, used);
        summary = run_difficulty(
            cancel_flag,
            state,
            runtime,
            &config,
            &difficulty,
            summary,
            used + 1,
            used + remaining,
            remaining,
            &mut used_today,
        )?;
        append_difficulty_summary(&state.lock().unwrap().result_log_dir, &summary)?;
        state.lock().unwrap().log.line_fmt(format_args!(
            "账号 {} 的数独{}难度已完成：一共玩了 {} 局，成功 {} 局，失败 {} 局，总收益 {:.8}，今天还剩 {} 次。",
            summary.email,
            localized_difficulty(&summary.difficulty),
            summary.played,
            summary.won,
            summary.failed,
            summary.total_reward,
            summary.remaining_after,
        ));
        all_summaries.push(summary);
    }

    for (_, summary) in summaries {
        all_summaries.push(summary);
    }
    append_account_summary(
        &state.lock().unwrap().result_log_dir,
        runtime.email(),
        current_unix_ms(),
        &all_summaries,
    )?;
    state.lock().unwrap().log.line_fmt(format_args!(
        "账号 {} 的自动数独运行完成。",
        runtime.email()
    ));
    Ok(all_summaries)
}

fn drain_pending_sessions(
    cancel_flag: &ui::CancelFlag,
    state: &Arc<Mutex<BatchState>>,
    runtime: &mut AccountRuntime,
    config: &SudokuConfigResponse,
    history: &SudokuHistoryResponse,
) -> io::Result<Vec<SudokuRoundSummary>> {
    let mut rounds = Vec::new();
    let used_today = used_today_by_difficulty(history);
    for item in history.items.iter().filter(|item| is_pending_session(item)) {
        ui::check_cancel(cancel_flag)?;
        let used = *used_today.get(&item.difficulty).unwrap_or(&0);
        let remaining = remaining_for_difficulty(config, &item.difficulty, used);
        let progress = RoundProgress {
            current: used.max(1),
            total: normalize_round_total(used.max(1), used + remaining),
        };
        state.lock().unwrap().log.line_fmt(format_args!(
            "账号 {} 发现数独{}难度残局，先继续玩完今天第 {}/{} 局（对局 {}）。",
            runtime.email(),
            localized_difficulty(&item.difficulty),
            progress.current,
            progress.total,
            item.session_id,
        ));
        let mut result = play_round(
            cancel_flag,
            state,
            runtime,
            snapshot_from_history_item(item, config),
            true,
            progress,
        )?;
        result.remaining_after = remaining;
        append_round_result(&state.lock().unwrap().result_log_dir, &result)?;
        log_round_result(&state.lock().unwrap().log, &result);
        rounds.push(result);
    }
    Ok(rounds)
}

fn run_difficulty(
    cancel_flag: &ui::CancelFlag,
    state: &Arc<Mutex<BatchState>>,
    runtime: &mut AccountRuntime,
    config: &SudokuConfigResponse,
    difficulty: &str,
    mut summary: SudokuDifficultySummary,
    next_round_index: i32,
    total_rounds: i32,
    remaining: i32,
    used_today: &mut HashMap<String, i32>,
) -> io::Result<SudokuDifficultySummary> {
    if summary.email.trim().is_empty() {
        summary.email = runtime.email().to_string();
        summary.difficulty = difficulty.to_string();
    }
    summary.remaining_after = remaining;
    if remaining <= 0 {
        if summary.when_unix_ms == 0 {
            summary.when_unix_ms = current_unix_ms();
        }
        return Ok(summary);
    }

    let mut current_remaining = remaining;
    'rounds: for played in 0..remaining {
        ui::check_cancel(cancel_flag)?;
        let current_round_index = next_round_index + played;
        if current_round_index <= *used_today.get(difficulty).unwrap_or(&0) {
            continue;
        }
        let progress = RoundProgress {
            current: current_round_index,
            total: normalize_round_total(current_round_index, total_rounds),
        };
        state.lock().unwrap().log.line_fmt(format_args!(
            "账号 {} 开始玩数独{}难度，今天第 {}/{} 局。",
            runtime.email(),
            localized_difficulty(difficulty),
            progress.current,
            progress.total,
        ));
        let start = loop {
            match start_new_round(cancel_flag, state, runtime, difficulty) {
                Ok(start) => break start,
                Err(error) if is_daily_limit_error(&error.to_string()) => {
                    summary.remaining_after = 0;
                    summary.when_unix_ms = current_unix_ms();
                    return Ok(summary);
                }
                Err(error) if is_start_failed_error(&error.to_string()) => {
                    summary.error_message = error.to_string();
                    summary.when_unix_ms = current_unix_ms();
                    return Ok(summary);
                }
                Err(error) if is_active_session_error(&error.to_string()) => {
                    let history = fetch_history(state, runtime)?;
                    *used_today = used_today_by_difficulty(&history);
                    let Some(item) = history
                        .items
                        .iter()
                        .find(|item| is_pending_session(item))
                        .cloned()
                    else {
                        return Err(error);
                    };
                    state.lock().unwrap().log.line_fmt(format_args!(
                        "账号 {} 检测到{}难度残局（对局 {}），先把残局玩完。",
                        runtime.email(),
                        localized_difficulty(&item.difficulty),
                        item.session_id,
                    ));
                    let pending_used = *used_today.get(&item.difficulty).unwrap_or(&0);
                    let pending_remaining =
                        remaining_for_difficulty(config, &item.difficulty, pending_used);
                    let pending_progress = RoundProgress {
                        current: pending_used.max(1),
                        total: normalize_round_total(
                            pending_used.max(1),
                            pending_used + pending_remaining,
                        ),
                    };
                    let mut result = play_round(
                        cancel_flag,
                        state,
                        runtime,
                        snapshot_from_history_item(&item, config),
                        true,
                        pending_progress,
                    )?;
                    result.remaining_after = pending_remaining;
                    if item.difficulty == difficulty {
                        current_remaining = pending_remaining;
                    }
                    append_round_result(&state.lock().unwrap().result_log_dir, &result)?;
                    log_round_result(&state.lock().unwrap().log, &result);
                    if item.difficulty == difficulty {
                        merge_round_into_summary(&mut summary, &result);
                        used_today.insert(difficulty.to_string(), current_round_index);
                        if should_stop_after_round_error(&result.error_message) {
                            return Ok(summary);
                        }
                        continue 'rounds;
                    }
                }
                Err(error) => return Err(error),
            }
        };
        current_remaining = current_remaining.saturating_sub(1);
        let mut result = play_round(
            cancel_flag,
            state,
            runtime,
            snapshot_from_start_response(&start, config),
            false,
            progress,
        )?;
        result.remaining_after = current_remaining;
        append_round_result(&state.lock().unwrap().result_log_dir, &result)?;
        log_round_result(&state.lock().unwrap().log, &result);
        merge_round_into_summary(&mut summary, &result);
        used_today.insert(difficulty.to_string(), current_round_index);
        if should_stop_after_round_error(&result.error_message) {
            return Ok(summary);
        }
    }
    Ok(summary)
}

fn start_new_round(
    cancel_flag: &ui::CancelFlag,
    state: &Arc<Mutex<BatchState>>,
    runtime: &mut AccountRuntime,
    difficulty: &str,
) -> io::Result<SudokuStartResponse> {
    loop {
        ui::check_cancel(cancel_flag)?;
        let result = with_auth_retry(state, runtime, |client, auth_token| {
            client.start_sudoku(auth_token, difficulty)
        });
        match result {
            Ok(start) if !start.ok => {
                return Err(io::Error::other("start_failed: 数独开局接口返回 ok=false"));
            }
            Ok(start) if start.session_id <= 0 || start.givens.is_empty() => {
                return Err(io::Error::other(
                    "start_failed: 数独开局接口返回的数据缺少有效对局",
                ));
            }
            Ok(start) => return Ok(start),
            Err(error) if is_retryable_start_error(&error.to_string()) => {
                ui::sleep_with_cancel(cancel_flag, std::time::Duration::from_millis(500))?;
                continue;
            }
            Err(error) => return Err(error),
        }
    }
}

fn fetch_history(
    state: &Arc<Mutex<BatchState>>,
    runtime: &mut AccountRuntime,
) -> io::Result<SudokuHistoryResponse> {
    with_auth_retry(state, runtime, |client, auth_token| {
        client.get_sudoku_history(auth_token)
    })
}

fn summarize_rounds_by_difficulty(
    email: &str,
    rounds: &[SudokuRoundSummary],
) -> HashMap<String, SudokuDifficultySummary> {
    let mut stats = HashMap::new();
    for round in rounds {
        let entry =
            stats
                .entry(round.difficulty.clone())
                .or_insert_with(|| SudokuDifficultySummary {
                    email: email.to_string(),
                    difficulty: round.difficulty.clone(),
                    ..SudokuDifficultySummary::default()
                });
        merge_round_into_summary(entry, round);
    }
    stats
}

fn is_retryable_start_error(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    lower.contains("timeout") || lower.contains("temporarily")
}

fn is_start_failed_error(message: &str) -> bool {
    message.to_ascii_lowercase().contains("start_failed")
}

fn should_stop_after_round_error(error_message: &str) -> bool {
    !error_message.trim().is_empty()
}
