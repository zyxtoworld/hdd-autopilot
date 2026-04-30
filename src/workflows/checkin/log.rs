use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::Path;

use crate::model::CheckinResult;
use crate::workflows::common::beijing_time;

pub fn append_checkin_log(log_dir: impl AsRef<Path>, result: &CheckinResult) -> io::Result<()> {
    fs::create_dir_all(log_dir.as_ref())?;
    let path = log_dir.as_ref().join("checkin.log");
    let line = format_checkin_result_line(result);
    let when = beijing_time(result.when_unix_ms);
    let entry = format!("[{}] {}\n", when.format("%Y-%m-%d %H:%M:%S"), line);
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    file.write_all(entry.as_bytes())?;
    file.flush()
}

pub fn format_checkin_result_line(result: &CheckinResult) -> String {
    let status = if result.status.trim().is_empty() {
        "签到失败（未知原因）"
    } else {
        result.status.trim()
    };
    let reason = if result.error_message.trim().is_empty() {
        String::new()
    } else {
        format!("，原因：{}", result.error_message.trim())
    };
    format!(
        "账号 {} 签到结果：{}，本次增加 {:.2}，当前余额 {:.8}{}。",
        result.email, status, result.delta, result.balance_after, reason,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn format_checkin_result_line_includes_error_reason_when_present() {
        let line = format_checkin_result_line(&CheckinResult {
            email: "demo@example.com".to_string(),
            status: "签到失败".to_string(),
            delta: 0.0,
            balance_after: 10.0,
            error_message: "登录状态已失效".to_string(),
            ..Default::default()
        });

        assert_eq!(
            line,
            "账号 demo@example.com 签到结果：签到失败，本次增加 0.00，当前余额 10.00000000，原因：登录状态已失效。"
        );
    }

    #[test]
    fn format_checkin_result_line_falls_back_when_status_is_empty() {
        let line = format_checkin_result_line(&CheckinResult {
            email: "demo@example.com".to_string(),
            status: "   ".to_string(),
            delta: 0.0,
            balance_after: 10.0,
            ..Default::default()
        });

        assert_eq!(
            line,
            "账号 demo@example.com 签到结果：签到失败（未知原因），本次增加 0.00，当前余额 10.00000000。"
        );
    }

    #[test]
    fn append_checkin_log_writes_reason_when_present() {
        let dir = tempdir().unwrap();
        let result = CheckinResult {
            email: "demo@example.com".to_string(),
            status: "签到失败".to_string(),
            delta: 0.0,
            balance_after: 12.34,
            error_message: "签到接口未返回成功标记".to_string(),
            ..Default::default()
        };

        append_checkin_log(dir.path(), &result).unwrap();
        let content = std::fs::read_to_string(dir.path().join("checkin.log")).unwrap();

        assert!(content.contains("账号 demo@example.com 签到结果：签到失败，本次增加 0.00，当前余额 12.34000000，原因：签到接口未返回成功标记。"));
    }
}
