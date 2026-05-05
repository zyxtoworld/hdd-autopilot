use std::io;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::api::ApiError;
use crate::model::{CheckinClaimResponse, CheckinMeResponse, CheckinResult, CheckinTodayResponse};
use crate::runtime::resolve_data_file_path;
use crate::ui;
use crate::workflows::common::{
    current_unix_ms, humanize_retryable_api_error, is_retryable_api_error,
};

use super::auth::{ensure_authenticated_for_checkin, reauthenticate};
use super::log::append_checkin_log;
use super::{AccountRuntime, BatchState};

const CHECKIN_RETRY_LOG_EVERY: usize = 10;
#[cfg(not(test))]
const CHECKIN_API_RETRY_MAX_ATTEMPTS: usize = 3;
#[cfg(test)]
const CHECKIN_API_RETRY_MAX_ATTEMPTS: usize = 2;
#[cfg(not(test))]
const CHECKIN_RETRY_BACKOFF: Duration = Duration::from_millis(500);
#[cfg(test)]
const CHECKIN_RETRY_BACKOFF: Duration = Duration::ZERO;

pub(super) fn run_one_account(
    cancel_flag: &ui::CancelFlag,
    state: Arc<Mutex<BatchState>>,
    runtime: AccountRuntime,
) -> Option<CheckinResult> {
    let email = runtime.email().to_string();
    match run_one_account_inner(cancel_flag, Arc::clone(&state), runtime) {
        Ok(result) => result,
        Err(error) if error.kind() == io::ErrorKind::Interrupted => None,
        Err(error) => Some(record_checkin_result(
            &state,
            failure_checkin_result(&email, error.to_string()),
        )),
    }
}

pub(super) fn run_one_account_inner(
    cancel_flag: &ui::CancelFlag,
    state: Arc<Mutex<BatchState>>,
    mut runtime: AccountRuntime,
) -> io::Result<Option<CheckinResult>> {
    ui::check_cancel(cancel_flag)?;
    let email = runtime.email().to_string();
    if let Err(error) =
        retry_checkin_api_until_success(cancel_flag, &state, &email, "签到登录状态", || {
            ensure_authenticated_for_checkin(&state, &mut runtime)
        })?
    {
        return Ok(Some(record_checkin_result(
            &state,
            failure_checkin_result(&email, error.to_string()),
        )));
    }

    ui::check_cancel(cancel_flag)?;
    let result = run_checkin_with_retry(cancel_flag, &state, &mut runtime, &email)?;
    Ok(Some(record_checkin_result(&state, result)))
}

fn record_checkin_result(state: &Arc<Mutex<BatchState>>, result: CheckinResult) -> CheckinResult {
    let log_dir = resolve_data_file_path("log/checkin");
    let _ = append_checkin_log(&log_dir, &result);
    state
        .lock()
        .unwrap()
        .log
        .line(crate::workflows::checkin::format_checkin_result_line(
            &result,
        ));
    result
}

pub(super) fn run_checkin_with_retry(
    cancel_flag: &ui::CancelFlag,
    state: &Arc<Mutex<BatchState>>,
    runtime: &mut AccountRuntime,
    email: &str,
) -> io::Result<CheckinResult> {
    match retry_checkin_api_until_success(cancel_flag, state, email, "签到接口", || {
        run_checkin_once_with_auth_retry(state, runtime, email)
    })? {
        Ok(result) => Ok(result),
        Err(error) => Ok(failure_checkin_result(
            email,
            checkin_api_error_message(&error),
        )),
    }
}

fn run_checkin_once_with_auth_retry(
    state: &Arc<Mutex<BatchState>>,
    runtime: &mut AccountRuntime,
    email: &str,
) -> Result<CheckinResult, ApiError> {
    match run_checkin(runtime, email) {
        Ok(result) => Ok(result),
        Err(error) if crate::api::is_unauthorized(&error) => {
            state.lock().unwrap().log.line_fmt(format_args!(
                "账号 {} 的登录状态中途失效了，正在重新登录后继续。",
                runtime.email()
            ));
            reauthenticate(state, runtime)?;
            ensure_authenticated_for_checkin(state, runtime)?;
            run_checkin(runtime, email)
        }
        Err(error) => Err(error),
    }
}

