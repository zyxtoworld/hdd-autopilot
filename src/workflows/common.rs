use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::api::{ApiClient, ApiError, is_unauthorized};
use crate::model::{AuthCache, AuthConfig, AuthMeResponse, AuthSession};
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
#[cfg(not(test))]
pub(crate) const TASK_REENTRY_MAX_RETRIES: usize = 3;
#[cfg(test)]
pub(crate) const TASK_REENTRY_MAX_RETRIES: usize = 2;

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

pub(crate) fn sleep_min_interval(
    cancel_flag: &ui::CancelFlag,
    min_interval_ms: i32,
) -> io::Result<()> {
    ui::check_cancel(cancel_flag)?;
    if min_interval_ms <= 0 {
        return Ok(());
    }
    ui::sleep_with_cancel(cancel_flag, Duration::from_millis(min_interval_ms as u64))
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
                if task_reentry_limit_reached(retry_count) {
                    log.line_fmt(format_args!(
                        "账号 {} 的{}连续重进玩法 {} 次仍失败，已停止该任务以避免阻塞线程：{}",
                        email, task_name, retry_count, error
                    ));
                    return Err(task_reentry_exhausted_error(task_name, retry_count, &error));
                }
                retry_count += 1;
                log.line_fmt(format_args!(
                    "账号 {} 的{}这次没有跑完：{}。会重新进入续残局/剩余次数（第 {}/{} 次重试）。",
                    email, task_name, error, retry_count, TASK_REENTRY_MAX_RETRIES
                ));
                ui::sleep_with_cancel(cancel_flag, ACCOUNT_TASK_RETRY_BACKOFF)?;
            }
        }
    }
}

pub(crate) fn task_reentry_limit_reached(retry_count: usize) -> bool {
    retry_count >= TASK_REENTRY_MAX_RETRIES
}

pub(crate) fn task_reentry_exhausted_error(
    task_name: &str,
    retry_count: usize,
    error: &io::Error,
) -> io::Error {
    io::Error::other(format!(
        "{}连续重进玩法 {} 次仍失败，已停止该任务以避免阻塞线程：{}",
        task_name, retry_count, error
    ))
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

pub(crate) fn format_log_time(when_unix_ms: i64) -> String {
    let when = system_local_datetime(when_unix_ms);
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
        when.year(),
        u8::from(when.month()),
        when.day(),
        when.hour(),
        when.minute(),
        when.second()
    )
}

pub(crate) fn append_account_log_line(
    log_dir: &Path,
    email: &str,
    content: &str,
) -> io::Result<()> {
    let path = dated_account_log_file_path(log_dir, email, current_unix_ms());
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    file.write_all(content.as_bytes())?;
    file.flush()
}

pub(crate) fn dated_project_log_dir(log_dir: &Path, when_unix_ms: i64) -> PathBuf {
    let date = format_log_date(when_unix_ms);
    let project = log_dir
        .file_name()
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| std::ffi::OsStr::new("unknown"));
    log_dir
        .parent()
        .map(|parent| parent.join(date.to_string()).join(project))
        .unwrap_or_else(|| log_dir.join(date.to_string()))
}

fn format_log_date(when_unix_ms: i64) -> String {
    let when = system_local_datetime(when_unix_ms);
    format!(
        "{:04}{:02}{:02}",
        when.year(),
        u8::from(when.month()),
        when.day()
    )
}

fn system_local_datetime(when_unix_ms: i64) -> time::OffsetDateTime {
    let when_unix_ms = if when_unix_ms > 0 {
        when_unix_ms
    } else {
        current_unix_ms()
    };
    let utc = time::OffsetDateTime::from_unix_timestamp_nanos(
        i128::from(when_unix_ms).saturating_mul(1_000_000),
    )
    .unwrap_or_else(|_| time::OffsetDateTime::from_unix_timestamp(0).unwrap());
    let offset = time::UtcOffset::local_offset_at(utc).unwrap_or(time::UtcOffset::UTC);
    utc.to_offset(offset)
}

pub(crate) fn dated_account_log_file_path(
    log_dir: &Path,
    email: &str,
    when_unix_ms: i64,
) -> PathBuf {
    dated_project_log_dir(log_dir, when_unix_ms).join(account_log_file_name(email))
}

