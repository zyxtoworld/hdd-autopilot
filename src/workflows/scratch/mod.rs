mod auth;
mod log;
mod round;

use std::io;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::api::ApiClient;
use crate::model::{AuthCache, AuthConfig};
use crate::runtime::resolve_data_file_path;
use crate::storage::{save_cache, upsert_account};
use crate::ui;
use crate::workflows::common::run_account_task_until_complete;

use self::auth::ensure_authenticated;
use self::round::{RoundLoop, run_one_round, settle_pending_rounds};

pub const DONE_MESSAGE: &str = "自动随机刮刮乐已完成。";

#[derive(Debug, Clone)]
pub struct RunOptions {
    pub history_retries: usize,
    pub history_wait: Duration,
}

impl Default for RunOptions {
    fn default() -> Self {
        Self {
            history_retries: 3,
            history_wait: Duration::from_millis(400),
        }
    }
}

#[derive(Debug, Clone)]
pub struct BatchState {
    pub config: AuthConfig,
    pub auth_cache_file: std::path::PathBuf,
    pub log: ui::TaskLog,
}

impl BatchState {
    pub fn save_account(&mut self, account: AuthCache) -> io::Result<()> {
        self.config = upsert_account(self.config.clone(), account);
        save_cache(&self.auth_cache_file, self.config.clone())
    }
}

#[derive(Debug, Clone)]
pub struct AccountRuntime {
    pub api_client: ApiClient,
    pub account: AuthCache,
    pub auth_token: String,
    pub total_cost: f64,
    pub total_reward: f64,
    pub rounds_played: i32,
}

impl AccountRuntime {
    pub fn email(&self) -> &str {
        self.account.email.trim()
    }
}

pub fn run_batch(config: AuthConfig, auth_cache_file: impl AsRef<Path>) -> io::Result<AuthConfig> {
    if config.accounts.is_empty() {
        println!("当前还没有可用账号。");
        return Ok(config);
    }

    let original_config = config.clone();
    let auth_cache_file = auth_cache_file.as_ref().to_path_buf();
    let result = ui::run_with_escape_interrupt(
        "自动随机刮刮乐运行中。",
        Some(DONE_MESSAGE),
        move |cancel_flag, log| {
            let state = Arc::new(Mutex::new(BatchState {
                config: config.clone(),
                auth_cache_file,
                log: log.clone(),
            }));
            let runtimes = new_account_runtimes(&config);
            let options = RunOptions::default();
            let log_dir = resolve_data_file_path("log/scratch");
            let mut handles = Vec::with_capacity(runtimes.len());

            log.line_fmt(format_args!(
                "开始自动随机刮刮乐，本次会处理 {} 个账号；每个账号都会一直玩到今天剩余次数用完。",
                runtimes.len()
            ));

            for runtime in runtimes {
                let cancel_flag = Arc::clone(&cancel_flag);
                let state = Arc::clone(&state);
                let options = options.clone();
                let log_dir = log_dir.clone();
                handles.push(std::thread::spawn(move || {
                    run_account_worker(&cancel_flag, state, runtime, &options, &log_dir)
                }));
            }

            for handle in handles {
                match handle.join() {
                    Ok(Ok(())) => {}
                    Ok(Err(error)) if error.kind() == io::ErrorKind::Interrupted => {}
                    Ok(Err(error)) => {
                        log.line_fmt(format_args!("自动随机刮刮乐线程提前结束：{}", error));
                    }
                    Err(_) => {
                        log.line("自动随机刮刮乐任务异常退出，请查看前面的账号日志定位原因。");
                    }
                }
            }

            Ok(state.lock().unwrap().config.clone())
        },
    )?;

    match result {
        Some(updated) => Ok(updated),
        None => Ok(original_config),
    }
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
            total_cost: 0.0,
            total_reward: 0.0,
            rounds_played: 0,
        })
        .collect()
}

fn run_account_worker(
    cancel_flag: &ui::CancelFlag,
    state: Arc<Mutex<BatchState>>,
    mut runtime: AccountRuntime,
    options: &RunOptions,
    log_dir: &Path,
) -> io::Result<()> {
    state
        .lock()
        .unwrap()
        .log
        .line_fmt(format_args!("当前账号：{}", runtime.email()));
    let email = runtime.email().to_string();
    let task_log = state.lock().unwrap().log.clone();
    run_account_task_until_complete(
        cancel_flag,
        &task_log,
        "自动随机刮刮乐",
        &email,
        || run_account_until_complete(cancel_flag, &state, &mut runtime, options, log_dir),
    )
}

fn run_account_until_complete(
    cancel_flag: &ui::CancelFlag,
    state: &Arc<Mutex<BatchState>>,
    runtime: &mut AccountRuntime,
    options: &RunOptions,
    log_dir: &Path,
) -> io::Result<()> {
    ensure_authenticated(state, runtime)?;
    settle_pending_rounds(cancel_flag, state, runtime, options, log_dir)?;

    loop {
        ui::check_cancel(cancel_flag)?;
        match run_one_round(cancel_flag, state, runtime, options, log_dir)? {
            RoundLoop::Continue => continue,
            RoundLoop::Done => {
                state.lock().unwrap().log.line_fmt(format_args!(
                    "账号 {} 今天的刮刮乐次数已经用完。",
                    runtime.email()
                ));
                return Ok(());
            }
            RoundLoop::Error(error) => return Err(error),
        }
    }
}
