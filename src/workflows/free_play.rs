use std::io;
use std::sync::Arc;
use std::sync::mpsc;

use crate::model::{AuthCache, AuthConfig, CheckinResult};
use crate::storage::upsert_account;
use crate::ui;
use crate::workflows::{checkin, memory, puzzle_15, puzzle_2048, sheepmatch, sudoku};

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
pub(crate) type Puzzle2048Runner = Arc<
    dyn Fn(&AuthConfig, AuthCache, &ui::CancelFlag) -> io::Result<puzzle_2048::AccountRunOutput>
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

pub(crate) struct FreeFeatureRunners {
    pub(crate) run_checkin: CheckinRunner,
    pub(crate) run_sheepmatch: SheepmatchRunner,
    pub(crate) run_puzzle_2048: Puzzle2048Runner,
    pub(crate) run_memory: MemoryRunner,
    pub(crate) run_puzzle_15: Puzzle15Runner,
    pub(crate) run_sudoku: SudokuRunner,
    pub(crate) save_merged_config: SaveMergedConfig,
}

fn run_feature_with_status<T, F>(
    log: &ui::TaskLog,
    email: &str,
    feature: &str,
    action: F,
) -> io::Result<T>
where
    F: FnOnce() -> io::Result<T>,
{
    log.line_fmt(format_args!(
        "【全自动白嫖｜{}｜{}】开始运行。",
        feature, email
    ));
    match action() {
        Ok(value) => {
            log.line_fmt(format_args!(
                "【全自动白嫖｜{}｜{}】运行完成。",
                feature, email
            ));
            Ok(value)
        }
        Err(error) => {
            log.line_fmt(format_args!(
                "【全自动白嫖｜{}｜{}】运行失败：{}",
                feature, email, error
            ));
            Err(error)
        }
    }
}

