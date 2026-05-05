use std::io;
use std::sync::Arc;
use std::sync::mpsc;
use std::time::Duration;

use crate::model::{AuthCache, AuthConfig, CheckinResult};
use crate::storage::upsert_account;
use crate::ui;
use crate::workflows::common::{AccountRewardSummary, print_account_reward_summary};
use crate::workflows::{
    checkin, flowfree, lightsout, maze, memory, minesweeper, nonogram, puzzle_15, puzzle_2048,
    sheepmatch, sokoban, sudoku,
};

pub const TASK_TITLE: &str = "全自动运行所有有次数限制的白嫖玩法";
pub const DONE_MESSAGE: &str = "全自动运行所有有次数限制的白嫖玩法已完成。";

const LIMITED_FREE_PLAY_LOG_TITLE: &str = "全自动有次数限制的白嫖玩法";
const LIMITED_FEATURE_RETRY_BACKOFF: Duration = Duration::ZERO;

pub(crate) type CheckinRunner = Arc<
    dyn Fn(
            &AuthConfig,
            AuthCache,
            &ui::CancelFlag,
        ) -> io::Result<Option<checkin::AccountCheckinOutput>>
        + Send
        + Sync,
>;
pub(crate) type SheepmatchRunner = Arc<
    dyn Fn(&AuthConfig, AuthCache, &ui::CancelFlag) -> io::Result<sheepmatch::AccountRunOutput>
        + Send
        + Sync,
>;
pub(crate) type MinesweeperRunner = Arc<
    dyn Fn(&AuthConfig, AuthCache, &ui::CancelFlag) -> io::Result<minesweeper::AccountRunOutput>
        + Send
        + Sync,
>;
pub(crate) type Puzzle2048Runner = Arc<
    dyn Fn(&AuthConfig, AuthCache, &ui::CancelFlag) -> io::Result<puzzle_2048::AccountRunOutput>
        + Send
        + Sync,
>;
pub(crate) type SokobanRunner = Arc<
    dyn Fn(&AuthConfig, AuthCache, &ui::CancelFlag) -> io::Result<sokoban::AccountRunOutput>
        + Send
        + Sync,
>;
pub(crate) type LightsoutRunner = Arc<
    dyn Fn(&AuthConfig, AuthCache, &ui::CancelFlag) -> io::Result<lightsout::AccountRunOutput>
        + Send
        + Sync,
>;
pub(crate) type MazeRunner = Arc<
    dyn Fn(&AuthConfig, AuthCache, &ui::CancelFlag) -> io::Result<maze::AccountRunOutput>
        + Send
        + Sync,
>;
pub(crate) type NonogramRunner = Arc<
    dyn Fn(&AuthConfig, AuthCache, &ui::CancelFlag) -> io::Result<nonogram::AccountRunOutput>
        + Send
        + Sync,
>;
pub(crate) type FlowfreeRunner = Arc<
    dyn Fn(&AuthConfig, AuthCache, &ui::CancelFlag) -> io::Result<flowfree::AccountRunOutput>
        + Send
        + Sync,
>;
pub(crate) type MemoryRunner = Arc<
    dyn Fn(&AuthConfig, AuthCache, &ui::CancelFlag) -> io::Result<memory::AccountRunOutput>
        + Send
        + Sync,
>;
pub(crate) type Puzzle15Runner = Arc<
    dyn Fn(&AuthConfig, AuthCache, &ui::CancelFlag) -> io::Result<puzzle_15::AccountRunOutput>
        + Send
        + Sync,
>;
pub(crate) type SudokuRunner = Arc<
    dyn Fn(&AuthConfig, AuthCache, &ui::CancelFlag) -> io::Result<sudoku::AccountRunOutput>
        + Send
        + Sync,
>;
pub(crate) type SaveMergedConfig = Box<dyn Fn(AuthConfig) -> io::Result<()> + Send>;

pub(crate) struct LimitedFreeFeatureRunners {
    pub(crate) run_checkin: CheckinRunner,
    pub(crate) run_minesweeper: MinesweeperRunner,
    pub(crate) run_sheepmatch: SheepmatchRunner,
    pub(crate) run_puzzle_2048: Puzzle2048Runner,
    pub(crate) run_sokoban: SokobanRunner,
    pub(crate) run_lightsout: LightsoutRunner,
    pub(crate) run_maze: MazeRunner,
    pub(crate) run_nonogram: NonogramRunner,
    pub(crate) run_flowfree: FlowfreeRunner,
    pub(crate) run_memory: MemoryRunner,
    pub(crate) run_puzzle_15: Puzzle15Runner,
    pub(crate) run_sudoku: SudokuRunner,
    pub(crate) save_merged_config: SaveMergedConfig,
}