fn retry_checkin_api_until_success<T, F>(
    cancel_flag: &ui::CancelFlag,
    state: &Arc<Mutex<BatchState>>,
    email: &str,
    operation: &str,
    mut action: F,
) -> io::Result<Result<T, ApiError>>
where
    F: FnMut() -> Result<T, ApiError>,
{
    let mut attempts = 0usize;
    loop {
        ui::check_cancel(cancel_flag)?;
        attempts += 1;
        match action() {
            Ok(value) => return Ok(Ok(value)),
            Err(error) if is_retryable_api_error(&error) => {
                log_retryable_checkin_api_error(state, email, operation, attempts, &error);
                if attempts >= CHECKIN_API_RETRY_MAX_ATTEMPTS {
                    log_retry_exhausted_checkin_api_error(
                        state, email, operation, attempts, &error,
                    );
                    return Ok(Err(error));
                }
                ui::sleep_with_cancel(cancel_flag, CHECKIN_RETRY_BACKOFF)?;
            }
            Err(error) => return Ok(Err(error)),
        }
    }
}

fn log_retryable_checkin_api_error(
    state: &Arc<Mutex<BatchState>>,
    email: &str,
    operation: &str,
    attempts: usize,
    error: &ApiError,
) {
    if attempts == 1 || attempts % CHECKIN_RETRY_LOG_EVERY == 0 {
        state.lock().unwrap().log.line_fmt(format_args!(
            "账号 {} 的{}暂时连不上，会继续等接口恢复后再试（第 {} 次尝试）：{}",
            email,
            operation,
            attempts,
            humanize_retryable_api_error(error)
        ));
    }
}

fn log_retry_exhausted_checkin_api_error(
    state: &Arc<Mutex<BatchState>>,
    email: &str,
    operation: &str,
    attempts: usize,
    error: &ApiError,
) {
    state.lock().unwrap().log.line_fmt(format_args!(
        "账号 {} 的{}连续重试 {} 次仍失败，本次签到记为失败并继续后续玩法：{}",
        email,
        operation,
        attempts,
        humanize_retryable_api_error(error)
    ));
}

fn failure_checkin_result(email: &str, error_message: impl Into<String>) -> CheckinResult {
    CheckinResult {
        email: email.to_string(),
        status: "签到失败".to_string(),
        error_message: error_message.into(),
        when_unix_ms: current_unix_ms(),
        ..CheckinResult::default()
    }
}

fn checkin_api_error_message(error: &ApiError) -> String {
    if is_retryable_api_error(error) {
        humanize_retryable_api_error(error)
    } else {
        error.to_string()
    }
}

fn run_checkin(runtime: &AccountRuntime, email: &str) -> Result<CheckinResult, ApiError> {
    let before = runtime.api_client.get_checkin_me(&runtime.auth_token)?;
    let today = runtime.api_client.get_checkin_today(&runtime.auth_token)?;
    if today.claimed {
        return Ok(CheckinResult {
            email: email.to_string(),
            status: "签到失败（今日已签到）".to_string(),
            success: false,
            delta: 0.0,
            balance_after: before.user.balance,
            when_unix_ms: current_unix_ms(),
            ..CheckinResult::default()
        });
    }
    let claim = runtime
        .api_client
        .claim_checkin_today(&runtime.auth_token)?;
    let after = runtime.api_client.get_checkin_me(&runtime.auth_token)?;
    Ok(finalize_checkin_result(email, before, today, claim, after))
}

