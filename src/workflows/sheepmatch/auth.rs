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

fn reauthenticate(state: &Arc<Mutex<BatchState>>, runtime: &mut AccountRuntime) -> io::Result<()> {
    state.lock().unwrap().log.line_fmt(format_args!(
        "检测到账号 {} 的登录状态失效，尝试重新登录。",
        runtime.email()
    ));
    runtime.auth_token.clear();
    ensure_authenticated(state, runtime)
}

pub(super) fn with_auth_retry_api<T, F>(
    state: &Arc<Mutex<BatchState>>,
    runtime: &mut AccountRuntime,
    action: F,
) -> Result<T, ApiError>
where
    F: Fn(&ApiClient, &str) -> Result<T, ApiError>,
{
    match action(&runtime.api_client, &runtime.auth_token) {
        Ok(value) => Ok(value),
        Err(error) if is_unauthorized(&error) => {
            state.lock().unwrap().log.line_fmt(format_args!(
                "账号 {} 的登录状态中途失效了，正在重新登录后继续。",
                runtime.email()
            ));
            reauthenticate(state, runtime).map_err(|error| ApiError::Message(error.to_string()))?;
            action(&runtime.api_client, &runtime.auth_token)
        }
        Err(error) => Err(error),
    }
}

#[cfg(test)]
pub(super) fn with_auth_retry<T, F>(
    state: &Arc<Mutex<BatchState>>,
    runtime: &mut AccountRuntime,
    action: F,
) -> io::Result<T>
where
    F: Fn(&ApiClient, &str) -> Result<T, ApiError>,
{
    with_auth_retry_api(state, runtime, action).map_err(api_error_to_io_error)
}

pub(super) fn with_auth_retry_until_success<T, F>(
    cancel_flag: &ui::CancelFlag,
    state: &Arc<Mutex<BatchState>>,
    runtime: &mut AccountRuntime,
    operation: &str,
    action: F,
) -> io::Result<T>
where
    F: Fn(&ApiClient, &str) -> Result<T, ApiError>,
{
    let mut attempts = 0usize;
    loop {
        ui::check_cancel(cancel_flag)?;
        attempts += 1;
        match with_auth_retry_api(state, runtime, &action) {
            Ok(value) => return Ok(value),
            Err(error) if is_retryable_api_error(&error) => {
                if attempts == 1 || attempts.is_multiple_of(API_RETRY_LOG_EVERY) {
                    state.lock().unwrap().log.line_fmt(format_args!(
                        "账号 {} 的{}暂时连不上，会等接口恢复后继续（第 {} 次尝试）：{}",
                        runtime.email(),
                        localized_retry_operation(operation),
                        attempts,
                        humanize_retryable_api_error(&error)
                    ));
                }
                if attempts >= API_RETRY_MAX_ATTEMPTS {
                    state.lock().unwrap().log.line_fmt(format_args!(
                        "账号 {} 的{}连续重试 {} 次仍失败，准备重新进入玩法续残局：{}",
                        runtime.email(),
                        localized_retry_operation(operation),
                        attempts,
                        humanize_retryable_api_error(&error)
                    ));
                    return Err(retry_exhausted_error(operation, attempts, &error));
                }
                ui::sleep_with_cancel(cancel_flag, API_RETRY_BACKOFF)?;
            }
            Err(error) => return Err(api_error_to_io_error(error)),
        }
    }
}

fn api_error_to_io_error(error: ApiError) -> io::Error {
    io::Error::other(error.to_string())
}

fn retry_exhausted_error(operation: &str, attempts: usize, error: &ApiError) -> io::Error {
    io::Error::new(
        io::ErrorKind::TimedOut,
        format!(
            "{}连续重试 {} 次仍失败，准备重新进入玩法续残局：{}",
            localized_retry_operation(operation),
            attempts,
            humanize_retryable_api_error(error)
        ),
    )
}

fn localized_retry_operation(operation: &str) -> &'static str {
    match operation {
        "tile config" => "羊了个羊配置接口",
        "tile me" => "羊了个羊次数接口",
        "tile history" => "羊了个羊历史接口",
        "tile start" => "羊了个羊开局接口",
        _ => "羊了个羊接口",
    }
}
