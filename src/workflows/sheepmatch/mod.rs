mod auth;
mod log;
mod round;
mod snapshot;
#[cfg(test)]
mod tests;

use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::api::ApiClient;
use crate::model::{
    AccountRunSummary, AuthCache, AuthConfig, ConfigResponse, DIFFICULTY_ORDER, RoundResultSummary,
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
    RoundProgress, next_round_index_for_new_round, normalize_round_total, play_round,
    remaining_plays, summarize_rounds_by_difficulty, total_round_count,
};
use self::snapshot::history_item_to_start_response;

pub const DONE_MESSAGE: &str = "自动羊了个羊处理完成。";

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
        result_log_dir: resolve_data_file_path("log/sheepmatch"),
        log: log.clone(),
    }));

    let accounts = config.accounts.clone();
    let base_url = config.base_url.clone();
    log.line_fmt(format_args!(
        "开始自动羊了个羊，本次会处理 {} 个账号，包含 {} 难度。",
        accounts.len(),
        localized_difficulty_list(DIFFICULTY_ORDER)
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
                    state.lock().unwrap().log.line_fmt(format_args!(
                        "账号 {} 自动羊了个羊运行失败：{}",
                        email, error
                    ));
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
                .line("自动羊了个羊线程提前结束：后台线程发生了未处理异常。"),
        }
    }

    Ok(state.lock().unwrap().config.clone())
}

pub fn run_account_for_free_play(
    config: &AuthConfig,
    account: AuthCache,
    cancel_flag: &ui::CancelFlag,
) -> io::Result<AccountRunOutput> {
    run_account_for_free_play_with_log(config, account, cancel_flag, &ui::TaskLog::stdout())
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
        result_log_dir: resolve_data_file_path("log/sheepmatch"),
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
) -> io::Result<Vec<AccountRunSummary>> {
    ui::check_cancel(cancel_flag)?;
    ensure_authenticated(state, runtime)?;
    append_run_header(
        &state.lock().unwrap().result_log_dir,
        runtime.email(),
        current_unix_ms(),
    )?;

    let config = with_auth_retry(state, runtime, |client, auth_token| {
        client.get_tile_config(auth_token)
    })?;
    state.lock().unwrap().log.line_fmt(format_args!(
        "账号 {} 已准备好：槽位上限={}，最多可同时进行 {} 局，最小操作间隔={}ms。",
        runtime.email(),
        config.slot_limit,
        config.max_active_sessions,
        config.min_interval_ms,
    ));

    let me = with_auth_retry(state, runtime, |client, auth_token| {
        client.get_tile_me(auth_token)
    })?;
    let mut used_today_by_difficulty = me.daily_plays_used.clone();
    let mut remaining_by_difficulty = me.daily_plays_remaining.clone();

    let drained_rounds = drain_pending_sessions(
        cancel_flag,
        state,
        runtime,
        &config,
        &used_today_by_difficulty,
        &remaining_by_difficulty,
    )?;
    let base_stats = summarize_rounds_by_difficulty(runtime.email(), &drained_rounds);
    let mut visited = std::collections::HashSet::new();
    let mut all_summaries = Vec::new();

    for difficulty in DIFFICULTY_ORDER {
        ui::check_cancel(cancel_flag)?;
        let seed = base_stats
            .get(*difficulty)
            .cloned()
            .unwrap_or_else(|| AccountRunSummary {
                email: runtime.email().to_string(),
                difficulty: (*difficulty).to_string(),
                ..AccountRunSummary::default()
            });
        let next_round = next_round_index_for_new_round(
            *used_today_by_difficulty.get(*difficulty).unwrap_or(&0),
        );
        let total_rounds = total_round_count(
            *used_today_by_difficulty.get(*difficulty).unwrap_or(&0),
            *remaining_by_difficulty.get(*difficulty).unwrap_or(&0),
        );
        let summary = run_difficulty(
            cancel_flag,
            state,
            runtime,
            &config,
            difficulty,
            seed,
            next_round,
            total_rounds,
            &mut used_today_by_difficulty,
            &mut remaining_by_difficulty,
        )?;
        append_difficulty_summary(&state.lock().unwrap().result_log_dir, &summary)?;
        state.lock().unwrap().log.line_fmt(format_args!(
            "账号 {} 的{}难度已完成：一共玩了 {} 局，成功 {} 局，放弃 {} 局，失败 {} 局，总收益 {:.8}，今天还剩 {} 次。",
            summary.email,
            localized_difficulty(&summary.difficulty),
            summary.played,
            summary.won,
            summary.abandoned,
            summary.failed,
            summary.total_reward,
            summary.remaining_after,
        ));
        visited.insert((*difficulty).to_string());
        all_summaries.push(summary);
    }

    for (difficulty, summary) in base_stats {
        if !visited.contains(&difficulty) {
            all_summaries.push(summary);
        }
    }

    append_account_summary(
        &state.lock().unwrap().result_log_dir,
        runtime.email(),
        current_unix_ms(),
        &all_summaries,
    )?;
    state
        .lock()
        .unwrap()
        .log
        .line_fmt(format_args!("账号 {} 运行完成。", runtime.email()));
    Ok(all_summaries)
}

