mod log;
mod round;
mod types;

use std::collections::{HashMap, HashSet};
use std::io;
use std::path::Path;
use std::sync::{Arc, Mutex};

use crate::model::{AuthCache, AuthConfig, MazeConfigResponse, MazeMeResponse, MazeStartResponse};
use crate::runtime::resolve_data_file_path;
use crate::ui;
use crate::workflows::common::{
    AccountRewardSummary, AccountRuntime, BatchState, current_unix_ms, ensure_authenticated,
    format_amount, print_account_reward_summary, run_account_task_until_complete,
    with_auth_retry_api_until_success,
};

use self::log::{
    append_account_summary, append_difficulty_summary, append_round_result, append_run_header,
    localized_difficulty, localized_difficulty_list, log_round_result,
};
use self::round::{
    is_active_session_error, is_daily_limit_error, is_pending_session, merge_round_into_summary,
    play_round, should_stop_after_round,
};
use self::types::{MazeDifficultySummary, MazeRoundSummary, RoundProgress};

pub const DONE_MESSAGE: &str = "自动迷宫已完成。";
const TASK_TITLE: &str = "自动迷宫";

#[derive(Debug, Clone)]
pub struct AccountRunOutput {
    pub account: AuthCache,
    pub total_reward: f64,
}

#[derive(Default)]
struct AccountProgressCache {
    summaries: HashMap<String, MazeDifficultySummary>,
    seen_session_ids: HashSet<i32>,
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
        result_log_dir: resolve_data_file_path("log/maze"),
        log: log.clone(),
    }));

    let accounts = config.accounts.clone();
    let base_url = config.base_url.clone();
    log.line_fmt(format_args!(
        "开始{}，本次会处理 {} 个账号。",
        TASK_TITLE,
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
                let mut progress_cache = AccountProgressCache::default();
                let task_log = state.lock().unwrap().log.clone();
                let summaries = run_account_task_until_complete(
                    &cancel_flag,
                    &task_log,
                    TASK_TITLE,
                    &email,
                    || run_account(&cancel_flag, &state, &mut runtime, &mut progress_cache),
                )?;
                Ok(AccountRewardSummary {
                    index,
                    email: summaries
                        .first()
                        .map(|summary| summary.email.clone())
                        .unwrap_or(email),
                    total_reward: summaries.iter().map(|summary| summary.total_reward).sum(),
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
                "{}任务异常退出，请查看前面的账号日志定位原因。",
                TASK_TITLE
            )),
        }
    }
    print_account_reward_summary(log, TASK_TITLE, &reward_summaries);

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
        result_log_dir: resolve_data_file_path("log/maze"),
        log: log.clone(),
    }));
    let mut runtime = AccountRuntime::new(&config.base_url, account);
    let mut progress_cache = AccountProgressCache::default();
    let task_log = state.lock().unwrap().log.clone();
    let email = runtime.email().to_string();
    let summaries =
        run_account_task_until_complete(cancel_flag, &task_log, TASK_TITLE, &email, || {
            run_account(cancel_flag, &state, &mut runtime, &mut progress_cache)
        })?;
    let total_reward = summaries.iter().map(|summary| summary.total_reward).sum();
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
    progress_cache: &mut AccountProgressCache,
) -> io::Result<Vec<MazeDifficultySummary>> {
    ui::check_cancel(cancel_flag)?;
    ensure_authenticated(state, runtime)?;
    append_run_header(
        &state.lock().unwrap().result_log_dir,
        runtime.email(),
        current_unix_ms(),
    )?;

    let config = with_auth_retry_api_until_success(
        cancel_flag,
        state,
        runtime,
        "maze config",
        |client, auth_token| client.get_maze_config(auth_token),
    )?;
    let difficulties = difficulty_order(&config);
    state.lock().unwrap().log.line_fmt(format_args!(
        "账号 {} 已准备好：迷宫包含 {} 难度，最多可同时进行 {} 局，最小操作间隔 {}ms。",
        runtime.email(),
        localized_difficulty_list(&difficulties),
        config.max_active_sessions,
        config.min_interval_ms,
    ));

    let me = fetch_me(cancel_flag, state, runtime)?;
    let mut used_today = me.daily_plays_used.clone();
    let drained = drain_pending_session(cancel_flag, state, runtime, &config, &me)?;
    for result in &drained {
        merge_round_into_cache(progress_cache, runtime.email(), result);
    }
    let mut summaries = progress_cache.summaries.clone();
    if drained.iter().any(should_stop_after_round) {
        let all = summaries.into_values().collect::<Vec<_>>();
        append_account_summary(
            &state.lock().unwrap().result_log_dir,
            runtime.email(),
            current_unix_ms(),
            &all,
        )?;
        return Ok(all);
    }
    let mut all_summaries = Vec::new();

    for difficulty in difficulties {
        ui::check_cancel(cancel_flag)?;
        let mut summary = summaries
            .remove(&difficulty)
            .unwrap_or_else(|| MazeDifficultySummary {
                email: runtime.email().to_string(),
                difficulty: difficulty.clone(),
                ..MazeDifficultySummary::default()
            });
        let used = *used_today.get(&difficulty).unwrap_or(&0);
        let remaining = remaining_from_me(&me, &difficulty);
        summary = run_difficulty(
            cancel_flag,
            state,
            runtime,
            &config,
            DifficultyRunPlan {
                difficulty: difficulty.clone(),
                summary,
                next_round_index: used + 1,
                total_rounds: used + remaining,
                remaining,
            },
            &mut used_today,
            progress_cache,
        )?;
        append_difficulty_summary(&state.lock().unwrap().result_log_dir, &summary)?;
        state.lock().unwrap().log.line_fmt(format_args!(
            "账号 {} 的迷宫{}难度已完成：一共玩了 {} 局，成功 {} 局，失败 {} 局，总收益 {}，今天还剩 {} 次。",
            summary.email,
            localized_difficulty(&summary.difficulty),
            summary.played,
            summary.won,
            summary.failed,
            format_amount(summary.total_reward),
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
        "账号 {} 的{}运行完成。",
        runtime.email(),
        TASK_TITLE
    ));
    Ok(all_summaries)
}

fn drain_pending_session(
    cancel_flag: &ui::CancelFlag,
    state: &Arc<Mutex<BatchState>>,
    runtime: &mut AccountRuntime,
    config: &MazeConfigResponse,
    me: &MazeMeResponse,
) -> io::Result<Vec<MazeRoundSummary>> {
    let mut rounds = Vec::new();
    if let Some(item) = me
        .active_session
        .as_ref()
        .filter(|item| is_pending_session(item))
    {
        ui::check_cancel(cancel_flag)?;
        let used = *me.daily_plays_used.get(&item.difficulty).unwrap_or(&0);
        let remaining = remaining_from_me(me, &item.difficulty);
        let progress = RoundProgress {
            current: used.max(1),
            total: normalize_round_total(used.max(1), used + remaining),
        };
        state.lock().unwrap().log.line_fmt(format_args!(
            "账号 {} 发现迷宫{}难度残局，先继续玩完今天第 {}/{} 局（对局 {}）。",
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
            config,
            item.clone(),
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

struct DifficultyRunPlan {
    difficulty: String,
    summary: MazeDifficultySummary,
    next_round_index: i32,
    total_rounds: i32,
    remaining: i32,
}

fn run_difficulty(
    cancel_flag: &ui::CancelFlag,
    state: &Arc<Mutex<BatchState>>,
    runtime: &mut AccountRuntime,
    config: &MazeConfigResponse,
    plan: DifficultyRunPlan,
    used_today: &mut HashMap<String, i32>,
    progress_cache: &mut AccountProgressCache,
) -> io::Result<MazeDifficultySummary> {
    let DifficultyRunPlan {
        difficulty,
        mut summary,
        next_round_index,
        total_rounds,
        remaining,
    } = plan;
    let difficulty = difficulty.as_str();
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
            "账号 {} 开始玩迷宫{}难度，今天第 {}/{} 局。",
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
                Err(error)
                    if is_new_round_unavailable_error(&error.to_string())
                        || is_active_session_error(&error.to_string()) =>
                {
                    let refreshed = fetch_me(cancel_flag, state, runtime)?;
                    *used_today = refreshed.daily_plays_used.clone();
                    current_remaining = remaining_from_me(&refreshed, difficulty);
                    summary.remaining_after = current_remaining;
                    if current_remaining <= 0 {
                        summary.when_unix_ms = current_unix_ms();
                        return Ok(summary);
                    }
                    let Some(item) = refreshed
                        .active_session
                        .as_ref()
                        .filter(|item| is_pending_session(item))
                        .cloned()
                    else {
                        return Err(error);
                    };
                    let pending_used = *used_today.get(&item.difficulty).unwrap_or(&0);
                    let pending_remaining = remaining_from_me(&refreshed, &item.difficulty);
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
                        config,
                        item.clone(),
                        true,
                        pending_progress,
                    )?;
                    result.remaining_after = pending_remaining;
                    if item.difficulty == difficulty {
                        current_remaining = pending_remaining;
                        result.remaining_after = current_remaining;
                    }
                    append_round_result(&state.lock().unwrap().result_log_dir, &result)?;
                    log_round_result(&state.lock().unwrap().log, &result);
                    merge_round_into_cache(progress_cache, runtime.email(), &result);
                    if item.difficulty == difficulty {
                        merge_round_into_summary(&mut summary, &result);
                        used_today.insert(difficulty.to_string(), current_round_index);
                    }
                    if should_stop_after_round(&result) {
                        return Ok(summary);
                    }
                    if item.difficulty == difficulty {
                        continue 'rounds;
                    }
                }
                Err(error) => return Err(error),
            }
        };
        current_remaining = start
            .daily_plays_remaining
            .get(difficulty)
            .copied()
            .unwrap_or_else(|| current_remaining.saturating_sub(1));
        let mut result = play_round(
            cancel_flag,
            state,
            runtime,
            config,
            start.session,
            false,
            progress,
        )?;
        result.remaining_after = current_remaining;
        append_round_result(&state.lock().unwrap().result_log_dir, &result)?;
        log_round_result(&state.lock().unwrap().log, &result);
        merge_round_into_cache(progress_cache, runtime.email(), &result);
        merge_round_into_summary(&mut summary, &result);
        used_today.insert(difficulty.to_string(), current_round_index);
        if should_stop_after_round(&result) {
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
) -> io::Result<MazeStartResponse> {
    let result = with_auth_retry_api_until_success(
        cancel_flag,
        state,
        runtime,
        "maze start",
        |client, auth_token| client.start_maze(auth_token, difficulty),
    );
    match result {
        Ok(start) if !start.ok => Err(io::Error::other(
            "new_round_unavailable: start api returned ok=false",
        )),
        Ok(start) if start.session.session_id <= 0 => Err(io::Error::other(
            "new_round_unavailable: start api returned invalid session",
        )),
        Ok(start) => Ok(start),
        Err(error) => Err(error),
    }
}

fn fetch_me(
    cancel_flag: &ui::CancelFlag,
    state: &Arc<Mutex<BatchState>>,
    runtime: &mut AccountRuntime,
) -> io::Result<MazeMeResponse> {
    with_auth_retry_api_until_success(
        cancel_flag,
        state,
        runtime,
        "maze me",
        |client, auth_token| client.get_maze_me(auth_token),
    )
}

fn remaining_from_me(me: &MazeMeResponse, difficulty: &str) -> i32 {
    me.daily_plays_remaining
        .get(difficulty)
        .copied()
        .unwrap_or(0)
        .max(0)
}

fn merge_round_into_cache(
    cache: &mut AccountProgressCache,
    email: &str,
    result: &MazeRoundSummary,
) {
    if result.session_id > 0 && !cache.seen_session_ids.insert(result.session_id) {
        return;
    }
    let entry = cache
        .summaries
        .entry(result.difficulty.clone())
        .or_insert_with(|| MazeDifficultySummary {
            email: email.to_string(),
            difficulty: result.difficulty.clone(),
            ..MazeDifficultySummary::default()
        });
    merge_round_into_summary(entry, result);
}

fn difficulty_order(config: &MazeConfigResponse) -> Vec<String> {
    let mut ordered = Vec::new();
    for difficulty in ["easy", "normal", "hard"] {
        if config.difficulties.contains_key(difficulty) {
            ordered.push(difficulty.to_string());
        }
    }
    let seen = ordered.iter().cloned().collect::<HashSet<_>>();
    let mut extra = config
        .difficulties
        .keys()
        .filter(|difficulty| !seen.contains(*difficulty))
        .cloned()
        .collect::<Vec<_>>();
    extra.sort();
    ordered.extend(extra);
    ordered
}

fn normalize_round_total(current: i32, total: i32) -> i32 {
    total.max(current.max(1))
}

fn is_new_round_unavailable_error(message: &str) -> bool {
    message
        .to_ascii_lowercase()
        .contains("new_round_unavailable")
}
