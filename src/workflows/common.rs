use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use chrono::{FixedOffset, TimeZone, Utc};

use crate::api::{ApiClient, ApiError, is_unauthorized};
use crate::model::{AuthCache, AuthConfig, AuthSession};
use crate::storage::{
    build_authorization, cache_from_login, get_session, password_usable, save_cache,
    upsert_account, upsert_session,
};
use crate::ui;

const API_RETRY_BACKOFF: Duration = Duration::from_millis(500);
const API_RETRY_LOG_EVERY: usize = 10;
#[cfg(not(test))]
pub(crate) const API_RETRY_MAX_ATTEMPTS: usize = 60;
#[cfg(test)]
pub(crate) const API_RETRY_MAX_ATTEMPTS: usize = 2;
const ACCOUNT_TASK_RETRY_BACKOFF: Duration = Duration::ZERO;

#[derive(Debug)]
pub(crate) struct BatchState {
    pub(crate) config: AuthConfig,
    pub(crate) auth_cache_file: Option<PathBuf>,
    pub(crate) result_log_dir: PathBuf,
    pub(crate) log: ui::TaskLog,
}

impl BatchState {
    pub(crate) fn save_account(&mut self, account: AuthCache) -> io::Result<()> {
        self.config = upsert_account(self.config.clone(), account);
        if let Some(path) = &self.auth_cache_file {
            save_cache(path, self.config.clone())
        } else {
            Ok(())
        }
    }
}

#[derive(Debug)]
pub(crate) struct AccountRuntime {
    api_client: ApiClient,
    account: AuthCache,
    auth_token: String,
}

impl AccountRuntime {
    pub(crate) fn new(base_url: &str, account: AuthCache) -> Self {
        Self {
            api_client: ApiClient::new(base_url),
            account,
            auth_token: String::new(),
        }
    }