fn drain_pending_sessions(
    cancel_flag: &ui::CancelFlag,
    state: &Arc<Mutex<BatchState>>,
    runtime: &mut AccountRuntime,
    config: &ConfigResponse,
    used_today_by_difficulty: &HashMap<String, i32>,
    remaining_by_difficulty: &HashMap<String, i32>,
) -> io::Result<Vec<RoundResultSummary>> {
    ui::check_cancel(cancel_flag)?;
    let history = with_auth_retry(state, runtime, |client, auth_token| {
        client.get_tile_history(auth_token)
    })?;
    let mut rounds = Vec::new();
    for item in history.items {
        if !item.status.trim().eq_ignore_ascii_case("pending") {
            continue;
        }
        ui::check_cancel(cancel_flag)?;
        let round_index = next_round_index_for_new_round(
            *used_today_by_difficulty.get(&item.difficulty).unwrap_or(&0),
        );
        let round_total = normalize_round_total(
            round_index,
            total_round_count(
                *used_today_by_difficulty.get(&item.difficulty).unwrap_or(&0),
                *remaining_by_difficulty.get(&item.difficulty).unwrap_or(&0),
            ),
        );
        state.lock().unwrap().log.line_fmt(format_args!(
            "账号 {} 继续{}难度，{}（对局 {}）。",
            runtime.email(),
            localized_difficulty(&item.difficulty),
            format_round_progress(round_index, round_total),
            item.session_id,
        ));
        let start = history_item_to_start_response(&item);
        let result = play_round(
            cancel_flag,
            state,
            runtime,
            config,
            &start,
            true,
            RoundProgress {
                current: round_index,
                total: round_total,
            },
        )?;
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
    config: &ConfigResponse,
    difficulty: &str,
    mut summary: AccountRunSummary,
    next_round_index: i32,
    total_rounds: i32,
    used_today_by_difficulty: &mut HashMap<String, i32>,
    remaining_by_difficulty: &mut HashMap<String, i32>,
) -> io::Result<AccountRunSummary> {
    if summary.email.trim().is_empty() {
        summary.email = runtime.email().to_string();
        summary.difficulty = difficulty.to_string();
    }

    let remaining = remaining_plays(state, runtime, difficulty)?;
    remaining_by_difficulty.insert(difficulty.to_string(), remaining);
    if remaining <= 0 {
        summary.remaining_after = 0;
        if summary.when_unix_ms == 0 {
            summary.when_unix_ms = current_unix_ms();
        }
        return Ok(summary);
    }

    let total_rounds = normalize_round_total(next_round_index, total_rounds);
    for played in 0..remaining {
        ui::check_cancel(cancel_flag)?;
        let progress = RoundProgress {
            current: next_round_index + played,
            total: total_rounds,
        };
        state.lock().unwrap().log.line_fmt(format_args!(
            "账号 {} 开始玩{}难度，{}。",
            runtime.email(),
            localized_difficulty(difficulty),
            format_round_progress(progress.current, progress.total),
        ));
        let start = with_auth_retry(state, runtime, |client, auth_token| {
            client.start_game(auth_token, difficulty)
        })?;
        let result = play_round(cancel_flag, state, runtime, config, &start, false, progress)?;
        append_round_result(&state.lock().unwrap().result_log_dir, &result)?;
        log_round_result(&state.lock().unwrap().log, &result);
        self::round::merge_round_into_summary(&mut summary, &result);
        used_today_by_difficulty.insert(difficulty.to_string(), progress.current);
        remaining_by_difficulty.insert(difficulty.to_string(), result.remaining_after);
        if !result.error_message.trim().is_empty() {
            return Ok(summary);
        }
    }
    Ok(summary)
}

fn format_round_progress(current: i32, total: i32) -> String {
    format!(
        "今天第 {}/{} 局",
        current.max(1),
        normalize_round_total(current, total)
    )
}
