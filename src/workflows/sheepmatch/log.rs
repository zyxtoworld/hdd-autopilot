use std::io;
use std::path::Path;

use crate::model::{AccountRunSummary, RoundResultSummary};
use crate::ui;
use crate::workflows::common::{
    append_account_log_line as append_line, beijing_time, join_log_clauses as join_clauses,
    reason_clause as format_reason_clause, round_mode_label,
};

pub(super) fn append_run_header(log_dir: &Path, email: &str, when_unix_ms: i64) -> io::Result<()> {
    let when = beijing_time(when_unix_ms);
    append_line(
        log_dir,
        email,
        &format!(
            "[{}] 开始运行，正在处理账号 {}。\n",
            when.format("%Y-%m-%d %H:%M:%S"),
            email
        ),
    )
}

pub(super) fn append_round_result(log_dir: &Path, result: &RoundResultSummary) -> io::Result<()> {
    append_line(log_dir, &result.email, &format_round_result_line(result))
}

pub(super) fn append_difficulty_summary(
    log_dir: &Path,
    summary: &AccountRunSummary,
) -> io::Result<()> {
    append_line(
        log_dir,
        &summary.email,
        &format_difficulty_summary_line(summary),
    )
}

pub(super) fn append_account_summary(
    log_dir: &Path,
    email: &str,
    when_unix_ms: i64,
    summaries: &[AccountRunSummary],
) -> io::Result<()> {
    let when = beijing_time(when_unix_ms);
    let total_played: i32 = summaries.iter().map(|item| item.played).sum();
    let total_won: i32 = summaries.iter().map(|item| item.won).sum();
    let total_abandoned: i32 = summaries.iter().map(|item| item.abandoned).sum();
    let total_failed: i32 = summaries.iter().map(|item| item.failed).sum();
    let total_reward: f64 = summaries.iter().map(|item| item.total_reward).sum();
    let balance_after = summaries.iter().rev().find_map(|item| item.balance_after);
    append_line(
        log_dir,
        email,
        &join_clauses(&[
            format!(
                "[{}] 账号 {} 的全部难度汇总：一共玩了 {} 局",
                when.format("%Y-%m-%d %H:%M:%S"),
                email,
                total_played
            ),
            format!("成功 {} 局", total_won),
            format!("失败 {} 局", total_failed),
            format!("放弃 {} 局", total_abandoned),
            format!("总收益 {:.8}", total_reward),
            format_balance_clause(balance_after),
        ]),
    )
}

fn format_round_result_line(result: &RoundResultSummary) -> String {
    let when = beijing_time(result.when_unix_ms);
    join_clauses(&[
        format!(
            "[{}] {} 的{}难度第 {} 局（{}，对局 {}）已结算：{}",
            when.format("%Y-%m-%d %H:%M:%S"),
            result.email,
            localized_difficulty(&result.difficulty),
            result.round_index.max(1),
            round_mode_label(result.continued),
            result.session_id,
            round_status_label(result)
        ),
        format!("收益 {:.8}", result.reward),
        format_balance_clause(result.balance_after),
        format!("今天这个难度还剩 {} 次", result.remaining_after),
        format!("这一局走了 {} 步", result.move_count),
        format_powerups_clause(&result.used_powerups),
        format!("耗时 {}ms", result.duration_ms),
        format_reason_clause(&result.error_message),
    ])
}

fn format_difficulty_summary_line(summary: &AccountRunSummary) -> String {
    join_clauses(&[
        format!(
            "[{}] {} 的{}难度已跑完：一共玩了 {} 局",
            beijing_time(summary.when_unix_ms).format("%Y-%m-%d %H:%M:%S"),
            summary.email,
            localized_difficulty(&summary.difficulty),
            summary.played
        ),
        format!("成功 {} 局", summary.won),
        format!("放弃 {} 局", summary.abandoned),
        format!("失败 {} 局", summary.failed),
        format!("总收益 {:.8}", summary.total_reward),
        format!("今天这个难度还剩 {} 次", summary.remaining_after),
        format_balance_clause(summary.balance_after),
        format_reason_clause(&summary.error_message),
    ])
}

fn format_balance_clause(balance: Option<f64>) -> String {
    balance
        .map(|value| format!("当前余额 {:.8}", value))
        .unwrap_or_default()
}

fn format_powerups_clause(used_powerups: &[String]) -> String {
    if used_powerups.is_empty() {
        return "未使用道具".to_string();
    }
    format!(
        "用到的道具：{}",
        used_powerups
            .iter()
            .map(|item| powerup_label(item))
            .collect::<Vec<_>>()
            .join("、")
    )
}

fn round_status_label(result: &RoundResultSummary) -> String {
    if !result.error_message.trim().is_empty() {
        return "失败".to_string();
    }
    match result.status.trim().to_ascii_lowercase().as_str() {
        "won" => "成功通关".to_string(),
        "lost" | "failed" | "game_over" => "未通关".to_string(),
        "abandoned" => "已放弃".to_string(),
        "undo" => "用到撤回后暂停重算".to_string(),
        "remove" => "用到移除后暂停重算".to_string(),
        "shuffle" => "用到洗牌后暂停重算".to_string(),
        "pending" | "running" | "active" => "残局未结算".to_string(),
        _ if result.status.trim().is_empty() => "残局未结算".to_string(),
        _ => result.status.clone(),
    }
}

fn format_runtime_round_result_line(result: &RoundResultSummary) -> String {
    let reason = if result.error_message.trim().is_empty() {
        String::new()
    } else {
        format!("，原因：{}", result.error_message.trim())
    };
    format!(
        "账号 {} 的{}难度{}结果：{}，{}，耗时 {}ms，奖励 {:.8}，当前余额 {}，今天还剩 {} 次，走了 {} 步{}。",
        result.email,
        localized_difficulty(&result.difficulty),
        super::format_round_progress(result.round_index, result.round_total),
        round_status_label(result),
        if result.used_powerups.is_empty() {
            "未使用道具".to_string()
        } else {
            format!(
                "用了{}",
                result
                    .used_powerups
                    .iter()
                    .map(|item| powerup_label(item))
                    .collect::<Vec<_>>()
                    .join("、")
            )
        },
        result.duration_ms,
        result.reward,
        result
            .balance_after
            .map(|value| format!("{:.8}", value))
            .unwrap_or_else(|| "未知".to_string()),
        result.remaining_after,
        result.move_count,
        reason,
    )
}

pub(super) fn log_round_result(log: &ui::TaskLog, result: &RoundResultSummary) {
    log.line(format_runtime_round_result_line(result));
}

pub(super) fn localized_difficulty(difficulty: &str) -> String {
    match difficulty.trim().to_ascii_lowercase().as_str() {
        "easy" => "简单".to_string(),
        "normal" => "普通".to_string(),
        "hard" => "困难".to_string(),
        "hell" => "地狱".to_string(),
        _ => difficulty.to_string(),
    }
}

pub(super) fn localized_difficulty_list(difficulties: &[&str]) -> String {
    difficulties
        .iter()
        .map(|item| localized_difficulty(item))
        .collect::<Vec<_>>()
        .join("、")
}

fn powerup_label(kind: &str) -> String {
    match kind.trim().to_ascii_lowercase().as_str() {
        "undo" => "撤回".to_string(),
        "remove" => "移除".to_string(),
        "shuffle" => "洗牌".to_string(),
        "abandon" => "放弃".to_string(),
        _ => kind.to_string(),
    }
}
