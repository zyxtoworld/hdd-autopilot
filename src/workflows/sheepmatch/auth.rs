use std::io;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::api::{ApiClient, ApiError, is_unauthorized};
use crate::model::AuthSession;
use crate::storage::{
    build_authorization, cache_from_login, get_session, password_usable, upsert_session,
};
use crate::ui;
use crate::workflows::common::{humanize_retryable_api_error, is_retryable_api_error};

use super::{AccountRuntime, BatchState};

const API_RETRY_BACKOFF: Duration = Duration::from_millis(500);
const API_RETRY_LOG_EVERY: usize = 10;

pub(super) fn ensure_authenticated(
    state: &Arc<Mutex<BatchState>>,
    runtime: &mut AccountRuntime,
) -> io::Result<()> {
    let base_url = runtime.api_client.base_url().to_string();
    if let Some(session) = get_session(&runtime.account, &base_url) {
        if !session.cookies.is_empty() {
            runtime
                .api_client
                .load_session_cookies(&session.cookies)
                .map_err(api_error_to_io_error)?;
            if let Ok(auth_me) = runtime.api_client.validate_auth_token("") {
                let email = auth_me.data.email.trim();
                if !email.is_empty()
                    && !runtime.account.email.trim().is_empty()
                    && !email.eq_ignore_ascii_case(runtime.account.email.trim())
                {
                    runtime.api_client.clear_session_cookies();
                    return Err(io::Error::other(format!(
                        "账号 {} 读取到的登录状态属于另一个账号 {}，请重新登录或检查 auth.json",
                        runtime.account.email.trim(),
                        email
                    )));
                }
                if !email.is_empty() {
                    runtime.account.email = email.to_string();
                }
                runtime.account = upsert_session(
                    runtime.account.clone(),
                    AuthSession {
                        base_url: base_url.clone(),
                        token_type: session.token_type.clone(),
                        access_token: session.access_token.clone(),
                        cookies: runtime.api_client.export_session_cookies(),
                    },
                );
                runtime.auth_token.clear();
                let mut state = state.lock().unwrap();
                state.save_account(runtime.account.clone())?;
                return Ok(());
            }
            runtime.api_client.clear_session_cookies();
            state.lock().unwrap().log.line_fmt(format_args!(
                "账号 {} 的上次登录状态已经过期，继续尝试其他方式恢复登录。",
                runtime.email()
            ));
        }

        let auth_token = build_authorization(&session.token_type, &session.access_token);
        if !auth_token.is_empty() {
            if let Ok(auth_me) = runtime.api_client.validate_auth_token(&auth_token) {
                if !auth_me.data.email.trim().is_empty() {
                    runtime.account.email = auth_me.data.email.trim().to_string();
                }
                runtime.account = upsert_session(
                    runtime.account.clone(),
                    AuthSession {
                        base_url: base_url.clone(),
                        token_type: session.token_type,
                        access_token: session.access_token,
                        cookies: runtime.api_client.export_session_cookies(),
                    },
                );
                runtime.auth_token = auth_token;
                let mut state = state.lock().unwrap();
                state.save_account(runtime.account.clone())?;
                return Ok(());
            }
            state.lock().unwrap().log.line_fmt(format_args!(
                "账号 {} 的上次登录信息已经失效，准备重新登录。",
                runtime.email()
            ));
        }
    }

    if !password_usable(&runtime.account) {
        return Err(io::Error::other(format!(
            "账号 {} 没有保存密码，无法自动重新登录",
            runtime.email()
        )));
    }

    let (login_response, auth_token) = runtime
        .api_client
        .do_login(&runtime.account.email, &runtime.account.password)
        .map_err(api_error_to_io_error)?;
    runtime.account = cache_from_login(
        &login_response,
        &runtime.account.email,
        &runtime.account.password,
        &base_url,
        runtime.api_client.export_session_cookies(),
    );
    runtime.auth_token = auth_token;
    let mut state = state.lock().unwrap();
    state.save_account(runtime.account.clone())
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
                ui::sleep_with_cancel(cancel_flag, API_RETRY_BACKOFF)?;
            }
            Err(error) => return Err(api_error_to_io_error(error)),
        }
    }
}

fn api_error_to_io_error(error: ApiError) -> io::Error {
    io::Error::other(error.to_string())
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
