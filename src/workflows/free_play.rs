use std::io;
use std::sync::Arc;
use std::sync::mpsc;

use crate::model::{AuthCache, AuthConfig, CheckinResult};
use crate::storage::upsert_account;
use crate::ui;
use crate::workflows::{checkin, memory, puzzle_15, puzzle_2048, sheepmatch, sudoku};

pub(crate) fn execute_all_free_features<
    FCheckin,
    FSheepmatch,
    FPuzzle2048Runner,
    FMemoryRunner,
    FPuzzle15Runner,
    FSudokuRunner,
    FSave,
>(
    original_config: AuthConfig,
    cancel_flag: &ui::CancelFlag,
    log: &ui::TaskLog,
    run_checkin: FCheckin,
    run_sheepmatch: FSheepmatch,
    run_puzzle_2048: FPuzzle2048Runner,
    run_memory: FMemoryRunner,
    run_puzzle_15: FPuzzle15Runner,
    run_sudoku: FSudokuRunner,
    save_merged_config: FSave,
) -> io::Result<(Vec<CheckinResult>, AuthConfig)>
where
    FCheckin: Fn(
            &AuthConfig,
            AuthCache,
            &ui::CancelFlag,
        ) -> io::Result<Option<checkin::AccountCheckinOutput>>
        + Send
        + Sync
        + 'static,
    FSheepmatch: Fn(&AuthConfig, AuthCache, &ui::CancelFlag) -> io::Result<sheepmatch::AccountRunOutput>
        + Send
        + Sync
        + 'static,
    FPuzzle2048Runner: Fn(&AuthConfig, AuthCache, &ui::CancelFlag) -> io::Result<puzzle_2048::AccountRunOutput>
        + Send
        + Sync
        + 'static,
    FMemoryRunner: Fn(&AuthConfig, AuthCache, &ui::CancelFlag) -> io::Result<memory::AccountRunOutput>
        + Send
        + Sync
        + 'static,
    FPuzzle15Runner: Fn(&AuthConfig, AuthCache, &ui::CancelFlag) -> io::Result<puzzle_15::AccountRunOutput>
        + Send
        + Sync
        + 'static,
    FSudokuRunner: Fn(&AuthConfig, AuthCache, &ui::CancelFlag) -> io::Result<sudoku::AccountRunOutput>
        + Send
        + Sync
        + 'static,
    FSave: Fn(AuthConfig) -> io::Result<()> + Send + 'static,
{
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

    let run_checkin = Arc::new(run_checkin);
    let run_sheepmatch = Arc::new(run_sheepmatch);
    let run_puzzle_2048 = Arc::new(run_puzzle_2048);
    let run_memory = Arc::new(run_memory);
    let run_puzzle_15 = Arc::new(run_puzzle_15);
    let run_sudoku = Arc::new(run_sudoku);
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
        handles.push(std::thread::spawn(move || {
            let result = (|| -> io::Result<AccountProgress> {
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
                        move || {
                            run_checkin(&account_config, account, &cancel_flag)
                                .map(FeatureProgress::Checkin)
                        }
                    }),
                    std::thread::spawn({
                        let account_config = account_config.clone();
                        let account = account.clone();
                        let cancel_flag = Arc::clone(&cancel_flag);
                        let run_sheepmatch = Arc::clone(&run_sheepmatch);
                        move || {
                            run_sheepmatch(&account_config, account, &cancel_flag)
                                .map(FeatureProgress::Sheepmatch)
                        }
                    }),
                    std::thread::spawn({
                        let account_config = account_config.clone();
                        let account = account.clone();
                        let cancel_flag = Arc::clone(&cancel_flag);
                        let run_puzzle_2048 = Arc::clone(&run_puzzle_2048);
                        move || {
                            run_puzzle_2048(&account_config, account, &cancel_flag)
                                .map(FeatureProgress::Puzzle2048)
                        }
                    }),
                    std::thread::spawn({
                        let account_config = account_config.clone();
                        let account = account.clone();
                        let cancel_flag = Arc::clone(&cancel_flag);
                        let run_memory = Arc::clone(&run_memory);
                        move || {
                            run_memory(&account_config, account, &cancel_flag)
                                .map(FeatureProgress::Memory)
                        }
                    }),
                    std::thread::spawn({
                        let account_config = account_config.clone();
                        let account = account.clone();
                        let cancel_flag = Arc::clone(&cancel_flag);
                        let run_puzzle_15 = Arc::clone(&run_puzzle_15);
                        move || {
                            run_puzzle_15(&account_config, account, &cancel_flag)
                                .map(FeatureProgress::Puzzle15)
                        }
                    }),
                    std::thread::spawn({
                        let account_config = account_config.clone();
                        let account = account.clone();
                        let cancel_flag = Arc::clone(&cancel_flag);
                        let run_sudoku = Arc::clone(&run_sudoku);
                        move || {
                            run_sudoku(&account_config, account, &cancel_flag)
                                .map(FeatureProgress::Sudoku)
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
                                    "全自动白嫖子任务提前结束：后台线程发生了未处理异常。",
                                ));
                            }
                        }
                    }
                }

                if let Some(error) = first_error {
                    return Err(error);
                }

                let account = merged_config.accounts.first().cloned().unwrap_or(account);
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
                    log.line(checkin::format_checkin_result_line(&checkin_result));
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
                        "全自动白嫖线程提前结束：后台线程发生了未处理异常。",
                    ));
                }
                break;
            }
        }
    }

    for handle in handles {
        if handle.join().is_err() {
            if first_error.is_none() {
                first_error = Some(io::Error::other(
                    "全自动白嫖线程提前结束：后台线程发生了未处理异常。",
                ));
            }
        }
    }

    if let Some(error) = first_error {
        return Err(error);
    }

    save_merged_config(merged_config.clone())?;
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
            {
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
            },
            {
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
            },
            {
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
            },
            {
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
            },
            {
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
            },
            {
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
            },
            {
                let saved_configs = Arc::clone(&saved_configs);
                move |merged_config| {
                    saved_configs.lock().unwrap().push(merged_config);
                    Ok(())
                }
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
            {
                let sheepmatch_started = Arc::clone(&sheepmatch_started);
                move |_config, account, _cancel_flag| {
                    let (started, ready) = &*sheepmatch_started;
                    let started = started.lock().unwrap();
                    let wait = ready
                        .wait_timeout_while(started, Duration::from_secs(1), |started| !*started)
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
            },
            {
                let sheepmatch_started = Arc::clone(&sheepmatch_started);
                move |_config, account, _cancel_flag| {
                    if account.email == "fast@example.com" {
                        let (started, ready) = &*sheepmatch_started;
                        *started.lock().unwrap() = true;
                        ready.notify_all();
                    }
                    Ok(sheepmatch::AccountRunOutput { account })
                }
            },
            move |_config, account, _cancel_flag| Ok(puzzle_2048::AccountRunOutput { account }),
            move |_config, account, _cancel_flag| Ok(memory::AccountRunOutput { account }),
            move |_config, account, _cancel_flag| Ok(puzzle_15::AccountRunOutput { account }),
            move |_config, account, _cancel_flag| Ok(sudoku::AccountRunOutput { account }),
            |_merged_config| Ok(()),
        )
        .unwrap();
    }
}
