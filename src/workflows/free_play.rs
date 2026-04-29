use std::io;
use std::sync::Arc;
use std::sync::mpsc;

use crate::model::{AuthCache, AuthConfig, CheckinResult};
use crate::storage::upsert_account;
use crate::ui;
use crate::workflows::{checkin, sheepmatch};

pub(crate) fn execute_all_free_features<FCheckin, FSheepmatch, FSave>(
    original_config: AuthConfig,
    cancel_flag: &ui::CancelFlag,
    log: &ui::TaskLog,
    run_checkin: FCheckin,
    run_sheepmatch: FSheepmatch,
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
    FSave: Fn(AuthConfig) -> io::Result<()> + Send + 'static,
{
    enum AccountProgress {
        Completed {
            checkin_result: CheckinResult,
            account: AuthCache,
        },
        Skipped,
    }

    let run_checkin = Arc::new(run_checkin);
    let run_sheepmatch = Arc::new(run_sheepmatch);
    let (result_tx, result_rx) = mpsc::channel::<io::Result<AccountProgress>>();
    let mut handles = Vec::with_capacity(original_config.accounts.len());

    for account in original_config.accounts.clone() {
        ui::check_cancel(cancel_flag)?;
        let cancel_flag = Arc::clone(cancel_flag);
        let base_config = original_config.clone();
        let run_checkin = Arc::clone(&run_checkin);
        let run_sheepmatch = Arc::clone(&run_sheepmatch);
        let result_tx = result_tx.clone();
        handles.push(std::thread::spawn(move || {
            let result = (|| -> io::Result<AccountProgress> {
                let checkin_output = match run_checkin(&base_config, account, &cancel_flag)? {
                    Some(output) => output,
                    None => return Ok(AccountProgress::Skipped),
                };
                ui::check_cancel(&cancel_flag)?;
                let mut account_config = base_config.clone();
                account_config.accounts = vec![checkin_output.account.clone()];
                let sheepmatch_output = run_sheepmatch(
                    &account_config,
                    checkin_output.account.clone(),
                    &cancel_flag,
                )?;
                Ok(AccountProgress::Completed {
                    checkin_result: checkin_output.result,
                    account: sheepmatch_output.account,
                })
            })();
            let _ = result_tx.send(result);
        }));
    }
    drop(result_tx);

    let mut merged_config = original_config.clone();
    let mut checkin_results = Vec::new();
    for _ in 0..handles.len() {
        match result_rx.recv() {
            Ok(Ok(AccountProgress::Completed {
                checkin_result,
                account,
            })) => {
                log.line(checkin::format_checkin_result_line(&checkin_result));
                checkin_results.push(checkin_result);
                merged_config = upsert_account(merged_config, account);
            }
            Ok(Ok(AccountProgress::Skipped)) => {}
            Ok(Err(error)) if error.kind() == io::ErrorKind::Interrupted => return Err(error),
            Ok(Err(error)) => return Err(error),
            Err(_) => {
                return Err(io::Error::other(
                    "全自动白嫖线程提前结束：后台线程发生了未处理异常。",
                ));
            }
        }
    }

    for handle in handles {
        if handle.join().is_err() {
            return Err(io::Error::other(
                "全自动白嫖线程提前结束：后台线程发生了未处理异常。",
            ));
        }
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
    fn execute_all_free_features_runs_each_account_in_menu_order_and_saves_once() {
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
                move |_config, account, _cancel_flag| {
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
                    assert_eq!(account.password, "after-checkin");
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
            let checkin_index = events
                .iter()
                .position(|event| event == &format!("checkin:{email}"))
                .unwrap();
            let sheepmatch_index = events
                .iter()
                .position(|event| event == &format!("sheepmatch:{email}"))
                .unwrap();
            assert!(
                checkin_index < sheepmatch_index,
                "expected checkin before sheepmatch for {email}, got {events:?}"
            );
        }

        assert!(
            merged_config
                .accounts
                .iter()
                .all(|account| account.access_token == "after-sheepmatch")
        );

        let saved_configs = saved_configs.lock().unwrap();
        assert_eq!(saved_configs.len(), 1);
        assert_eq!(saved_configs[0], merged_config);
    }

    #[test]
    fn execute_all_free_features_advances_per_account_instead_of_global_stages() {
        let config = sample_config(&["fast@example.com", "slow@example.com"]);
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
                    if account.email == "slow@example.com" {
                        let (started, ready) = &*sheepmatch_started;
                        let started = started.lock().unwrap();
                        let wait = ready
                            .wait_timeout_while(started, Duration::from_secs(1), |started| !*started)
                            .unwrap();
                        if !*wait.0 {
                            return Err(io::Error::other(
                                "other account did not reach sheepmatch before slow checkin finished",
                            ));
                        }
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
            |_merged_config| Ok(()),
        )
        .unwrap();
    }
}
