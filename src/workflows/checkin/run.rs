use std::sync::{Arc, Mutex};

use crate::api::ApiError;
use crate::model::{CheckinClaimResponse, CheckinMeResponse, CheckinResult, CheckinTodayResponse};
use crate::runtime::resolve_data_file_path;
use crate::ui;

use super::auth::{ensure_authenticated, reauthenticate};
use super::log::append_checkin_log;
use super::{AccountRuntime, BatchState, current_unix_ms};

pub(super) fn run_one_account(
    cancel_flag: &ui::CancelFlag,
    state: Arc<Mutex<BatchState>>,
    mut runtime: AccountRuntime,
) -> Option<CheckinResult> {
    if ui::check_cancel(cancel_flag).is_err() {
        return None;
    }
    if let Err(error) = ensure_authenticated(&state, &mut runtime) {
        return Some(CheckinResult {
            email: runtime.email().to_string(),
            status: "签到失败".to_string(),
            error_message: error.to_string(),
            when_unix_ms: current_unix_ms(),
            ..CheckinResult::default()
        });
    }

    if ui::check_cancel(cancel_flag).is_err() {
        return None;
    }
    let email = runtime.email().to_string();
    let result = run_checkin_with_retry(&state, &mut runtime, &email);
    let log_dir = resolve_data_file_path("log/checkin");
    let _ = append_checkin_log(&log_dir, &result);
    state
        .lock()
        .unwrap()
        .log
        .line(crate::workflows::checkin::format_checkin_result_line(
            &result,
        ));
    Some(result)
}

pub(super) fn run_checkin_with_retry(
    state: &Arc<Mutex<BatchState>>,
    runtime: &mut AccountRuntime,
    email: &str,
) -> CheckinResult {
    match run_checkin(runtime, email) {
        Ok(result) => result,
        Err(error) if crate::api::is_unauthorized(&error) => {
            state.lock().unwrap().log.line_fmt(format_args!(
                "账号 {} 的登录状态中途失效了，正在重新登录后继续。",
                runtime.email()
            ));
            if let Err(relogin_error) = reauthenticate(state, runtime) {
                return CheckinResult {
                    email: email.to_string(),
                    status: "签到失败".to_string(),
                    error_message: relogin_error.to_string(),
                    when_unix_ms: current_unix_ms(),
                    ..CheckinResult::default()
                };
            }
            match run_checkin(runtime, email) {
                Ok(result) => result,
                Err(error) => CheckinResult {
                    email: email.to_string(),
                    status: "签到失败".to_string(),
                    error_message: error.to_string(),
                    when_unix_ms: current_unix_ms(),
                    ..CheckinResult::default()
                },
            }
        }
        Err(error) => CheckinResult {
            email: email.to_string(),
            status: "签到失败".to_string(),
            error_message: error.to_string(),
            when_unix_ms: current_unix_ms(),
            ..CheckinResult::default()
        },
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
    use crate::api::ApiError;
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
    fn run_checkin_with_retry_surfaces_non_unauthorized_error() {
        let state = Arc::new(Mutex::new(BatchState {
            config: AuthConfig::default(),
            auth_cache_file: Some(std::env::temp_dir().join("checkin-test-auth.json")),
            log: crate::ui::TaskLog::stdout(),
        }));
        let mut runtime = AccountRuntime {
            api_client: crate::api::ApiClient::new("http://127.0.0.1:9"),
            account: AuthCache {
                email: "demo@example.com".to_string(),
                ..Default::default()
            },
            auth_token: "token".to_string(),
        };

        let result = run_checkin_with_retry(&state, &mut runtime, "demo@example.com");

        assert_eq!(result.status, "签到失败");
        assert!(!result.error_message.trim().is_empty());
        assert!(!matches!(
            ApiError::Message(result.error_message.clone()),
            ApiError::Unauthorized(_)
        ));
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
