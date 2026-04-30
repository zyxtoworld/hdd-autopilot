use std::sync::{Arc, Mutex};

use crate::api::{ApiError, is_unauthorized};
use crate::model::{AuthMeResponse, AuthSession};
use crate::storage::{
    build_authorization, cache_from_login, get_session, password_usable, upsert_session,
};

use super::{AccountRuntime, BatchState};

pub(super) fn load_auth_me_with_retry(
    state: &Arc<Mutex<BatchState>>,
    runtime: &mut AccountRuntime,
) -> Result<AuthMeResponse, ApiError> {
    match runtime.api_client.validate_auth_token(&runtime.auth_token) {
        Ok(response) => Ok(response),
        Err(error) if is_unauthorized(&error) => {
            state.lock().unwrap().log.line_fmt(format_args!(
                "账号 {} 的登录状态中途失效了，正在重新登录后继续。",
                runtime.email()
            ));
            reauthenticate(state, runtime)?;
            runtime.api_client.validate_auth_token(&runtime.auth_token)
        }
        Err(error) => Err(error),
    }
}

pub(super) fn ensure_authenticated(
    state: &Arc<Mutex<BatchState>>,
    runtime: &mut AccountRuntime,
) -> Result<(), ApiError> {
    let base_url = runtime.api_client.base_url().to_string();
    if let Some(session) = get_session(&runtime.account, &base_url) {
        if !session.cookies.is_empty() {
            runtime.api_client.load_session_cookies(&session.cookies)?;
            if let Ok(auth_me) = runtime.api_client.validate_auth_token("") {
                let email = auth_me.data.email.trim();
                if !email.is_empty()
                    && !runtime.account.email.trim().is_empty()
                    && !email.eq_ignore_ascii_case(runtime.account.email.trim())
                {
                    runtime.api_client.clear_session_cookies();
                    return Err(ApiError::Message(format!(
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
                state
                    .save_account(runtime.account.clone())
                    .map_err(|error| ApiError::Message(error.to_string()))?;
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
                state
                    .save_account(runtime.account.clone())
                    .map_err(|error| ApiError::Message(error.to_string()))?;
                return Ok(());
            }
            state.lock().unwrap().log.line_fmt(format_args!(
                "账号 {} 的上次登录信息已经失效，准备重新登录。",
                runtime.email()
            ));
        }
    }
    if !password_usable(&runtime.account) {
        return Err(ApiError::Message(format!(
            "账号 {} 没有保存密码，无法自动重新登录",
            runtime.email()
        )));
    }
    let (login_response, auth_token) = runtime
        .api_client
        .do_login(&runtime.account.email, &runtime.account.password)?;
    runtime.account = cache_from_login(
        &login_response,
        &runtime.account.email,
        &runtime.account.password,
        &base_url,
        runtime.api_client.export_session_cookies(),
    );
    runtime.auth_token = auth_token;
    let mut state = state.lock().unwrap();
    state
        .save_account(runtime.account.clone())
        .map_err(|error| ApiError::Message(error.to_string()))?;
    Ok(())
}

pub(super) fn reauthenticate(
    state: &Arc<Mutex<BatchState>>,
    runtime: &mut AccountRuntime,
) -> Result<(), ApiError> {
    state.lock().unwrap().log.line_fmt(format_args!(
        "检测到账号 {} 的登录状态失效，尝试重新登录。",
        runtime.email()
    ));
    runtime.auth_token.clear();
    ensure_authenticated(state, runtime)
}
