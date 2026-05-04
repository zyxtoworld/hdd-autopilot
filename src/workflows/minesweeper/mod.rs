mod log;
mod round;
mod types;

use std::collections::{HashMap, HashSet};
use std::io;
use std::path::Path;
use std::sync::{Arc, Mutex};

use crate::model::{
    AuthCache, AuthConfig, MinesweeperConfigResponse, MinesweeperMeResponse,
    MinesweeperStartResponse,
};
use crate::runtime::resolve_data_file_path;
use crate::ui;
use crate::workflows::common::{
    AccountRewardSummary, AccountRuntime, BatchState, current_unix_ms, ensure_authenticated,
    format_amount, print_account_reward_summary, run_account_task_until_complete,
    with_auth_retry_api_until_success,
};

use self::log::{
    append_account_summary, append_difficulty_summary, append_round_result, append_run_header,
    localized_difficulty, localized_difficulty_list, log_round_result, remaining_after_clause,
};
use self::round::{
    RoundPlayContext, difficulty_order, is_active_session_error, is_daily_limit_error,
    is_pending_session, merge_round_into_summary, play_round, snapshot_from_session,
    snapshot_from_start_response,
};
use self::types::{MinesweeperDifficultySummary, MinesweeperRoundSummary, RoundProgress};

pub const DONE_MESSAGE: &str = "自动扫雷已完成。";
const UNKNOWN_REMAINING_AFTER: i32 = -1;
const UNKNOWN_ROUND_TOTAL: i32 = 0;

#[derive(Debug, Clone)]
pub struct AccountRunOutput {
    pub account: AuthCache,
    pub total_reward: f64,
}