    pub(crate) fn email(&self) -> &str {
        self.account.email.trim()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct AccountRewardSummary {
    pub(crate) index: usize,
    pub(crate) email: String,
    pub(crate) total_reward: f64,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct ServerClockSnapshot {
    server_now_ms: i64,
    observed_at: Instant,
}

impl ServerClockSnapshot {
    pub(crate) fn new(server_now_ms: i64) -> Self {
        Self {
            server_now_ms: if server_now_ms > 0 {
                server_now_ms
            } else {
                current_unix_ms()
            },
            observed_at: Instant::now(),
        }
    }

    pub(crate) fn elapsed_since_ms(self, started_at_ms: i64) -> i64 {
        self.estimated_now_ms().saturating_sub(started_at_ms.max(0))
    }

    fn estimated_now_ms(self) -> i64 {
        let local_elapsed = self.observed_at.elapsed().as_millis().min(i64::MAX as u128) as i64;
        self.server_now_ms.saturating_add(local_elapsed)
    }
}

pub(crate) fn print_account_reward_summary(
    log: &ui::TaskLog,
    title: &str,
    summaries: &[AccountRewardSummary],
) {
    if summaries.is_empty() {
        return;
    }
    let mut summaries = summaries.to_vec();
    summaries.sort_by_key(|summary| summary.index);
    log.line_fmt(format_args!("【{}】所有账号收益汇总：", title));
    let mut all_accounts_total = 0.0;
    for summary in &summaries {
        all_accounts_total += summary.total_reward;
        log.line_fmt(format_args!(
            "  {}. {}：总收益 {}",
            summary.index + 1,
            summary.email,
            format_amount(summary.total_reward)
        ));
    }
    log.line_fmt(format_args!(
        "【{}】所有账号总收益：{}",
        title,
        format_amount(all_accounts_total)
    ));
}

pub(crate) fn format_amount(value: f64) -> String {
    if !value.is_finite() {
        return value.to_string();
    }
    let normalized = if value.abs() < 0.000000005 {
        0.0
    } else {
        value
    };
    let mut text = format!("{:.8}", normalized);
    while text.contains('.') && text.ends_with('0') {
        text.pop();
    }
    if text.ends_with('.') {
        text.pop();
    }
    if text == "-0" { "0".to_string() } else { text }
}

pub(crate) fn format_duration_ms(duration_ms: i64) -> String {
    let seconds = duration_ms.max(0) as f64 / 1000.0;
    format!("{seconds:.3}秒")
}

pub(crate) fn run_account_task_until_complete<T, F>(
    cancel_flag: &ui::CancelFlag,
    log: &ui::TaskLog,
    task_name: &str,
    email: &str,
    mut action: F,
) -> io::Result<T>
where
    F: FnMut() -> io::Result<T>,
{
    let mut retry_count = 0usize;
    loop {
        ui::check_cancel(cancel_flag)?;
        match action() {
            Ok(value) => return Ok(value),
            Err(error) if error.kind() == io::ErrorKind::Interrupted => return Err(error),
            Err(error) => {
                retry_count += 1;
                log.line_fmt(format_args!(
                    "账号 {} 的{}这次没有跑完：{}。不会跳过，等一下重新进入续残局/剩余次数（第 {} 次重试）。",
                    email, task_name, error, retry_count
                ));
                ui::sleep_with_cancel(cancel_flag, ACCOUNT_TASK_RETRY_BACKOFF)?;
            }
        }
    }
}

pub(crate) fn retry_operation_with_step(operation: &str, step: i32) -> String {
    format!("{}#step={}", operation, step.max(1))
}

pub(crate) fn current_unix_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(i64::MAX as u128) as i64)
        .unwrap_or(0)
}

pub(crate) fn beijing_time(when_unix_ms: i64) -> chrono::DateTime<FixedOffset> {
    Utc.timestamp_millis_opt(when_unix_ms)
        .single()
        .unwrap_or_else(|| {
            Utc.timestamp_millis_opt(current_unix_ms())
                .single()
                .unwrap()
        })
        .with_timezone(&FixedOffset::east_opt(8 * 60 * 60).unwrap())
}

#[cfg(test)]
pub(crate) fn same_beijing_day(left_unix_ms: i64, right_unix_ms: i64) -> bool {
    if left_unix_ms <= 0 || right_unix_ms <= 0 {
        return false;
    }
    const BEIJING_OFFSET_MS: i64 = 8 * 60 * 60 * 1000;
    const DAY_MS: i64 = 24 * 60 * 60 * 1000;
    left_unix_ms
        .saturating_add(BEIJING_OFFSET_MS)
        .div_euclid(DAY_MS)
        == right_unix_ms
            .saturating_add(BEIJING_OFFSET_MS)
            .div_euclid(DAY_MS)
}

pub(crate) fn append_account_log_line(
    log_dir: &Path,
    email: &str,
    content: &str,
) -> io::Result<()> {
    fs::create_dir_all(log_dir)?;
    let path = account_log_file_path(log_dir, email);
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    file.write_all(content.as_bytes())?;
    file.flush()
}

pub(crate) fn account_log_file_path(log_dir: &Path, email: &str) -> PathBuf {
    let mut sanitized = String::new();
    for ch in email.trim().to_ascii_lowercase().chars() {
        match ch {
            'a'..='z' | '0'..='9' | '.' | '_' | '-' | '@' => sanitized.push(ch),
            _ => sanitized.push('_'),
        }
    }
    if sanitized.is_empty() {
        sanitized = "unknown".to_string();
    }
    log_dir.join(sanitized.replace('@', "_at_") + ".log")
}

pub(crate) fn join_log_clauses(parts: &[String]) -> String {
    let parts = parts
        .iter()
        .map(|part| part.trim())
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    if parts.is_empty() {
        return String::new();
    }
    parts.join("，") + "。\n"
}

pub(crate) fn round_mode_label(continued: bool) -> &'static str {
    if continued { "续玩" } else { "新开局" }
}

pub(crate) fn round_progress_label(current: i32, total: i32) -> String {
    let current = current.max(1);
    if total <= 0 {
        format!("今天第 {} 局", current)
    } else {
        format!("今天第 {}/{} 局", current, total.max(current))
    }
}

pub(crate) fn reason_clause(error_message: &str) -> String {
    let error_message = error_message.trim();
    if error_message.is_empty() {
        return String::new();
    }
    format!("原因：{}", error_message)
}

pub(crate) fn is_pending_round_status(status: &str) -> bool {
    matches!(
        status.trim().to_ascii_lowercase().as_str(),
        "" | "pending" | "running" | "active"
    )
}

