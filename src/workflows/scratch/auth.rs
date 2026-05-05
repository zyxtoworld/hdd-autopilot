use std::io;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::api::{ApiClient, ApiError, is_unauthorized};
use crate::ui;
use crate::workflows::common::{
    API_RETRY_MAX_ATTEMPTS, ensure_authenticated_session, humanize_retryable_api_error,
    is_retryable_api_error,
};

use super::{AccountRuntime, BatchState};

const API_RETRY_BACKOFF: Duration = Duration::from_millis(500);
const API_RETRY_LOG_EVERY: usize = 10;

pub(super) fn with_auth_retry<T, F>(
    cancel_flag: &ui::CancelFlag,
    state: &Arc<Mutex<BatchState>>,
    runtime: &mut AccountRuntime,
    action: F,
) -> io::Result<T>
where
    F: Fn(&ApiClient, &str) -> Result<T, ApiError>,
{
    let mut attempts = 0usize;
    loop {
        ui::check_cancel(cancel_flag)?;
        attempts += 1;
        let result = match action(&runtime.api_client, &runtime.auth_token) {
            Ok(value) => Ok(value),
            Err(error) if is_unauthorized(&error) => {
                state.lock().unwrap().log.line_fmt(format_args!(
                    "账号 {} 的登录状态中途失效了，正在重新登录后继续。",
                    runtime.email()
                ));
                reauthenticate(state, runtime)?;
                action(&runtime.api_client, &runtime.auth_token)
            }
            other => other,
        };
        match result {
            Ok(value) => return Ok(value),
            Err(error) if is_retryable_api_error(&error) => {
                if attempts == 1 || attempts.is_multiple_of(API_RETRY_LOG_EVERY) {
                    state.lock().unwrap().log.line_fmt(format_args!(
                        "账号 {} 的刮刮乐接口暂时连不上，会继续等接口恢复后再试（第 {} 次尝试）：{}",
                        runtime.email(),
                        attempts,
                        humanize_retryable_api_error(&error)
                    ));
                }
                if attempts >= API_RETRY_MAX_ATTEMPTS {
                    state.lock().unwrap().log.line_fmt(format_args!(
                        "账号 {} 的刮刮乐接口连续重试 {} 次仍失败，准备重新进入玩法：{}",
                        runtime.email(),
                        attempts,
                        humanize_retryable_api_error(&error)
                    ));
                    return Err(io::Error::new(
                        io::ErrorKind::TimedOut,
                        format!(
                            "刮刮乐接口连续重试 {} 次仍失败，准备重新进入玩法：{}",
                            attempts,
                            humanize_retryable_api_error(&error)
                        ),
                    ));
                }
                ui::sleep_with_cancel(cancel_flag, API_RETRY_BACKOFF)?;
            }
            Err(error) => return Err(api_error_to_io_error(error)),
        }
    }
}

pub(super) fn ensure_authenticated(
    state: &Arc<Mutex<BatchState>>,
    runtime: &mut AccountRuntime,
) -> io::Result<()> {
    let log = state.lock().unwrap().log.clone();
    ensure_authenticated_session(
        &log,
        &mut runtime.api_client,
        &mut runtime.account,
        &mut runtime.auth_token,
        |account| state.lock().unwrap().save_account(account),
    )
}

pub(super) fn reauthenticate(
    state: &Arc<Mutex<BatchState>>,
    runtime: &mut AccountRuntime,
) -> io::Result<()> {
    state.lock().unwrap().log.line_fmt(format_args!(
        "检测到账号 {} 的登录状态失效，尝试重新登录。",
        runtime.email()
    ));
    runtime.auth_token.clear();
    ensure_authenticated(state, runtime)
}

pub(super) fn api_error_to_io_error(error: ApiError) -> io::Error {
    io::Error::other(error.to_string())
}
