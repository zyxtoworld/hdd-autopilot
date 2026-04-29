use std::time::Duration;

use crate::{MiningError, Mode};

pub(crate) fn humanize_error(error: &MiningError) -> String {
    error
        .to_string()
        .replace("\\r\\n", "\n")
        .replace("\\n", "\n")
}

pub(crate) fn humanize_duration(duration: Duration) -> String {
    if duration.as_secs() >= 60 {
        format!("{} 分钟", duration.as_secs() / 60)
    } else {
        format!("{} 秒", duration.as_secs())
    }
}

pub(crate) fn mode_description(mode: Mode) -> &'static str {
    match mode {
        Mode::InviteThenBalance => "先尝试邀请码，不够时再切换到余额兑换码",
        Mode::BalanceThenInvite => "先尝试余额兑换码，不够时再切换到邀请码",
        Mode::InviteOnly => "只尝试邀请码",
        Mode::BalanceOnly => "只尝试余额兑换码",
    }
}

pub(crate) fn localized_message(message: &str, fallback: &str) -> String {
    let trimmed = message.trim();
    if trimmed.is_empty() {
        return fallback.to_string();
    }
    let lower = trimmed.to_ascii_lowercase();
    if lower.contains("daily win limit reached") || lower.contains("daily limit reached") {
        return "今日命中次数已达上限".to_string();
    }
    if lower.contains("no open round") {
        return "当前没有开放轮次".to_string();
    }
    if lower.contains("round closed") {
        return "当前轮次已关闭".to_string();
    }
    if lower.contains("pool disabled") {
        return "矿池当前未开放".to_string();
    }
    if lower.contains("challenge rejected") {
        return "挑战被矿池拒绝".to_string();
    }
    if lower.contains("inventory depleted") {
        return "当前邀请码和余额兑换码库存都已耗尽".to_string();
    }
    if trimmed.bytes().any(|byte| byte.is_ascii_alphabetic()) {
        return fallback.to_string();
    }
    trimmed.to_string()
}

pub(crate) fn result_label(result: &str) -> String {
    match result.trim().to_ascii_lowercase().as_str() {
        "daily win limit reached" => "今日命中次数已达上限".to_string(),
        "round_closed" => "轮次已关闭".to_string(),
        "late" => "提交过晚".to_string(),
        "ok" | "accepted" | "success" => "成功".to_string(),
        _ => localized_message(result, "未说明"),
    }
}

pub(crate) fn preference_label(value: &str) -> String {
    match value.trim().to_ascii_lowercase().as_str() {
        "invite" => "邀请码".to_string(),
        "balance" => "余额兑换码".to_string(),
        _ => localized_message(value, "未说明"),
    }
}

pub(crate) fn code_type_label(value: &str) -> String {
    let lower = value.trim().to_ascii_lowercase();
    if lower.is_empty() || lower == "none" {
        return "无".to_string();
    }
    if lower.contains("parallel") || lower.contains("concurrent") {
        return "并发码".to_string();
    }
    if lower.contains("invite") {
        return "邀请码".to_string();
    }
    if lower.contains("balance") {
        return "余额兑换码".to_string();
    }
    localized_message(value, "未说明")
}

pub(crate) fn reward_benefit_label(
    code_type: &str,
    balance_amount: f64,
    concurrency: i32,
) -> String {
    let normalized = code_type_label(code_type);
    if normalized == "并发码" {
        if concurrency > 0 {
            return format!("可增加 {} 并发", concurrency);
        }
        return String::new();
    }
    if normalized == "余额兑换码" && balance_amount > 0.0 {
        let mut amount = format!("{:.2}", balance_amount);
        while amount.ends_with('0') {
            amount.pop();
        }
        if amount.ends_with('.') {
            amount.pop();
        }
        return format!("可增加 {} 余额", amount);
    }
    String::new()
}

pub(crate) fn reward_display_label(
    code_type: &str,
    balance_amount: f64,
    concurrency: i32,
) -> String {
    let code_type = code_type_label(code_type);
    let benefit = reward_benefit_label(code_type.as_str(), balance_amount, concurrency);
    if benefit.is_empty() {
        code_type
    } else {
        format!("{}（{}）", code_type, benefit)
    }
}

#[cfg(test)]
mod tests {
    use super::{reward_benefit_label, reward_display_label};

    #[test]
    fn reward_display_label_formats_balance_and_concurrency_benefits() {
        assert_eq!(
            reward_display_label("balance", 6.5, 0),
            "余额兑换码（可增加 6.5 余额）"
        );
        assert_eq!(
            reward_display_label("concurrent", 0.0, 4),
            "并发码（可增加 4 并发）"
        );
        assert_eq!(reward_benefit_label("invite", 0.0, 0), "");
    }
}