pub(crate) fn ensure_authenticated(
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

pub(crate) fn with_auth_retry_api<T, F>(
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

pub(crate) fn with_auth_retry_api_until_success<T, F>(
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
                log_retryable_api_error(state, runtime.email(), operation, attempts, &error);
                if attempts >= API_RETRY_MAX_ATTEMPTS {
                    log_retry_exhausted_api_error(
                        state,
                        runtime.email(),
                        operation,
                        attempts,
                        &error,
                    );
                    return Err(retry_exhausted_api_error(operation, attempts, &error));
                }
                ui::sleep_with_cancel(cancel_flag, API_RETRY_BACKOFF)?;
            }
            Err(error) => return Err(api_error_to_io_error(error)),
        }
    }
}

pub(crate) fn api_error_to_io_error(error: ApiError) -> io::Error {
    io::Error::other(error.to_string())
}

pub(crate) fn retry_exhausted_api_error(
    operation: &str,
    attempts: usize,
    error: &ApiError,
) -> io::Error {
    let (operation, step) = parse_retry_operation(operation);
    let step_clause = step
        .map(|step| format!("，卡在第 {} 步", step))
        .unwrap_or_default();
    io::Error::new(
        io::ErrorKind::TimedOut,
        format!(
            "{}{}连续重试 {} 次仍失败，准备重新进入玩法续残局：{}",
            localized_retry_operation(operation),
            step_clause,
            attempts,
            humanize_retryable_api_error(error)
        ),
    )
}

pub(crate) fn is_retryable_api_error(error: &ApiError) -> bool {
    match error {
        ApiError::Message(message) => {
            !is_non_retryable_api_message(message) && is_retryable_api_message(message)
        }
        ApiError::HttpStatus { status, message } => {
            if is_non_retryable_api_message(message) {
                return false;
            }
            if *status == 429 {
                return true;
            }
            matches!(*status, 408 | 425 | 500..=599) || is_retryable_api_message(message)
        }
        ApiError::Unauthorized(_) => false,
    }
}

pub(crate) fn log_retryable_api_error(
    state: &Arc<Mutex<BatchState>>,
    email: &str,
    operation: &str,
    attempts: usize,
    error: &ApiError,
) {
    if attempts == 1 || attempts.is_multiple_of(API_RETRY_LOG_EVERY) {
        let (operation, step) = parse_retry_operation(operation);
        let step_clause = step
            .map(|step| format!("，卡在第 {} 步", step))
            .unwrap_or_default();
        state.lock().unwrap().log.line_fmt(format_args!(
            "账号 {} 的{}暂时连不上{}，会继续等接口恢复后再试（第 {} 次尝试）：{}",
            email,
            localized_retry_operation(operation),
            step_clause,
            attempts,
            humanize_retryable_api_error(error)
        ));
    }
}

pub(crate) fn log_retry_exhausted_api_error(
    state: &Arc<Mutex<BatchState>>,
    email: &str,
    operation: &str,
    attempts: usize,
    error: &ApiError,
) {
    let (operation, step) = parse_retry_operation(operation);
    let step_clause = step
        .map(|step| format!("，卡在第 {} 步", step))
        .unwrap_or_default();
    state.lock().unwrap().log.line_fmt(format_args!(
        "账号 {} 的{}{}连续重试 {} 次仍失败，准备重新进入玩法续残局：{}",
        email,
        localized_retry_operation(operation),
        step_clause,
        attempts,
        humanize_retryable_api_error(error)
    ));
}

pub(crate) fn humanize_retryable_api_error(error: &ApiError) -> String {
    if let ApiError::HttpStatus { status, message } = error {
        let reason = match *status {
            429 => Some("请求太频繁，服务端要求稍后再试"),
            500..=599 => Some("服务端暂时异常"),
            408 => Some("请求超时"),
            425 => Some("服务端要求稍后再试"),
            _ => None,
        };
        if let Some(reason) = reason {
            if let Some(path) = extract_url_path(message) {
                return format!("{}（接口：{}）", reason, path);
            }
            return reason.to_string();
        }
    }

    let message = error.to_string();
    let lower = message.to_ascii_lowercase();
    let reason = if lower.contains("error sending request") {
        "网络请求发送失败"
    } else if lower.contains("operation timed out")
        || lower.contains("deadline has elapsed")
        || lower.contains("timed out")
        || lower.contains("timeout")
    {
        "请求超时"
    } else if lower.contains("error decoding response body")
        || lower.contains("request or response body error")
    {
        "接口返回内容暂时异常"
    } else if lower.contains("connection") || lower.contains("connect") {
        "网络连接暂时异常"
    } else if lower.contains("dns") {
        "域名解析暂时异常"
    } else {
        return message;
    };

    if let Some(path) = extract_url_path(&message) {
        format!("{}（接口：{}）", reason, path)
    } else {
        reason.to_string()
    }
}