fn account_log_file_name(email: &str) -> String {
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
    sanitized.replace('@', "_at_") + ".log"
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
    let log = state.lock().unwrap().log.clone();
    ensure_authenticated_session(
        &log,
        &mut runtime.api_client,
        &mut runtime.account,
        &mut runtime.auth_token,
        |account| state.lock().unwrap().save_account(account),
    )
}

pub(crate) fn ensure_authenticated_session<F>(
    log: &ui::TaskLog,
    api_client: &mut ApiClient,
    account: &mut AuthCache,
    auth_token: &mut String,
    mut save_account: F,
) -> io::Result<()>
where
    F: FnMut(AuthCache) -> io::Result<()>,
{
    let base_url = api_client.base_url().to_string();
    if let Some(session) = get_session(account, &base_url) {
        let cached_auth_token = build_authorization(&session.token_type, &session.access_token);
        if !cached_auth_token.is_empty() {
            if let Ok(auth_me) = api_client.validate_auth_token(&cached_auth_token) {
                persist_authenticated_session(
                    account,
                    auth_token,
                    &base_url,
                    session.token_type.clone(),
                    session.access_token.clone(),
                    cached_auth_token,
                    auth_me,
                    &mut save_account,
                )?;
                return Ok(());
            }
            log.line_fmt(format_args!(
                "账号 {} 的上次登录信息已经失效，准备重新登录。",
                account.email.trim()
            ));
        }
    }

    login_with_password_session(
        log,
        api_client,
        account,
        auth_token,
        &base_url,
        save_account,
    )
}

fn login_with_password_session<F>(
    _log: &ui::TaskLog,
    api_client: &mut ApiClient,
    account: &mut AuthCache,
    auth_token: &mut String,
    base_url: &str,
    mut save_account: F,
) -> io::Result<()>
where
    F: FnMut(AuthCache) -> io::Result<()>,
{
    if !password_usable(account) {
        return Err(io::Error::other(format!(
            "账号 {} 没有保存密码，无法自动重新登录",
            account.email.trim()
        )));
    }

    let (login_response, new_auth_token) = api_client
        .do_login(&account.email, &account.password)
        .map_err(api_error_to_io_error)?;
    *account = cache_from_login(&login_response, &account.email, &account.password, base_url);
    *auth_token = new_auth_token;
    save_account(account.clone())
}