fn run_limited_feature_with_status<T, F>(
    cancel_flag: &ui::CancelFlag,
    log: &ui::TaskLog,
    email: &str,
    feature: &str,
    mut action: F,
) -> io::Result<T>
where
    F: FnMut() -> io::Result<T>,
{
    let mut retry_count = 0usize;
    loop {
        ui::check_cancel(cancel_flag)?;
        if retry_count == 0 {
            log.line_fmt(format_args!(
                "【{}｜{}｜{}】开始运行。",
                LIMITED_FREE_PLAY_LOG_TITLE, feature, email
            ));
        } else {
            log.line_fmt(format_args!(
                "【{}｜{}｜{}】重新进入玩法，继续处理残局和剩余次数（第 {} 次重试）。",
                LIMITED_FREE_PLAY_LOG_TITLE, feature, email, retry_count
            ));
        }

        match action() {
            Ok(value) => {
                log.line_fmt(format_args!(
                    "【{}｜{}｜{}】运行完成。",
                    LIMITED_FREE_PLAY_LOG_TITLE, feature, email
                ));
                return Ok(value);
            }
            Err(error) if error.kind() == io::ErrorKind::Interrupted => return Err(error),
            Err(error) => {
                retry_count += 1;
                log.line_fmt(format_args!(
                    "【{}｜{}｜{}】这次没有跑完：{}。不会跳过，会等一下重新进去续残局/剩余次数。",
                    LIMITED_FREE_PLAY_LOG_TITLE, feature, email, error
                ));
                ui::sleep_with_cancel(cancel_flag, LIMITED_FEATURE_RETRY_BACKOFF)?;
            }
        }
    }
}