fn parse_retry_operation(operation: &str) -> (&str, Option<i32>) {
    let Some((operation, step)) = operation.split_once("#step=") else {
        return (operation, None);
    };
    (operation, step.trim().parse::<i32>().ok())
}

fn localized_retry_operation(operation: &str) -> &'static str {
    match operation {
        "puzzle2048 config" => "谜题2048配置接口",
        "puzzle2048 me" => "谜题2048次数查询接口",
        "puzzle2048 history" => "谜题2048历史接口",
        "puzzle2048 start" => "谜题2048开局接口",
        "puzzle2048 move" => "谜题2048移动接口",
        "puzzle2048 abandon" => "谜题2048结算接口",
        "memory config" => "记忆翻牌配置接口",
        "memory me" => "记忆翻牌次数查询接口",
        "memory history" => "记忆翻牌历史接口",
        "memory start" => "记忆翻牌开局接口",
        "memory flip" => "记忆翻牌翻牌接口",
        "minesweeper config" => "扫雷配置接口",
        "minesweeper me" => "扫雷次数查询接口",
        "minesweeper history" => "扫雷历史接口",
        "minesweeper start" => "扫雷开局接口",
        "minesweeper click" => "扫雷点击接口",
        "puzzle15 config" => "华容道配置接口",
        "puzzle15 me" => "华容道次数查询接口",
        "puzzle15 history" => "华容道历史接口",
        "puzzle15 start" => "华容道开局接口",
        "puzzle15 move" => "华容道移动接口",
        "sudoku config" => "数独配置接口",
        "sudoku me" => "数独次数查询接口",
        "sudoku history" => "数独历史接口",
        "sudoku start" => "数独开局接口",
        "sudoku fill" => "数独填数接口",
        "arrow-out config" => "箭头逃离配置接口",
        "arrow-out me" => "箭头逃离账号状态接口",
        "arrow-out history" => "箭头逃离历史接口",
        "arrow-out start" => "箭头逃离开局接口",
        "arrow-out finish" => "箭头逃离结算接口",
        "arrow-out abandon" => "箭头逃离放弃接口",
        _ => "接口",
    }
}

fn extract_url_path(message: &str) -> Option<String> {
    let (start, marker_len) = message
        .find("https://")
        .map(|index| (index, "https://".len()))
        .or_else(|| {
            message
                .find("http://")
                .map(|index| (index, "http://".len()))
        })?;
    let after_scheme = &message[start + marker_len..];
    let path_start = after_scheme.find('/')?;
    let path = &after_scheme[path_start..];
    let path = path.trim_end_matches([')', '.', ' ', '。']);
    if path.is_empty() {
        None
    } else {
        Some(path.to_string())
    }
}

fn is_retryable_api_message(message: &str) -> bool {
    let normalized = message.to_ascii_lowercase();
    [
        "error request",
        "error sending request",
        "request or response body error",
        "error decoding response body",
        "operation timed out",
        "deadline has elapsed",
        "timed out",
        "timeout",
        "connection",
        "connect",
        "connection reset",
        "connection closed",
        "temporarily",
        "temporary",
        "dns",
        "tls",
        "tcp",
        "hyper",
        "unexpected eof",
        "broken pipe",
    ]
    .iter()
    .any(|needle| normalized.contains(needle))
}