pub(super) fn finalize_checkin_result(
    email: &str,
    before: CheckinMeResponse,
    _today: CheckinTodayResponse,
    claim: CheckinClaimResponse,
    after: CheckinMeResponse,
) -> CheckinResult {
    let success = claim.ok && !claim.already_claimed;
    let status = if claim.already_claimed {
        "签到失败（今日已签到）"
    } else if success {
        "签到成功"
    } else {
        "签到失败（签到接口未返回成功标记）"
    };
    let mut delta = after.user.balance - before.user.balance;
    if delta == 0.0 && claim.reward_amount > 0.0 {
        delta = claim.reward_amount;
    }
    CheckinResult {
        email: email.to_string(),
        status: status.to_string(),
        success,
        delta,
        balance_after: after.user.balance,
        when_unix_ms: current_unix_ms(),
        error_message: String::new(),
    }
}

pub(super) fn humanize_balance_error(error: &ApiError) -> String {
    error.to_string()
}

pub(super) fn humanize_account_status(status: &str) -> &str {
    let normalized = status.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "" | "unknown" => "未知",
        "active" => "正常",
        "inactive" => "未激活",
        "disabled" => "已停用",
        "banned" => "已封禁",
        _ => status.trim(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{AuthCache, AuthConfig};

    #[test]
    fn finalize_checkin_result_keeps_go_already_claimed_semantics() {
        let result = finalize_checkin_result(
            "demo@example.com",
            CheckinMeResponse {
                authenticated: true,
                user: crate::model::CheckinUser {
                    balance: 10.0,
                    ..Default::default()
                },
            },
            CheckinTodayResponse {
                claimed: false,
                ..Default::default()
            },
            CheckinClaimResponse {
                already_claimed: true,
                ok: true,
                reward_amount: 2.0,
                ..Default::default()
            },
            CheckinMeResponse {
                authenticated: true,
                user: crate::model::CheckinUser {
                    balance: 10.0,
                    ..Default::default()
                },
            },
        );

        assert_eq!(result.status, "签到失败（今日已签到）");
        assert!(!result.success);
        assert_eq!(result.delta, 2.0);
        assert_eq!(result.balance_after, 10.0);
    }

    #[test]
    fn finalize_checkin_result_uses_failure_status_when_claim_not_ok() {
        let result = finalize_checkin_result(
            "demo@example.com",
            CheckinMeResponse {
                authenticated: true,
                user: crate::model::CheckinUser {
                    balance: 10.0,
                    ..Default::default()
                },
            },
            CheckinTodayResponse {
                claimed: false,
                ..Default::default()
            },
            CheckinClaimResponse {
                ok: false,
                reward_amount: 0.0,
                ..Default::default()
            },
            CheckinMeResponse {
                authenticated: true,
                user: crate::model::CheckinUser {
                    balance: 10.0,
                    ..Default::default()
                },
            },
        );

        assert_eq!(result.status, "签到失败（签到接口未返回成功标记）");
        assert!(!result.success);
        assert_eq!(result.delta, 0.0);
    }

    #[test]
    fn run_checkin_with_retry_retries_retryable_error_then_records_failure() {
        let state = Arc::new(Mutex::new(BatchState {
            config: AuthConfig::default(),
            auth_cache_file: Some(std::env::temp_dir().join("checkin-test-auth.json")),
            log: crate::ui::TaskLog::stdout(),
        }));
        let cancel_flag = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let mut runtime = AccountRuntime {
            api_client: crate::api::ApiClient::new("http://127.0.0.1:9"),
            account: AuthCache {
                email: "demo@example.com".to_string(),
                ..Default::default()
            },
            auth_token: "token".to_string(),
        };

        let result =
            run_checkin_with_retry(&cancel_flag, &state, &mut runtime, "demo@example.com").unwrap();

        assert_eq!(result.status, "签到失败");
        assert!(!result.success);
        assert!(result.error_message.contains("网络"));
    }

    #[test]
    fn run_checkin_with_retry_keeps_unauthorized_label_but_surfaces_reason() {
        let result = CheckinResult {
            email: "demo@example.com".to_string(),
            status: "签到失败".to_string(),
            error_message: "获取签到账号信息失败：登录状态已失效，请重新登录".to_string(),
            ..Default::default()
        };

        let line = crate::workflows::checkin::format_checkin_result_line(&result);

        assert!(line.contains("签到失败"));
        assert!(line.contains("原因：获取签到账号信息失败：登录状态已失效，请重新登录"));
    }
}