pub(crate) fn execute_all_limited_free_features(
    original_config: AuthConfig,
    cancel_flag: &ui::CancelFlag,
    log: &ui::TaskLog,
    runners: LimitedFreeFeatureRunners,
) -> io::Result<(Vec<CheckinResult>, AuthConfig)> {
    enum AccountProgress {
        Completed {
            checkin_result: Option<CheckinResult>,
            account: AuthCache,
            reward_summary: AccountRewardSummary,
            error_message: Option<String>,
        },
    }

    enum FeatureProgress {
        Checkin(Option<checkin::AccountCheckinOutput>),
        Minesweeper(minesweeper::AccountRunOutput),
        Sheepmatch(sheepmatch::AccountRunOutput),
        Puzzle2048(puzzle_2048::AccountRunOutput),
        Sokoban(sokoban::AccountRunOutput),
        Lightsout(lightsout::AccountRunOutput),
        Maze(maze::AccountRunOutput),
        Nonogram(nonogram::AccountRunOutput),
        Flowfree(flowfree::AccountRunOutput),
        Memory(memory::AccountRunOutput),
        Puzzle15(puzzle_15::AccountRunOutput),
        Sudoku(sudoku::AccountRunOutput),
    }

    let LimitedFreeFeatureRunners {
        run_checkin,
        run_minesweeper,
        run_sheepmatch,
        run_puzzle_2048,
        run_sokoban,
        run_lightsout,
        run_maze,
        run_nonogram,
        run_flowfree,
        run_memory,
        run_puzzle_15,
        run_sudoku,
        save_merged_config,
    } = runners;
    let (result_tx, result_rx) = mpsc::channel::<io::Result<AccountProgress>>();
    let mut handles = Vec::with_capacity(original_config.accounts.len());

    let mut reward_summaries = original_config
        .accounts
        .iter()
        .enumerate()
        .map(|(index, account)| AccountRewardSummary {
            index,
            email: account.email.trim().to_string(),
            total_reward: 0.0,
        })
        .collect::<Vec<_>>();

    for (account_index, account) in original_config.accounts.clone().into_iter().enumerate() {
        ui::check_cancel(cancel_flag)?;
        let cancel_flag = Arc::clone(cancel_flag);
        let base_config = original_config.clone();
        let run_checkin = Arc::clone(&run_checkin);
        let run_minesweeper = Arc::clone(&run_minesweeper);
        let run_sheepmatch = Arc::clone(&run_sheepmatch);
        let run_puzzle_2048 = Arc::clone(&run_puzzle_2048);
        let run_sokoban = Arc::clone(&run_sokoban);
        let run_lightsout = Arc::clone(&run_lightsout);
        let run_maze = Arc::clone(&run_maze);
        let run_nonogram = Arc::clone(&run_nonogram);
        let run_flowfree = Arc::clone(&run_flowfree);
        let run_memory = Arc::clone(&run_memory);
        let run_puzzle_15 = Arc::clone(&run_puzzle_15);
        let run_sudoku = Arc::clone(&run_sudoku);
        let result_tx = result_tx.clone();
        let log = log.clone();
        handles.push(std::thread::spawn(move || {
            let result: io::Result<AccountProgress> = {
                let account_email = account.email.trim().to_string();
                log.line_fmt(format_args!(
                    "【{}｜账号 {}】开始并发执行：自动签到、自动扫雷、自动羊了个羊、自动谜题2048、自动推箱子、自动点灯、自动迷宫、自动数织、自动连线、自动记忆翻牌、自动华容道、自动数独。",
                    LIMITED_FREE_PLAY_LOG_TITLE, account_email
                ));
                let account_config = AuthConfig {
                    base_url: base_config.base_url.clone(),
                    accounts: vec![account.clone()],
                };
                let feature_handles = vec![
                    std::thread::spawn({
                        let account_config = account_config.clone();
                        let account = account.clone();
                        let cancel_flag = Arc::clone(&cancel_flag);
                        let run_checkin = Arc::clone(&run_checkin);
                        let log = log.clone();
                        let email = account_email.clone();
                        move || {
                            run_limited_feature_with_status(&cancel_flag, &log, &email, "自动签到", || {
                                run_checkin(&account_config, account.clone(), &cancel_flag)
                                    .map(FeatureProgress::Checkin)
                            })
                        }
                    }),
                    std::thread::spawn({
                        let account_config = account_config.clone();
                        let account = account.clone();
                        let cancel_flag = Arc::clone(&cancel_flag);
                        let run_minesweeper = Arc::clone(&run_minesweeper);
                        let log = log.clone();
                        let email = account_email.clone();
                        move || {
                            run_limited_feature_with_status(&cancel_flag, &log, &email, "自动扫雷", || {
                                run_minesweeper(&account_config, account.clone(), &cancel_flag)
                                    .map(FeatureProgress::Minesweeper)
                            })
                        }
                    }),
                    std::thread::spawn({
                        let account_config = account_config.clone();
                        let account = account.clone();
                        let cancel_flag = Arc::clone(&cancel_flag);
                        let run_sheepmatch = Arc::clone(&run_sheepmatch);
                        let log = log.clone();
                        let email = account_email.clone();
                        move || {
                            run_limited_feature_with_status(&cancel_flag, &log, &email, "自动羊了个羊", || {
                                run_sheepmatch(&account_config, account.clone(), &cancel_flag)
                                    .map(FeatureProgress::Sheepmatch)
                            })
                        }
                    }),
                    std::thread::spawn({
                        let account_config = account_config.clone();
                        let account = account.clone();
                        let cancel_flag = Arc::clone(&cancel_flag);
                        let run_puzzle_2048 = Arc::clone(&run_puzzle_2048);
                        let log = log.clone();
                        let email = account_email.clone();
                        move || {
                            run_limited_feature_with_status(&cancel_flag, &log, &email, "自动谜题2048", || {
                                run_puzzle_2048(&account_config, account.clone(), &cancel_flag)
                                    .map(FeatureProgress::Puzzle2048)
                            })
                        }
                    }),
                    std::thread::spawn({
                        let account_config = account_config.clone();
                        let account = account.clone();
                        let cancel_flag = Arc::clone(&cancel_flag);
                        let run_sokoban = Arc::clone(&run_sokoban);
                        let log = log.clone();
                        let email = account_email.clone();
                        move || {
                            run_limited_feature_with_status(&cancel_flag, &log, &email, "自动推箱子", || {
                                run_sokoban(&account_config, account.clone(), &cancel_flag)
                                    .map(FeatureProgress::Sokoban)
                            })
                        }
                    }),
                    std::thread::spawn({
                        let account_config = account_config.clone();
                        let account = account.clone();
                        let cancel_flag = Arc::clone(&cancel_flag);
                        let run_lightsout = Arc::clone(&run_lightsout);
                        let log = log.clone();
                        let email = account_email.clone();
                        move || {
                            run_limited_feature_with_status(&cancel_flag, &log, &email, "自动点灯", || {
                                run_lightsout(&account_config, account.clone(), &cancel_flag)
                                    .map(FeatureProgress::Lightsout)
                            })
                        }
                    }),
                    std::thread::spawn({
                        let account_config = account_config.clone();
                        let account = account.clone();
                        let cancel_flag = Arc::clone(&cancel_flag);
                        let run_maze = Arc::clone(&run_maze);
                        let log = log.clone();
                        let email = account_email.clone();
                        move || {
                            run_limited_feature_with_status(&cancel_flag, &log, &email, "自动迷宫", || {
                                run_maze(&account_config, account.clone(), &cancel_flag)
                                    .map(FeatureProgress::Maze)
                            })
                        }
                    }),
                    std::thread::spawn({
                        let account_config = account_config.clone();
                        let account = account.clone();
                        let cancel_flag = Arc::clone(&cancel_flag);
                        let run_nonogram = Arc::clone(&run_nonogram);
                        let log = log.clone();
                        let email = account_email.clone();
                        move || {
                            run_limited_feature_with_status(&cancel_flag, &log, &email, "自动数织", || {
                                run_nonogram(&account_config, account.clone(), &cancel_flag)
                                    .map(FeatureProgress::Nonogram)
                            })
                        }
                    }),
                    std::thread::spawn({
                        let account_config = account_config.clone();
                        let account = account.clone();
                        let cancel_flag = Arc::clone(&cancel_flag);
                        let run_flowfree = Arc::clone(&run_flowfree);
                        let log = log.clone();
                        let email = account_email.clone();
                        move || {
                            run_limited_feature_with_status(&cancel_flag, &log, &email, "自动连线", || {
                                run_flowfree(&account_config, account.clone(), &cancel_flag)
                                    .map(FeatureProgress::Flowfree)
                            })
                        }
                    }),
                    std::thread::spawn({
                        let account_config = account_config.clone();
                        let account = account.clone();
                        let cancel_flag = Arc::clone(&cancel_flag);
                        let run_memory = Arc::clone(&run_memory);
                        let log = log.clone();
                        let email = account_email.clone();
                        move || {
                            run_limited_feature_with_status(&cancel_flag, &log, &email, "自动记忆翻牌", || {
                                run_memory(&account_config, account.clone(), &cancel_flag)
                                    .map(FeatureProgress::Memory)
                            })
                        }
                    }),
                    std::thread::spawn({
                        let account_config = account_config.clone();
                        let account = account.clone();
                        let cancel_flag = Arc::clone(&cancel_flag);
                        let run_puzzle_15 = Arc::clone(&run_puzzle_15);
                        let log = log.clone();
                        let email = account_email.clone();
                        move || {
                            run_limited_feature_with_status(&cancel_flag, &log, &email, "自动华容道", || {
                                run_puzzle_15(&account_config, account.clone(), &cancel_flag)
                                    .map(FeatureProgress::Puzzle15)
                            })
                        }
                    }),
                    std::thread::spawn({
                        let account_config = account_config.clone();
                        let account = account.clone();
                        let cancel_flag = Arc::clone(&cancel_flag);
                        let run_sudoku = Arc::clone(&run_sudoku);
                        let log = log.clone();
                        let email = account_email.clone();
                        move || {
                            run_limited_feature_with_status(&cancel_flag, &log, &email, "自动数独", || {
                                run_sudoku(&account_config, account.clone(), &cancel_flag)
                                    .map(FeatureProgress::Sudoku)
                            })
                        }
                    }),
                ];

                let mut merged_config = account_config.clone();
                let mut checkin_result = None;
                let mut first_error = None;
                let mut total_reward = 0.0;
                for handle in feature_handles {
                    match handle.join() {
                        Ok(Ok(progress)) => match progress {
                            FeatureProgress::Checkin(Some(output)) => {
                                total_reward += output.result.delta;
                                checkin_result = Some(output.result);
                                merged_config = upsert_account(merged_config, output.account);
                            }
                            FeatureProgress::Checkin(None) => {}
                            FeatureProgress::Minesweeper(output) => {
                                total_reward += output.total_reward;
                                merged_config = upsert_account(merged_config, output.account);
                            }
                            FeatureProgress::Sheepmatch(output) => {
                                total_reward += output.total_reward;
                                merged_config = upsert_account(merged_config, output.account);
                            }
                            FeatureProgress::Puzzle2048(output) => {
                                total_reward += output.total_reward;
                                merged_config = upsert_account(merged_config, output.account);
                            }
                            FeatureProgress::Sokoban(output) => {
                                total_reward += output.total_reward;
                                merged_config = upsert_account(merged_config, output.account);
                            }
                            FeatureProgress::Lightsout(output) => {
                                total_reward += output.total_reward;
                                merged_config = upsert_account(merged_config, output.account);
                            }
                            FeatureProgress::Maze(output) => {
                                total_reward += output.total_reward;
                                merged_config = upsert_account(merged_config, output.account);
                            }
                            FeatureProgress::Nonogram(output) => {
                                total_reward += output.total_reward;
                                merged_config = upsert_account(merged_config, output.account);
                            }
                            FeatureProgress::Flowfree(output) => {
                                total_reward += output.total_reward;
                                merged_config = upsert_account(merged_config, output.account);
                            }
                            FeatureProgress::Memory(output) => {
                                total_reward += output.total_reward;
                                merged_config = upsert_account(merged_config, output.account);
                            }
                            FeatureProgress::Puzzle15(output) => {
                                total_reward += output.total_reward;
                                merged_config = upsert_account(merged_config, output.account);
                            }
                            FeatureProgress::Sudoku(output) => {
                                total_reward += output.total_reward;
                                merged_config = upsert_account(merged_config, output.account);
                            }
                        },
                        Ok(Err(error)) => {
                            if first_error.is_none() {
                                first_error = Some(error);
                            }
                        }
                        Err(_) => {
                            if first_error.is_none() {
                                first_error = Some(io::Error::other(
                                    "全自动有次数限制的白嫖玩法的某个项目异常退出，请查看前面的项目日志了解原因。",
                                ));
                            }
                        }
                    }
                }

                let account = merged_config.accounts.first().cloned().unwrap_or(account);
                log.line_fmt(format_args!(
                    "【{}｜账号 {}】所有白嫖项目都已结束，正在合并最新登录状态。",
                    LIMITED_FREE_PLAY_LOG_TITLE, account_email
                ));
                Ok(AccountProgress::Completed {
                    checkin_result,
                    account,
                    reward_summary: AccountRewardSummary {
                        index: account_index,
                        email: account_email,
                        total_reward,
                    },
                    error_message: first_error.map(|error| error.to_string()),
                })
            };
            let _ = result_tx.send(result);
        }));
    }
    drop(result_tx);

    let mut merged_config = original_config.clone();
    let mut checkin_results = Vec::new();
    let mut first_error = None;
    for _ in 0..handles.len() {
        match result_rx.recv() {
            Ok(Ok(AccountProgress::Completed {
                checkin_result,
                account,
                reward_summary,
                error_message,
            })) => {
                if let Some(checkin_result) = checkin_result {
                    checkin_results.push(checkin_result);
                }
                if let Some(slot) = reward_summaries.get_mut(reward_summary.index) {
                    *slot = reward_summary;
                }
                merged_config = upsert_account(merged_config, account);
                if let Some(error_message) = error_message
                    && first_error.is_none()
                {
                    first_error = Some(io::Error::other(error_message));
                }
            }
            Ok(Err(error)) => {
                if first_error.is_none() {
                    first_error = Some(error);
                }
            }
            Err(_) => {
                if first_error.is_none() {
                    first_error = Some(io::Error::other(
                        "全自动有次数限制的白嫖玩法任务提前结束，请查看前面的项目日志了解原因。",
                    ));
                }
                break;
            }
        }
    }

    for handle in handles {
        if handle.join().is_err() && first_error.is_none() {
            first_error = Some(io::Error::other(
                "全自动有次数限制的白嫖玩法任务异常退出，请查看前面的项目日志了解原因。",
            ));
        }
    }

    print_account_reward_summary(log, LIMITED_FREE_PLAY_LOG_TITLE, &reward_summaries);

    if let Some(error) = first_error {
        return Err(error);
    }

    save_merged_config(merged_config.clone())?;
    log.line_fmt(format_args!(
        "【{}】所有账号都已完成，已合并并保存最新登录状态（共 {} 个账号）。",
        LIMITED_FREE_PLAY_LOG_TITLE,
        merged_config.accounts.len()
    ));
    Ok((checkin_results, merged_config))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::{Condvar, Mutex};
    use std::time::Duration;

    fn sample_config(emails: &[&str]) -> AuthConfig {
        AuthConfig {
            base_url: "http://example.com".to_string(),
            accounts: emails
                .iter()
                .map(|email| AuthCache {
                    email: (*email).to_string(),
                    ..AuthCache::default()
                })
                .collect(),
        }
    }

    #[test]
    fn execute_all_limited_free_features_runs_all_games_per_account_and_saves_once() {
        let config = sample_config(&["alpha@example.com", "beta@example.com"]);
        let cancel_flag = Arc::new(AtomicBool::new(false));
        let events = Arc::new(Mutex::new(Vec::<String>::new()));
        let saved_configs = Arc::new(Mutex::new(Vec::<AuthConfig>::new()));

        let log = ui::TaskLog::stdout();
        let (results, merged_config) = execute_all_limited_free_features(
            config,
            &cancel_flag,
            &log,
            LimitedFreeFeatureRunners {
                run_checkin: Arc::new({
                    let events = Arc::clone(&events);
                    move |account_config, account, _cancel_flag| {
                        assert_eq!(account_config.accounts.len(), 1);
                        assert_eq!(account_config.accounts[0].email, account.email);
                        events
                            .lock()
                            .unwrap()
                            .push(format!("checkin:{}", account.email));
                        let mut updated_account = account.clone();
                        updated_account.password = "after-checkin".to_string();
                        Ok(Some(checkin::AccountCheckinOutput {
                            account: updated_account,
                            result: CheckinResult {
                                email: account.email,
                                status: "签到成功".to_string(),
                                success: true,
                                delta: 1.0,
                                ..CheckinResult::default()
                            },
                        }))
                    }
                }),
                run_minesweeper: Arc::new({
                    let events = Arc::clone(&events);
                    move |account_config, account, _cancel_flag| {
                        assert_eq!(account_config.accounts.len(), 1);
                        assert_eq!(account_config.accounts[0].email, account.email);
                        events
                            .lock()
                            .unwrap()
                            .push(format!("minesweeper:{}", account.email));
                        let mut updated_account = account.clone();
                        updated_account.access_token = "after-minesweeper".to_string();
                        Ok(minesweeper::AccountRunOutput {
                            account: updated_account,
                            total_reward: 2.5,
                        })
                    }
                }),
                run_sheepmatch: Arc::new({
                    let events = Arc::clone(&events);
                    move |account_config, account, _cancel_flag| {
                        assert_eq!(account_config.accounts.len(), 1);
                        assert_eq!(account_config.accounts[0].email, account.email);
                        events
                            .lock()
                            .unwrap()
                            .push(format!("sheepmatch:{}", account.email));
                        let mut updated_account = account.clone();
                        updated_account.access_token = "after-sheepmatch".to_string();
                        Ok(sheepmatch::AccountRunOutput {
                            account: updated_account,
                            total_reward: 2.0,
                        })
                    }
                }),
                run_puzzle_2048: Arc::new({
                    let events = Arc::clone(&events);
                    move |account_config, account, _cancel_flag| {
                        assert_eq!(account_config.accounts.len(), 1);
                        assert_eq!(account_config.accounts[0].email, account.email);
                        events
                            .lock()
                            .unwrap()
                            .push(format!("puzzle_2048:{}", account.email));
                        let mut updated_account = account.clone();
                        updated_account.token_type = "after-puzzle_2048".to_string();
                        updated_account.access_token = "after-puzzle_2048-token".to_string();
                        Ok(puzzle_2048::AccountRunOutput {
                            account: updated_account,
                            total_reward: 3.0,
                        })
                    }
                }),
                run_sokoban: Arc::new({
                    let events = Arc::clone(&events);
                    move |account_config, account, _cancel_flag| {
                        assert_eq!(account_config.accounts.len(), 1);
                        assert_eq!(account_config.accounts[0].email, account.email);
                        events
                            .lock()
                            .unwrap()
                            .push(format!("sokoban:{}", account.email));
                        Ok(sokoban::AccountRunOutput {
                            account,
                            total_reward: 0.3,
                        })
                    }
                }),
                run_lightsout: Arc::new({
                    let events = Arc::clone(&events);
                    move |account_config, account, _cancel_flag| {
                        assert_eq!(account_config.accounts.len(), 1);
                        assert_eq!(account_config.accounts[0].email, account.email);
                        events
                            .lock()
                            .unwrap()
                            .push(format!("lightsout:{}", account.email));
                        Ok(lightsout::AccountRunOutput {
                            account,
                            total_reward: 0.3,
                        })
                    }
                }),
                run_maze: Arc::new({
                    let events = Arc::clone(&events);
                    move |account_config, account, _cancel_flag| {
                        assert_eq!(account_config.accounts.len(), 1);
                        assert_eq!(account_config.accounts[0].email, account.email);
                        events
                            .lock()
                            .unwrap()
                            .push(format!("maze:{}", account.email));
                        Ok(maze::AccountRunOutput {
                            account,
                            total_reward: 0.3,
                        })
                    }
                }),
                run_nonogram: Arc::new({
                    let events = Arc::clone(&events);
                    move |account_config, account, _cancel_flag| {
                        assert_eq!(account_config.accounts.len(), 1);
                        assert_eq!(account_config.accounts[0].email, account.email);
                        events
                            .lock()
                            .unwrap()
                            .push(format!("nonogram:{}", account.email));
                        Ok(nonogram::AccountRunOutput {
                            account,
                            total_reward: 0.3,
                        })
                    }
                }),
                run_flowfree: Arc::new({
                    let events = Arc::clone(&events);
                    move |account_config, account, _cancel_flag| {
                        assert_eq!(account_config.accounts.len(), 1);
                        assert_eq!(account_config.accounts[0].email, account.email);
                        events
                            .lock()
                            .unwrap()
                            .push(format!("flowfree:{}", account.email));
                        Ok(flowfree::AccountRunOutput {
                            account,
                            total_reward: 0.3,
                        })
                    }
                }),
                run_memory: Arc::new({
                    let events = Arc::clone(&events);
                    move |account_config, account, _cancel_flag| {
                        assert_eq!(account_config.accounts.len(), 1);
                        assert_eq!(account_config.accounts[0].email, account.email);
                        events
                            .lock()
                            .unwrap()
                            .push(format!("memory:{}", account.email));
                        let mut updated_account = account.clone();
                        updated_account.access_token = "after-memory".to_string();
                        Ok(memory::AccountRunOutput {
                            account: updated_account,
                            total_reward: 4.0,
                        })
                    }
                }),
                run_puzzle_15: Arc::new({
                    let events = Arc::clone(&events);
                    move |account_config, account, _cancel_flag| {
                        assert_eq!(account_config.accounts.len(), 1);
                        assert_eq!(account_config.accounts[0].email, account.email);
                        events
                            .lock()
                            .unwrap()
                            .push(format!("puzzle_15:{}", account.email));
                        let mut updated_account = account.clone();
                        updated_account.token_type = "after-puzzle_15".to_string();
                        updated_account.access_token = "after-puzzle_15-token".to_string();
                        Ok(puzzle_15::AccountRunOutput {
                            account: updated_account,
                            total_reward: 5.0,
                        })
                    }
                }),
                run_sudoku: Arc::new({
                    let events = Arc::clone(&events);
                    move |account_config, account, _cancel_flag| {
                        assert_eq!(account_config.accounts.len(), 1);
                        assert_eq!(account_config.accounts[0].email, account.email);
                        events
                            .lock()
                            .unwrap()
                            .push(format!("sudoku:{}", account.email));
                        let mut updated_account = account.clone();
                        updated_account.token_type = "after-sudoku".to_string();
                        updated_account.access_token = "after-sudoku-token".to_string();
                        Ok(sudoku::AccountRunOutput {
                            account: updated_account,
                            total_reward: 6.0,
                        })
                    }
                }),
                save_merged_config: Box::new({
                    let saved_configs = Arc::clone(&saved_configs);
                    move |merged_config| {
                        saved_configs.lock().unwrap().push(merged_config);
                        Ok(())
                    }
                }),
            },
        )
        .unwrap();

        assert_eq!(results.len(), 2);
        let events = events.lock().unwrap().clone();
        for email in ["alpha@example.com", "beta@example.com"] {
            for feature in [
                "checkin",
                "minesweeper",
                "sheepmatch",
                "puzzle_2048",
                "sokoban",
                "lightsout",
                "maze",
                "nonogram",
                "flowfree",
                "memory",
                "puzzle_15",
                "sudoku",
            ] {
                assert!(
                    events
                        .iter()
                        .any(|event| event == &format!("{feature}:{email}")),
                    "expected {feature} to run for {email}, got {events:?}"
                );
            }
        }

        assert!(
            merged_config
                .accounts
                .iter()
                .all(|account| account.password == "after-checkin")
        );
        assert!(
            merged_config
                .accounts
                .iter()
                .all(|account| account.access_token == "after-sudoku-token")
        );
        assert!(
            merged_config
                .accounts
                .iter()
                .all(|account| account.token_type == "after-sudoku")
        );

        let saved_configs = saved_configs.lock().unwrap();
        assert_eq!(saved_configs.len(), 1);
        assert_eq!(saved_configs[0], merged_config);
    }

    #[test]
    fn execute_all_limited_free_features_runs_features_for_same_account_concurrently() {
        let config = sample_config(&["fast@example.com"]);
        let cancel_flag = Arc::new(AtomicBool::new(false));
        let sheepmatch_started = Arc::new((Mutex::new(false), Condvar::new()));

        let log = ui::TaskLog::stdout();
        execute_all_limited_free_features(
            config,
            &cancel_flag,
            &log,
            LimitedFreeFeatureRunners {
                run_checkin: Arc::new({
                    let sheepmatch_started = Arc::clone(&sheepmatch_started);
                    move |_config, account, _cancel_flag| {
                        let (started, ready) = &*sheepmatch_started;
                        let started = started.lock().unwrap();
                        let wait = ready
                            .wait_timeout_while(started, Duration::from_secs(1), |started| {
                                !*started
                            })
                            .unwrap();
                        if !*wait.0 {
                            return Err(io::Error::other(
                                "sheepmatch did not start before checkin finished",
                            ));
                        }
                        Ok(Some(checkin::AccountCheckinOutput {
                            account: account.clone(),
                            result: CheckinResult {
                                email: account.email,
                                status: "签到成功".to_string(),
                                success: true,
                                delta: 1.0,
                                ..CheckinResult::default()
                            },
                        }))
                    }
                }),
                run_minesweeper: Arc::new(move |_config, account, _cancel_flag| {
                    Ok(minesweeper::AccountRunOutput {
                        account,
                        total_reward: 1.5,
                    })
                }),
                run_sheepmatch: Arc::new({
                    let sheepmatch_started = Arc::clone(&sheepmatch_started);
                    move |_config, account, _cancel_flag| {
                        if account.email == "fast@example.com" {
                            let (started, ready) = &*sheepmatch_started;
                            *started.lock().unwrap() = true;
                            ready.notify_all();
                        }
                        Ok(sheepmatch::AccountRunOutput {
                            account,
                            total_reward: 2.0,
                        })
                    }
                }),
                run_puzzle_2048: Arc::new(move |_config, account, _cancel_flag| {
                    Ok(puzzle_2048::AccountRunOutput {
                        account,
                        total_reward: 3.0,
                    })
                }),
                run_sokoban: Arc::new(move |_config, account, _cancel_flag| {
                    Ok(sokoban::AccountRunOutput {
                        account,
                        total_reward: 0.0,
                    })
                }),
                run_lightsout: Arc::new(move |_config, account, _cancel_flag| {
                    Ok(lightsout::AccountRunOutput {
                        account,
                        total_reward: 0.0,
                    })
                }),
                run_maze: Arc::new(move |_config, account, _cancel_flag| {
                    Ok(maze::AccountRunOutput {
                        account,
                        total_reward: 0.0,
                    })
                }),
                run_nonogram: Arc::new(move |_config, account, _cancel_flag| {
                    Ok(nonogram::AccountRunOutput {
                        account,
                        total_reward: 0.0,
                    })
                }),
                run_flowfree: Arc::new(move |_config, account, _cancel_flag| {
                    Ok(flowfree::AccountRunOutput {
                        account,
                        total_reward: 0.0,
                    })
                }),
                run_memory: Arc::new(move |_config, account, _cancel_flag| {
                    Ok(memory::AccountRunOutput {
                        account,
                        total_reward: 4.0,
                    })
                }),
                run_puzzle_15: Arc::new(move |_config, account, _cancel_flag| {
                    Ok(puzzle_15::AccountRunOutput {
                        account,
                        total_reward: 5.0,
                    })
                }),
                run_sudoku: Arc::new(move |_config, account, _cancel_flag| {
                    Ok(sudoku::AccountRunOutput {
                        account,
                        total_reward: 6.0,
                    })
                }),
                save_merged_config: Box::new(|_merged_config| Ok(())),
            },
        )
        .unwrap();
    }

    #[test]
    fn execute_all_limited_free_features_retries_failed_feature_until_success() {
        let config = sample_config(&["retry@example.com"]);
        let cancel_flag = Arc::new(AtomicBool::new(false));
        let sheepmatch_calls = Arc::new(AtomicUsize::new(0));
        let saved_configs = Arc::new(Mutex::new(Vec::<AuthConfig>::new()));

        let log = ui::TaskLog::stdout();
        let (results, merged_config) = execute_all_limited_free_features(
            config,
            &cancel_flag,
            &log,
            LimitedFreeFeatureRunners {
                run_checkin: Arc::new(move |_config, _account, _cancel_flag| Ok(None)),
                run_minesweeper: Arc::new(move |_config, account, _cancel_flag| {
                    Ok(minesweeper::AccountRunOutput {
                        account,
                        total_reward: 0.0,
                    })
                }),
                run_sheepmatch: Arc::new({
                    let sheepmatch_calls = Arc::clone(&sheepmatch_calls);
                    move |_config, account, _cancel_flag| {
                        if sheepmatch_calls.fetch_add(1, Ordering::SeqCst) == 0 {
                            return Err(io::Error::other("temporary sheepmatch error"));
                        }
                        Ok(sheepmatch::AccountRunOutput {
                            account,
                            total_reward: 2.0,
                        })
                    }
                }),
                run_puzzle_2048: Arc::new(move |_config, account, _cancel_flag| {
                    Ok(puzzle_2048::AccountRunOutput {
                        account,
                        total_reward: 0.0,
                    })
                }),
                run_sokoban: Arc::new(move |_config, account, _cancel_flag| {
                    Ok(sokoban::AccountRunOutput {
                        account,
                        total_reward: 0.0,
                    })
                }),
                run_lightsout: Arc::new(move |_config, account, _cancel_flag| {
                    Ok(lightsout::AccountRunOutput {
                        account,
                        total_reward: 0.0,
                    })
                }),
                run_maze: Arc::new(move |_config, account, _cancel_flag| {
                    Ok(maze::AccountRunOutput {
                        account,
                        total_reward: 0.0,
                    })
                }),
                run_nonogram: Arc::new(move |_config, account, _cancel_flag| {
                    Ok(nonogram::AccountRunOutput {
                        account,
                        total_reward: 0.0,
                    })
                }),
                run_flowfree: Arc::new(move |_config, account, _cancel_flag| {
                    Ok(flowfree::AccountRunOutput {
                        account,
                        total_reward: 0.0,
                    })
                }),
                run_memory: Arc::new(move |_config, account, _cancel_flag| {
                    Ok(memory::AccountRunOutput {
                        account,
                        total_reward: 0.0,
                    })
                }),
                run_puzzle_15: Arc::new(move |_config, account, _cancel_flag| {
                    Ok(puzzle_15::AccountRunOutput {
                        account,
                        total_reward: 0.0,
                    })
                }),
                run_sudoku: Arc::new(move |_config, account, _cancel_flag| {
                    Ok(sudoku::AccountRunOutput {
                        account,
                        total_reward: 0.0,
                    })
                }),
                save_merged_config: Box::new({
                    let saved_configs = Arc::clone(&saved_configs);
                    move |merged_config| {
                        saved_configs.lock().unwrap().push(merged_config);
                        Ok(())
                    }
                }),
            },
        )
        .unwrap();

        assert!(results.is_empty());
        assert_eq!(sheepmatch_calls.load(Ordering::SeqCst), 2);
        assert_eq!(merged_config.accounts.len(), 1);
        let saved_configs = saved_configs.lock().unwrap();
        assert_eq!(saved_configs.len(), 1);
        assert_eq!(saved_configs[0], merged_config);
    }
}
