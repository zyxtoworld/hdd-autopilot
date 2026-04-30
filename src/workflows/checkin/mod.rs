mod auth;
mod log;
mod run;

use std::io;
use std::path::Path;
use std::sync::{Arc, Mutex};

use crate::api::ApiClient;
use crate::model::{AuthCache, AuthConfig, CheckinResult};
use crate::storage::{save_cache, upsert_account};
use crate::ui;
use crate::workflows::common::{AccountRewardSummary, format_amount, print_account_reward_summary};

use self::auth::{ensure_authenticated, load_auth_me_with_retry};
pub use self::log::{append_checkin_log, format_checkin_result_line};
use self::run::{humanize_account_status, humanize_balance_error, run_one_account};

#[derive(Debug)]
pub struct BatchState {
    pub config: AuthConfig,
    pub auth_cache_file: Option<std::path::PathBuf>,
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
pub struct AccountRuntime {
    pub api_client: ApiClient,
    pub account: AuthCache,
    pub auth_token: String,
}

impl AccountRuntime {
    pub fn email(&self) -> &str {
        self.account.email.trim()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct BalanceLine {
    pub email: String,
    pub balance: String,
    pub status: String,
}

#[derive(Debug, Clone)]
pub struct AccountCheckinOutput {
    pub account: AuthCache,
    pub result: CheckinResult,
}

pub fn run_batch(
    config: AuthConfig,
    auth_cache_file: impl AsRef<Path>,
    cancel_flag: &ui::CancelFlag,
    log: &ui::TaskLog,
) -> io::Result<Vec<CheckinResult>> {
    let state = Arc::new(Mutex::new(BatchState {
        config: config.clone(),
        auth_cache_file: Some(auth_cache_file.as_ref().to_path_buf()),
        log: log.clone(),
    }));
    let runtimes = new_account_runtimes(&config);
    let mut handles = Vec::with_capacity(runtimes.len());

    let mut reward_summaries = runtimes
        .iter()
        .enumerate()
        .map(|(index, runtime)| AccountRewardSummary {
            index,
            email: runtime.email().to_string(),
            total_reward: 0.0,
        })
        .collect::<Vec<_>>();

    for (index, runtime) in runtimes.into_iter().enumerate() {
        let state = Arc::clone(&state);
        let cancel_flag = Arc::clone(cancel_flag);
        handles.push(std::thread::spawn(move || {
            (index, run_one_account(&cancel_flag, state, runtime))
        }));
    }

    let mut results = Vec::new();
    for handle in handles {
        if let Ok((index, Some(result))) = handle.join() {
            if let Some(summary) = reward_summaries.get_mut(index) {
                summary.email = result.email.clone();
                summary.total_reward = result.delta;
            }
            results.push(result);
        }
    }
    print_account_reward_summary(log, "自动签到", &reward_summaries);
    Ok(results)
}

pub fn run_account(
    config: &AuthConfig,
    account: AuthCache,
    cancel_flag: &ui::CancelFlag,
) -> io::Result<Option<AccountCheckinOutput>> {
    run_account_with_log(config, account, cancel_flag, &ui::TaskLog::stdout())
}

pub fn run_account_with_log(
    config: &AuthConfig,
    account: AuthCache,
    cancel_flag: &ui::CancelFlag,
    log: &ui::TaskLog,
) -> io::Result<Option<AccountCheckinOutput>> {
    let fallback_account = account.clone();
    let state = Arc::new(Mutex::new(BatchState {
        config: AuthConfig {
            base_url: config.base_url.clone(),
            accounts: vec![account.clone()],
        },
        auth_cache_file: None,
        log: log.clone(),
    }));
    let runtime = AccountRuntime {
        api_client: ApiClient::new(&config.base_url),
        account,
        auth_token: String::new(),
    };
    let result = run_one_account(cancel_flag, Arc::clone(&state), runtime);
    let updated_account = state
        .lock()
        .unwrap()
        .config
        .accounts
        .first()
        .cloned()
        .unwrap_or(fallback_account);
    Ok(result.map(|result| AccountCheckinOutput {
        account: updated_account,
        result,
    }))
}

pub fn load_balance_lines(
    config: AuthConfig,
    auth_cache_file: impl AsRef<Path>,
) -> io::Result<(AuthConfig, Vec<BalanceLine>)> {
    let state = Arc::new(Mutex::new(BatchState {
        config: config.clone(),
        auth_cache_file: Some(auth_cache_file.as_ref().to_path_buf()),
        log: ui::TaskLog::stdout(),
    }));
    let runtimes = new_account_runtimes(&config);
    let mut handles = Vec::with_capacity(runtimes.len());

    for (index, runtime) in runtimes.into_iter().enumerate() {
        let state = Arc::clone(&state);
        handles.push(std::thread::spawn(move || {
            (index, load_one_balance_line(state, runtime))
        }));
    }

    let mut indexed = Vec::new();
    for handle in handles {
        if let Ok((index, line)) = handle.join() {
            indexed.push((index, line));
        }
    }
    indexed.sort_by_key(|(index, _)| *index);
    let config = state.lock().unwrap().config.clone();
    let lines = indexed
        .into_iter()
        .map(|(_, line)| line)
        .collect::<Vec<_>>();
    Ok((config, lines))
}

pub fn new_account_runtimes(config: &AuthConfig) -> Vec<AccountRuntime> {
    config
        .accounts
        .iter()
        .cloned()
        .map(|account| AccountRuntime {
            api_client: ApiClient::new(&config.base_url),
            account,
            auth_token: String::new(),
        })
        .collect()
}

fn load_one_balance_line(
    state: Arc<Mutex<BatchState>>,
    mut runtime: AccountRuntime,
) -> BalanceLine {
    let mut line = BalanceLine {
        email: runtime.email().to_string(),
        balance: "--".to_string(),
        status: "刷新失败".to_string(),
    };
    if let Err(error) = ensure_authenticated(&state, &mut runtime) {
        line.status = humanize_balance_error(&error);
        return line;
    }
    match load_auth_me_with_retry(&state, &mut runtime) {
        Ok(auth_me) => {
            let email = auth_me.data.email.trim();
            if !email.is_empty() {
                line.email = email.to_string();
            }
            line.balance = format_amount(auth_me.data.balance);
            line.status = humanize_account_status(&auth_me.data.status).to_string();
            line
        }
        Err(error) => {
            line.status = humanize_balance_error(&error);
            line
        }
    }
}