fn is_non_retryable_api_message(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    let direct = [
        "daily limit reached",
        "daily plays",
        "no remaining plays",
        "remaining plays exhausted",
        "active session",
        "max active",
    ]
    .iter()
    .any(|needle| lower.contains(needle));
    direct
        || message.contains("今天这个难度的次数已经用完")
        || message.contains("次数已经用完")
        || message.contains("次数已用完")
        || message.contains("今日次数")
        || message.contains("未结束对局")
        || message.contains("未结束的对局")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn daily_limit_429_is_not_retryable() {
        let error = ApiError::HttpStatus {
            status: 429,
            message: "请求失败了（状态码 429）：今天这个难度的次数已经用完了".to_string(),
        };

        assert!(!is_retryable_api_error(&error));
    }

    #[test]
    fn real_rate_limit_429_remains_retryable() {
        let error = ApiError::HttpStatus {
            status: 429,
            message: "too many requests, retry after a moment".to_string(),
        };

        assert!(is_retryable_api_error(&error));
    }

    #[test]
    fn localized_rate_limit_429_remains_retryable() {
        let error = ApiError::HttpStatus {
            status: 429,
            message: "请求失败了（状态码 429）：请求太频繁，服务端要求稍后再试".to_string(),
        };

        assert!(is_retryable_api_error(&error));
    }

    #[test]
    fn generic_429_remains_retryable_after_business_errors_are_filtered() {
        let error = ApiError::HttpStatus {
            status: 429,
            message: "请求失败了（状态码 429）：服务端返回了错误".to_string(),
        };

        assert!(is_retryable_api_error(&error));
    }

    #[test]
    fn retry_operation_can_carry_step_for_http_errors() {
        let operation = retry_operation_with_step("puzzle2048 move", 12);
        let (operation, step) = parse_retry_operation(&operation);

        assert_eq!(operation, "puzzle2048 move");
        assert_eq!(step, Some(12));
    }

    #[test]
    fn retry_exhaustion_returns_timed_out_error_with_step() {
        let error = ApiError::HttpStatus {
            status: 503,
            message: "请求失败了（状态码 503）：服务端暂时异常".to_string(),
        };
        let exhausted = retry_exhausted_api_error(
            &retry_operation_with_step("puzzle2048 move", 12),
            API_RETRY_MAX_ATTEMPTS,
            &error,
        );

        assert_eq!(exhausted.kind(), io::ErrorKind::TimedOut);
        assert!(exhausted.to_string().contains("卡在第 12 步"));
    }

    #[test]
    fn humanized_http_retry_errors_are_plain_language() {
        let rate_limited = ApiError::HttpStatus {
            status: 429,
            message: "请求失败了（状态码 429）：服务端返回了错误".to_string(),
        };
        let server_error = ApiError::HttpStatus {
            status: 503,
            message: "请求失败了（状态码 503）：服务端暂时异常".to_string(),
        };

        assert_eq!(
            humanize_retryable_api_error(&rate_limited),
            "请求太频繁，服务端要求稍后再试"
        );
        assert_eq!(
            humanize_retryable_api_error(&server_error),
            "服务端暂时异常"
        );
    }

    #[test]
    fn same_beijing_day_counts_after_midnight_before_utc_rollover() {
        let april_30_0100_beijing = 1_777_482_000_000;
        let april_30_1700_beijing = 1_777_539_600_000;

        assert!(same_beijing_day(
            april_30_0100_beijing,
            april_30_1700_beijing
        ));
    }

    #[test]
    fn pending_round_statuses_are_process_states() {
        assert!(is_pending_round_status(""));
        assert!(is_pending_round_status("pending"));
        assert!(is_pending_round_status("RUNNING"));
        assert!(is_pending_round_status("active"));
        assert!(!is_pending_round_status("won"));
        assert!(!is_pending_round_status("lost"));
    }

    #[test]
    fn retry_error_message_is_human_readable() {
        let error = ApiError::Message(
            "error sending request for url (https://sub.hdd.sb/puzzle15-api/move)".to_string(),
        );

        assert_eq!(
            humanize_retryable_api_error(&error),
            "网络请求发送失败（接口：/puzzle15-api/move）"
        );
    }

    #[test]
    fn amount_display_trims_trailing_zeroes() {
        assert_eq!(format_amount(12.34000000), "12.34");
        assert_eq!(format_amount(10.0), "10");
        assert_eq!(format_amount(0.0), "0");
    }

    #[test]
    fn duration_display_uses_seconds_with_three_decimals() {
        assert_eq!(format_duration_ms(1234), "1.234秒");
        assert_eq!(format_duration_ms(0), "0.000秒");
    }

    #[test]
    fn server_clock_snapshot_estimates_elapsed_from_server_time() {
        let clock = ServerClockSnapshot::new(10_900);
        let elapsed = clock.elapsed_since_ms(10_000);

        assert!((900..5_000).contains(&elapsed));
    }
}