fn persist_authenticated_session<F>(
    account: &mut AuthCache,
    auth_token: &mut String,
    base_url: &str,
    token_type: String,
    access_token: String,
    resolved_auth_token: String,
    auth_me: AuthMeResponse,
    save_account: &mut F,
) -> io::Result<()>
where
    F: FnMut(AuthCache) -> io::Result<()>,
{
    let email = auth_me.data.email.trim();
    if !email.is_empty()
        && !account.email.trim().is_empty()
        && !email.eq_ignore_ascii_case(account.email.trim())
    {
        return Err(io::Error::other(format!(
            "账号 {} 读取到的登录状态属于另一个账号 {}，请重新登录或检查 auth.json",
            account.email.trim(),
            email
        )));
    }
    if !email.is_empty() {
        account.email = email.to_string();
    }
    *account = upsert_session(
        account.clone(),
        AuthSession {
            base_url: base_url.to_string(),
            token_type,
            access_token,
        },
    );
    *auth_token = resolved_auth_token;
    save_account(account.clone())
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

pub(crate) fn with_auth_retry_api_mutation_until_success<T, F, Recover, IsConflict>(
    cancel_flag: &ui::CancelFlag,
    state: &Arc<Mutex<BatchState>>,
    runtime: &mut AccountRuntime,
    operation: &str,
    action: F,
    mut recover: Recover,
    is_conflict: IsConflict,
) -> io::Result<T>
where
    F: Fn(&ApiClient, &str) -> Result<T, ApiError>,
    Recover: FnMut(
        &ui::CancelFlag,
        &Arc<Mutex<BatchState>>,
        &mut AccountRuntime,
    ) -> io::Result<Option<T>>,
    IsConflict: Fn(&ApiError) -> bool,
{
    let mut attempts = 0usize;
    loop {
        ui::check_cancel(cancel_flag)?;
        attempts += 1;
        match with_auth_retry_api(state, runtime, &action) {
            Ok(value) => return Ok(value),
            Err(error) if is_conflict(&error) => {
                if let Some(value) = recover(cancel_flag, state, runtime)? {
                    return Ok(value);
                }
                return Err(api_error_to_io_error(error));
            }
            Err(error) if is_retryable_api_error(&error) => {
                log_retryable_api_error(state, runtime.email(), operation, attempts, &error);
                if let Some(value) = recover(cancel_flag, state, runtime)? {
                    return Ok(value);
                }
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

pub(crate) fn is_state_conflict_api_error(error: &ApiError) -> bool {
    if let ApiError::HttpStatus { status, .. } = error
        && *status == 409
    {
        return true;
    }
    is_state_conflict_message(&error.to_string())
}

pub(crate) fn is_state_conflict_io_error(error: &io::Error) -> bool {
    is_state_conflict_message(&error.to_string())
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
        "sokoban config" => "推箱子配置接口",
        "sokoban me" => "推箱子次数查询接口",
        "sokoban history" => "推箱子历史接口",
        "sokoban start" => "推箱子开局接口",
        "sokoban move" => "推箱子移动接口",
        "lightsout config" => "点灯配置接口",
        "lightsout me" => "点灯次数查询接口",
        "lightsout history" => "点灯历史接口",
        "lightsout start" => "点灯开局接口",
        "lightsout click" => "点灯点击接口",
        "maze config" => "迷宫配置接口",
        "maze me" => "迷宫次数查询接口",
        "maze history" => "迷宫历史接口",
        "maze start" => "迷宫开局接口",
        "maze move" => "迷宫移动接口",
        "nonogram config" => "数织配置接口",
        "nonogram me" => "数织次数查询接口",
        "nonogram history" => "数织历史接口",
        "nonogram start" => "数织开局接口",
        "nonogram finish" => "数织提交接口",
        "flowfree config" => "连线配置接口",
        "flowfree me" => "连线次数查询接口",
        "flowfree history" => "连线历史接口",
        "flowfree start" => "连线开局接口",
        "flowfree finish" => "连线提交接口",
        "flowfree abandon" => "连线放弃接口",
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

fn is_state_conflict_message(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    message.contains("状态码 409")
        || message.contains("请求状态冲突")
        || message.contains("未结束对局")
        || message.contains("状态还没同步")
        || lower.contains("active session")
        || lower.contains("already finished")
        || lower.contains("already ended")
        || lower.contains("max active")
        || lower.contains("conflict")
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
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

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
    fn localized_retry_operation_covers_logic_games() {
        assert_eq!(localized_retry_operation("sokoban move"), "推箱子移动接口");
        assert_eq!(localized_retry_operation("lightsout click"), "点灯点击接口");
        assert_eq!(localized_retry_operation("maze move"), "迷宫移动接口");
        assert_eq!(localized_retry_operation("nonogram finish"), "数织提交接口");
        assert_eq!(localized_retry_operation("flowfree finish"), "连线提交接口");
    }

    #[test]
    fn account_task_stops_after_reentry_limit() {
        let calls = AtomicUsize::new(0);
        let cancel_flag = Arc::new(AtomicBool::new(false));
        let error = run_account_task_until_complete(
            &cancel_flag,
            &crate::ui::TaskLog::stdout(),
            "自动测试",
            "demo@example.com",
            || {
                calls.fetch_add(1, Ordering::SeqCst);
                Err::<(), _>(io::Error::other("无法恢复"))
            },
        )
        .unwrap_err();

        assert_eq!(calls.load(Ordering::SeqCst), TASK_REENTRY_MAX_RETRIES + 1);
        assert!(error.to_string().contains("避免阻塞线程"));
    }

    #[test]
    fn dated_account_log_file_path_uses_date_project_and_account() {
        let when = current_unix_ms();
        let date = format_log_date(when);

        assert_eq!(
            dated_account_log_file_path(Path::new("log/minesweeper"), "demo@example.com", when),
            Path::new("log")
                .join(date)
                .join("minesweeper")
                .join("demo_at_example.com.log")
        );
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
