use std::sync::{Arc, Mutex};

use crate::api::{ApiError, is_unauthorized};
use crate::model::AuthMeResponse;
use crate::workflows::common::ensure_authenticated_session;

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
    let log = state.lock().unwrap().log.clone();
    ensure_authenticated_session(
        &log,
        &mut runtime.api_client,
        &mut runtime.account,
        &mut runtime.auth_token,
        |account| state.lock().unwrap().save_account(account),
    )
    .map_err(|error| ApiError::Message(error.to_string()))
}

pub(super) fn ensure_authenticated_for_checkin(
    state: &Arc<Mutex<BatchState>>,
    runtime: &mut AccountRuntime,
) -> Result<(), ApiError> {
    ensure_authenticated(state, runtime)
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