#[derive(Default)]
struct AccountProgressCache {
    summaries: HashMap<String, MinesweeperDifficultySummary>,
    seen_play_ids: HashSet<i32>,
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
        result_log_dir: resolve_data_file_path("log/minesweeper"),
        log: log.clone(),
    }));

    let accounts = config.accounts.clone();
    let base_url = config.base_url.clone();
    log.line_fmt(format_args!(
        "开始自动扫雷，本次会处理 {} 个账号。",
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
                    "自动扫雷",
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
            Err(_) => state
                .lock()
                .unwrap()
                .log
                .line("自动扫雷任务异常退出，请查看前面的账号日志定位原因。"),
        }
    }
    print_account_reward_summary(log, "自动扫雷", &reward_summaries);

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
        result_log_dir: resolve_data_file_path("log/minesweeper"),
        log: log.clone(),
    }));
    let mut runtime = AccountRuntime::new(&config.base_url, account);
    let mut progress_cache = AccountProgressCache::default();
    let task_log = state.lock().unwrap().log.clone();
    let email = runtime.email().to_string();
    let summaries =
        run_account_task_until_complete(cancel_flag, &task_log, "自动扫雷", &email, || {
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
) -> io::Result<Vec<MinesweeperDifficultySummary>> {
    ui::check_cancel(cancel_flag)?;
    ensure_authenticated(state, runtime)?;
    append_run_header(
        &state.lock().unwrap().result_log_dir,
        runtime.email(),
        current_unix_ms(),
    )?;

    let config = fetch_config(cancel_flag, state, runtime)?;
    let difficulties = difficulty_order(&config);
    state.lock().unwrap().log.line_fmt(format_args!(
        "账号 {} 已准备好：扫雷包含 {} 难度，最小操作间隔 {}ms；新局会按奖励从高到低优先运行，直到接口提示不能继续开局。",
        runtime.email(),
        localized_difficulty_list(&difficulties),
        config.min_interval_ms,
    ));

    let me = fetch_me(cancel_flag, state, runtime)?;
    let mut summaries = progress_cache.summaries.clone();
    let mut local_round_count = total_played(&summaries);
    let mut remaining_by_difficulty = me.daily_plays_remaining.clone();

    if let Some(active) = me
        .active_round
        .as_ref()
        .filter(|item| is_pending_session(item))
        .cloned()
    {
        let next_round_index = local_round_count + 1;
        let result = drain_active_round(
            cancel_flag,
            state,
            runtime,
            &config,
            &active,
            RoundProgress {
                current: next_round_index,
                total: UNKNOWN_ROUND_TOTAL,
            },
            remaining_after_for_difficulty(&remaining_by_difficulty, &active.difficulty),
        )?;
        if merge_round_into_cache(progress_cache, runtime.email(), &result) {
            merge_round_into_summaries(&mut summaries, runtime.email(), &result);
            local_round_count += 1;
        }
    }

    let mut unavailable_difficulties = HashSet::new();
    loop {
        ui::check_cancel(cancel_flag)?;
        let Some(difficulty) = choose_next_difficulty(
            &difficulties,
            &unavailable_difficulties,
            &remaining_by_difficulty,
        ) else {
            state.lock().unwrap().log.line_fmt(format_args!(
                "账号 {} 的扫雷所有难度都暂时不能开局，停止扫雷新局。",
                runtime.email()
            ));
            break;
        };
        let next_round_index = local_round_count + 1;
        state.lock().unwrap().log.line_fmt(format_args!(
            "账号 {} 开始玩扫雷{}难度，今天第 {} 局。",
            runtime.email(),
            localized_difficulty(&difficulty),
            next_round_index,
        ));
        let start = match start_new_round(cancel_flag, state, runtime, &difficulty) {
            Ok(start) => start,
            Err(error) if is_difficulty_unavailable_error(&error.to_string()) => {
                unavailable_difficulties.insert(difficulty.clone());
                remaining_by_difficulty.insert(difficulty.clone(), 0);
                state.lock().unwrap().log.line_fmt(format_args!(
                    "账号 {} 的扫雷{}难度暂时不能开局：{}。会继续尝试下一个奖励较低的难度。",
                    runtime.email(),
                    localized_difficulty(&difficulty),
                    error,
                ));
                continue;
            }
            Err(error) if is_active_session_error(&error.to_string()) => {
                let refreshed = fetch_me(cancel_flag, state, runtime)?;
                if !refreshed.daily_plays_remaining.is_empty() {
                    remaining_by_difficulty = refreshed.daily_plays_remaining.clone();
                }
                let Some(active) = refreshed
                    .active_round
                    .as_ref()
                    .filter(|item| is_pending_session(item))
                    .cloned()
                else {
                    return Err(error);
                };
                let result = drain_active_round(
                    cancel_flag,
                    state,
                    runtime,
                    &config,
                    &active,
                    RoundProgress {
                        current: next_round_index,
                        total: UNKNOWN_ROUND_TOTAL,
                    },
                    remaining_after_for_difficulty(&remaining_by_difficulty, &active.difficulty),
                )?;
                if merge_round_into_cache(progress_cache, runtime.email(), &result) {
                    merge_round_into_summaries(&mut summaries, runtime.email(), &result);
                    local_round_count += 1;
                }
                continue;
            }
            Err(error) => return Err(error),
        };
        let snapshot = snapshot_from_start_response(&start)
            .map_err(|error| io::Error::other(format!("扫雷开局数据无效：{}", error)))?;
        let remaining_after =
            update_remaining_after_start(&mut remaining_by_difficulty, &difficulty, &start);
        let result = play_round(
            cancel_flag,
            state,
            runtime,
            &config,
            snapshot,
            RoundPlayContext {
                continued: false,
                progress: RoundProgress {
                    current: next_round_index,
                    total: UNKNOWN_ROUND_TOTAL,
                },
                remaining_after,
            },
        )?;
        append_round_result(&state.lock().unwrap().result_log_dir, &result)?;
        log_round_result(&state.lock().unwrap().log, &result);
        if merge_round_into_cache(progress_cache, runtime.email(), &result) {
            merge_round_into_summaries(&mut summaries, runtime.email(), &result);
            local_round_count += 1;
        }
    }

    for difficulty in difficulties {
        let final_remaining = remaining_after_for_difficulty(&remaining_by_difficulty, &difficulty);
        let mut summary = summaries
            .entry(difficulty.clone())
            .or_insert_with(|| MinesweeperDifficultySummary {
                email: runtime.email().to_string(),
                difficulty: difficulty.clone(),
                remaining_after: final_remaining,
                when_unix_ms: current_unix_ms(),
                ..MinesweeperDifficultySummary::default()
            })
            .clone();
        summary.remaining_after = final_remaining;
        summaries.insert(difficulty.clone(), summary.clone());
        append_difficulty_summary(&state.lock().unwrap().result_log_dir, &summary)?;
        state.lock().unwrap().log.line_fmt(format_args!(
            "账号 {} 的扫雷{}难度已完成：一共玩了 {} 局，成功 {} 局，踩雷 {} 局，异常 {} 局，总收益 {}，{}。",
            summary.email,
            localized_difficulty(&summary.difficulty),
            summary.played,
            summary.won,
            summary.lost,
            summary.failed,
            format_amount(summary.total_reward),
            remaining_after_clause(summary.remaining_after),
        ));
    }

    let all_summaries = summaries.into_values().collect::<Vec<_>>();
    append_account_summary(
        &state.lock().unwrap().result_log_dir,
        runtime.email(),
        current_unix_ms(),
        &all_summaries,
    )?;
    state.lock().unwrap().log.line_fmt(format_args!(
        "账号 {} 的自动扫雷运行完成。",
        runtime.email()
    ));
    Ok(all_summaries)
}

fn drain_active_round(
    cancel_flag: &ui::CancelFlag,
    state: &Arc<Mutex<BatchState>>,
    runtime: &mut AccountRuntime,
    config: &MinesweeperConfigResponse,
    active: &crate::model::MinesweeperSession,
    progress: RoundProgress,
    remaining_after: i32,
) -> io::Result<MinesweeperRoundSummary> {
    state.lock().unwrap().log.line_fmt(format_args!(
        "账号 {} 发现扫雷{}难度残局，先继续玩完今天第 {}/{} 局（对局 {}）。",
        runtime.email(),
        localized_difficulty(&active.difficulty),
        progress.current,
        progress.total,
        active.play_id,
    ));
    let snapshot = snapshot_from_session(active)
        .map_err(|error| io::Error::other(format!("扫雷残局数据无效：{}", error)))?;
    let result = play_round(
        cancel_flag,
        state,
        runtime,
        config,
        snapshot,
        RoundPlayContext {
            continued: true,
            progress,
            remaining_after,
        },
    )?;
    append_round_result(&state.lock().unwrap().result_log_dir, &result)?;
    log_round_result(&state.lock().unwrap().log, &result);
    Ok(result)
}

fn choose_next_difficulty(
    difficulties: &[String],
    unavailable_difficulties: &HashSet<String>,
    remaining_by_difficulty: &HashMap<String, i32>,
) -> Option<String> {
    let has_known_remaining = !remaining_by_difficulty.is_empty();
    difficulties
        .iter()
        .find(|difficulty| {
            !unavailable_difficulties.contains(*difficulty)
                && (!has_known_remaining
                    || remaining_by_difficulty
                        .get(*difficulty)
                        .copied()
                        .unwrap_or(0)
                        > 0)
        })
        .cloned()
}

fn remaining_after_for_difficulty(
    remaining_by_difficulty: &HashMap<String, i32>,
    difficulty: &str,
) -> i32 {
    remaining_by_difficulty
        .get(difficulty)
        .copied()
        .map(|value| value.max(0))
        .unwrap_or(UNKNOWN_REMAINING_AFTER)
}

fn update_remaining_after_start(
    remaining_by_difficulty: &mut HashMap<String, i32>,
    difficulty: &str,
    start: &MinesweeperStartResponse,
) -> i32 {
    if !start.daily_plays_remaining.is_empty() {
        *remaining_by_difficulty = start.daily_plays_remaining.clone();
        return remaining_after_for_difficulty(remaining_by_difficulty, difficulty);
    }
    if let Some(remaining) = remaining_by_difficulty.get_mut(difficulty) {
        *remaining = (*remaining).saturating_sub(1);
        return (*remaining).max(0);
    }
    UNKNOWN_REMAINING_AFTER
}

fn total_played(summaries: &HashMap<String, MinesweeperDifficultySummary>) -> i32 {
    summaries
        .values()
        .map(|summary| summary.played)
        .sum::<i32>()
        .max(0)
}

fn is_difficulty_unavailable_error(message: &str) -> bool {
    if is_daily_limit_error(message) {
        return true;
    }
    let lower = message.to_ascii_lowercase();
    lower.contains("new_round_unavailable")
        || lower.contains("no remaining plays")
        || lower.contains("remaining plays exhausted")
        || lower.contains("not available")
        || lower.contains("invalid difficulty")
        || message.contains("不能开局")
        || message.contains("无法开局")
        || message.contains("没有返回可玩的新局")
}

fn fetch_config(
    cancel_flag: &ui::CancelFlag,
    state: &Arc<Mutex<BatchState>>,
    runtime: &mut AccountRuntime,
) -> io::Result<MinesweeperConfigResponse> {
    with_auth_retry_api_until_success(
        cancel_flag,
        state,
        runtime,
        "minesweeper config",
        |client, auth_token| client.get_minesweeper_config(auth_token),
    )
}

fn fetch_me(
    cancel_flag: &ui::CancelFlag,
    state: &Arc<Mutex<BatchState>>,
    runtime: &mut AccountRuntime,
) -> io::Result<MinesweeperMeResponse> {
    with_auth_retry_api_until_success(
        cancel_flag,
        state,
        runtime,
        "minesweeper me",
        |client, auth_token| client.get_minesweeper_me(auth_token),
    )
}

fn start_new_round(
    cancel_flag: &ui::CancelFlag,
    state: &Arc<Mutex<BatchState>>,
    runtime: &mut AccountRuntime,
    difficulty: &str,
) -> io::Result<MinesweeperStartResponse> {
    let start = with_auth_retry_api_until_success(
        cancel_flag,
        state,
        runtime,
        "minesweeper start",
        |client, auth_token| client.start_minesweeper(auth_token, difficulty),
    )?;
    if !start.ok {
        return Err(io::Error::other(
            "new_round_unavailable: 扫雷接口没有返回可玩的新局",
        ));
    }
    if start.session.play_id <= 0 || start.session.rows <= 0 || start.session.cols <= 0 {
        return Err(io::Error::other(
            "new_round_unavailable: 扫雷接口返回的数据缺少有效对局",
        ));
    }
    Ok(start)
}

fn merge_round_into_cache(
    cache: &mut AccountProgressCache,
    email: &str,
    result: &MinesweeperRoundSummary,
) -> bool {
    if result.play_id > 0 && !cache.seen_play_ids.insert(result.play_id) {
        return false;
    }
    let entry = cache
        .summaries
        .entry(result.difficulty.clone())
        .or_insert_with(|| MinesweeperDifficultySummary {
            email: email.to_string(),
            difficulty: result.difficulty.clone(),
            ..MinesweeperDifficultySummary::default()
        });
    merge_round_into_summary(entry, result);
    true
}

fn merge_round_into_summaries(
    summaries: &mut HashMap<String, MinesweeperDifficultySummary>,
    email: &str,
    result: &MinesweeperRoundSummary,
) {
    let entry = summaries
        .entry(result.difficulty.clone())
        .or_insert_with(|| MinesweeperDifficultySummary {
            email: email.to_string(),
            difficulty: result.difficulty.clone(),
            ..MinesweeperDifficultySummary::default()
        });
    merge_round_into_summary(entry, result);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn choose_next_difficulty_uses_reward_order() {
        let difficulties = vec![
            "expert".to_string(),
            "intermediate".to_string(),
            "beginner".to_string(),
        ];

        assert_eq!(
            choose_next_difficulty(&difficulties, &HashSet::new(), &HashMap::new()),
            Some("expert".to_string())
        );
    }

    #[test]
    fn choose_next_difficulty_skips_unavailable_rewards() {
        let difficulties = vec![
            "expert".to_string(),
            "intermediate".to_string(),
            "beginner".to_string(),
        ];
        let unavailable = HashSet::from(["expert".to_string(), "intermediate".to_string()]);

        assert_eq!(
            choose_next_difficulty(&difficulties, &unavailable, &HashMap::new()),
            Some("beginner".to_string())
        );
    }

    #[test]
    fn choose_next_difficulty_uses_known_remaining_counts() {
        let difficulties = vec![
            "expert".to_string(),
            "intermediate".to_string(),
            "beginner".to_string(),
        ];
        let remaining = HashMap::from([
            ("expert".to_string(), 0),
            ("intermediate".to_string(), 2),
            ("beginner".to_string(), 5),
        ]);

        assert_eq!(
            choose_next_difficulty(&difficulties, &HashSet::new(), &remaining),
            Some("intermediate".to_string())
        );
    }
}