pub(crate) fn execute_all_free_features(
    original_config: AuthConfig,
    cancel_flag: &ui::CancelFlag,
    log: &ui::TaskLog,
    runners: FreeFeatureRunners,
) -> io::Result<(Vec<CheckinResult>, AuthConfig)> {
    enum AccountProgress {
        Completed {
            checkin_result: Option<CheckinResult>,
            account: AuthCache,
        },
    }

    enum FeatureProgress {
        Checkin(Option<checkin::AccountCheckinOutput>),
        Sheepmatch(sheepmatch::AccountRunOutput),
        Puzzle2048(puzzle_2048::AccountRunOutput),
        Memory(memory::AccountRunOutput),
        Puzzle15(puzzle_15::AccountRunOutput),
        Sudoku(sudoku::AccountRunOutput),
    }

    let FreeFeatureRunners {
        run_checkin,
        run_sheepmatch,
        run_puzzle_2048,
        run_memory,
        run_puzzle_15,
        run_sudoku,
        save_merged_config,
    } = runners;
    let (result_tx, result_rx) = mpsc::channel::<io::Result<AccountProgress>>();
    let mut handles = Vec::with_capacity(original_config.accounts.len());

    for account in original_config.accounts.clone() {
        ui::check_cancel(cancel_flag)?;
        let cancel_flag = Arc::clone(cancel_flag);
        let base_config = original_config.clone();
        let run_checkin = Arc::clone(&run_checkin);
        let run_sheepmatch = Arc::clone(&run_sheepmatch);
        let run_puzzle_2048 = Arc::clone(&run_puzzle_2048);
        let run_memory = Arc::clone(&run_memory);
        let run_puzzle_15 = Arc::clone(&run_puzzle_15);
        let run_sudoku = Arc::clone(&run_sudoku);
        let result_tx = result_tx.clone();
        let log = log.clone();
        handles.push(std::thread::spawn(move || {
            let result = (|| -> io::Result<AccountProgress> {
                let account_email = account.email.trim().to_string();
                log.line_fmt(format_args!(
                    "【全自动白嫖｜账号 {}】开始并发执行：自动签到、自动羊了个羊、自动谜题2048、自动记忆翻牌、自动华容道、自动数独。",
                    account_email
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
                            run_feature_with_status(&log, &email, "自动签到", || {
                                run_checkin(&account_config, account, &cancel_flag)
                                    .map(FeatureProgress::Checkin)
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
                            run_feature_with_status(&log, &email, "自动羊了个羊", || {
                                run_sheepmatch(&account_config, account, &cancel_flag)
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
                            run_feature_with_status(&log, &email, "自动谜题2048", || {
                                run_puzzle_2048(&account_config, account, &cancel_flag)
                                    .map(FeatureProgress::Puzzle2048)
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
                            run_feature_with_status(&log, &email, "自动记忆翻牌", || {
                                run_memory(&account_config, account, &cancel_flag)
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
                            run_feature_with_status(&log, &email, "自动华容道", || {
                                run_puzzle_15(&account_config, account, &cancel_flag)
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
                            run_feature_with_status(&log, &email, "自动数独", || {
                                run_sudoku(&account_config, account, &cancel_flag)
                                    .map(FeatureProgress::Sudoku)
                            })
                        }
                    }),
                ];

                let mut merged_config = account_config.clone();
                let mut checkin_result = None;
                let mut first_error = None;
                for handle in feature_handles {
                    match handle.join() {
                        Ok(Ok(progress)) => match progress {
                            FeatureProgress::Checkin(Some(output)) => {
                                checkin_result = Some(output.result);
                                merged_config = upsert_account(merged_config, output.account);
                            }
                            FeatureProgress::Checkin(None) => {}
                            FeatureProgress::Sheepmatch(output) => {
                                merged_config = upsert_account(merged_config, output.account);
                            }
                            FeatureProgress::Puzzle2048(output) => {
                                merged_config = upsert_account(merged_config, output.account);
                            }
                            FeatureProgress::Memory(output) => {
                                merged_config = upsert_account(merged_config, output.account);
                            }
                            FeatureProgress::Puzzle15(output) => {
                                merged_config = upsert_account(merged_config, output.account);
                            }
                            FeatureProgress::Sudoku(output) => {
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
                                    "全自动白嫖的某个项目异常退出，请查看前面的项目日志了解原因。",
                                ));
                            }
                        }
                    }
                }

                if let Some(error) = first_error {
                    return Err(error);
                }

                let account = merged_config.accounts.first().cloned().unwrap_or(account);
                log.line_fmt(format_args!(
                    "【全自动白嫖｜账号 {}】所有白嫖项目都已结束，正在合并最新登录状态。",
                    account_email
                ));
                Ok(AccountProgress::Completed {
                    checkin_result,
                    account,
                })
            })();
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
            })) => {
                if let Some(checkin_result) = checkin_result {
                    checkin_results.push(checkin_result);
                }
                merged_config = upsert_account(merged_config, account);
            }
            Ok(Err(error)) => {
                if first_error.is_none() {
                    first_error = Some(error);
                }
            }
            Err(_) => {
                if first_error.is_none() {
                    first_error = Some(io::Error::other(
                        "全自动白嫖任务提前结束，请查看前面的项目日志了解原因。",
                    ));
                }
                break;
            }
        }
    }

    for handle in handles {
        if handle.join().is_err() && first_error.is_none() {
            first_error = Some(io::Error::other(
                "全自动白嫖任务异常退出，请查看前面的项目日志了解原因。",
            ));
        }
    }

    if let Some(error) = first_error {
        return Err(error);
    }

    save_merged_config(merged_config.clone())?;
    log.line_fmt(format_args!(
        "【全自动白嫖】所有账号都已完成，已合并并保存最新登录状态（共 {} 个账号）。",
        merged_config.accounts.len()
    ));
    Ok((checkin_results, merged_config))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicBool;
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
    fn execute_all_free_features_runs_all_free_games_per_account_and_saves_once() {
        let config = sample_config(&["alpha@example.com", "beta@example.com"]);
        let cancel_flag = Arc::new(AtomicBool::new(false));
        let events = Arc::new(Mutex::new(Vec::<String>::new()));
        let saved_configs = Arc::new(Mutex::new(Vec::<AuthConfig>::new()));

        let log = ui::TaskLog::stdout();
        let (results, merged_config) = execute_all_free_features(
            config,
            &cancel_flag,
            &log,
            FreeFeatureRunners {
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
                                ..CheckinResult::default()
                            },
                        }))
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
                "sheepmatch",
                "puzzle_2048",
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
    fn execute_all_free_features_runs_features_for_same_account_concurrently() {
        let config = sample_config(&["fast@example.com"]);
        let cancel_flag = Arc::new(AtomicBool::new(false));
        let sheepmatch_started = Arc::new((Mutex::new(false), Condvar::new()));

        let log = ui::TaskLog::stdout();
        execute_all_free_features(
            config,
            &cancel_flag,
            &log,
            FreeFeatureRunners {
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
                                ..CheckinResult::default()
                            },
                        }))
                    }
                }),
                run_sheepmatch: Arc::new({
                    let sheepmatch_started = Arc::clone(&sheepmatch_started);
                    move |_config, account, _cancel_flag| {
                        if account.email == "fast@example.com" {
                            let (started, ready) = &*sheepmatch_started;
                            *started.lock().unwrap() = true;
                            ready.notify_all();
                        }
                        Ok(sheepmatch::AccountRunOutput { account })
                    }
                }),
                run_puzzle_2048: Arc::new(move |_config, account, _cancel_flag| {
                    Ok(puzzle_2048::AccountRunOutput { account })
                }),
                run_memory: Arc::new(move |_config, account, _cancel_flag| {
                    Ok(memory::AccountRunOutput { account })
                }),
                run_puzzle_15: Arc::new(move |_config, account, _cancel_flag| {
                    Ok(puzzle_15::AccountRunOutput { account })
                }),
                run_sudoku: Arc::new(move |_config, account, _cancel_flag| {
                    Ok(sudoku::AccountRunOutput { account })
                }),
                save_merged_config: Box::new(|_merged_config| Ok(())),
            },
        )
        .unwrap();
    }
}
