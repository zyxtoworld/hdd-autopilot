use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use chrono::{TimeZone, Utc};

use crate::ui;

use super::types::{Puzzle15DifficultySummary, Puzzle15RoundSummary};

pub(super) fn append_run_header(log_dir: &Path, email: &str, when_unix_ms: i64) -> io::Result<()> {
    append_line(
        log_dir,
        email,
        &format!(
            "[{}] 开始运行，正在处理账号 {}\n",
            beijing_time(when_unix_ms).format("%Y-%m-%d %H:%M:%S"),
            email
        ),
    )
}

pub(super) fn append_round_result(log_dir: &Path, result: &Puzzle15RoundSummary) -> io::Result<()> {
    append_line(log_dir, &result.email, &format_round_result_line(result))
}

pub(super) fn append_difficulty_summary(
    log_dir: &Path,
    summary: &Puzzle15DifficultySummary,
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
    summaries: &[Puzzle15DifficultySummary],
) -> io::Result<()> {
    let total_played: i32 = summaries.iter().map(|item| item.played).sum();
    let total_won: i32 = summaries.iter().map(|item| item.won).sum();
    let total_failed: i32 = summaries.iter().map(|item| item.failed).sum();
    let total_reward: f64 = summaries.iter().map(|item| item.total_reward).sum();
    append_line(
        log_dir,
        email,
        &join_clauses(&[
            format!(
                "[{}] 账号 {} 的华容道全部难度汇总：一共玩了 {} 局",
                beijing_time(when_unix_ms).format("%Y-%m-%d %H:%M:%S"),
                email,
                total_played
            ),
            format!("成功 {} 局", total_won),
            format!("失败 {} 局", total_failed),
            format!("总收益 {:.8}", total_reward),
        ]),
    )
}

pub(super) fn log_round_result(log: &ui::TaskLog, result: &Puzzle15RoundSummary) {
    log.line(format_runtime_round_result_line(result));
}

pub(super) fn localized_difficulty(difficulty: &str) -> String {
    match difficulty.trim().to_ascii_lowercase().as_str() {
        crate::model::PUZZLE_15_DIFFICULTY_EASY => "入门".to_string(),
        crate::model::PUZZLE_15_DIFFICULTY_CLASSIC => "经典".to_string(),
        crate::model::PUZZLE_15_DIFFICULTY_HARD => "挑战".to_string(),
        _ => difficulty.to_string(),
    }
}

pub(super) fn localized_difficulty_list(difficulties: &[String]) -> String {
    difficulties
        .iter()
        .map(|item| localized_difficulty(item))
        .collect::<Vec<_>>()
        .join("、")
}

fn format_round_result_line(result: &Puzzle15RoundSummary) -> String {
    join_clauses(&[
        format!(
            "[{}] 账号 {} 的{}难度第 {} 局（{}，对局 {}）已结算：{}",
            beijing_time(result.when_unix_ms).format("%Y-%m-%d %H:%M:%S"),
            result.email,
            localized_difficulty(&result.difficulty),
            result.round_index.max(1),
            round_mode_label(result.continued),
            result.session_id,
            round_status_label(result)
        ),
        format!("收益 {:.8}", result.reward),
        format!("今天这个难度还剩 {} 次", result.remaining_after),
        format!("棋盘 {}x{}", result.size, result.size),
        format!("计划 {} 步", result.planned_steps),
        format!("累计移动 {} 步", result.move_count),
        format!("耗时 {}ms", result.duration_ms),
        format_reason_clause(&result.error_message),
    ])
}

fn format_difficulty_summary_line(summary: &Puzzle15DifficultySummary) -> String {
    join_clauses(&[
        format!(
            "[{}] 账号 {} 的{}难度已跑完：一共玩了 {} 局",
            beijing_time(summary.when_unix_ms).format("%Y-%m-%d %H:%M:%S"),
            summary.email,
            localized_difficulty(&summary.difficulty),
            summary.played
        ),
        format!("成功 {} 局", summary.won),
        format!("失败 {} 局", summary.failed),
        format!("总收益 {:.8}", summary.total_reward),
        format!("今天这个难度还剩 {} 次", summary.remaining_after),
        format_reason_clause(&summary.error_message),
    ])
}

fn format_runtime_round_result_line(result: &Puzzle15RoundSummary) -> String {
    let reason = if result.error_message.trim().is_empty() {
        String::new()
    } else {
        format!("，原因：{}", result.error_message.trim())
    };
    format!(
        "账号 {} 的华容道{}难度{}结果：{}，棋盘 {}x{}，计划 {} 步，累计移动 {} 步，耗时 {}ms，奖励 {:.8}，今天还剩 {} 次{}。",
        result.email,
        localized_difficulty(&result.difficulty),
        format_round_progress(result.round_index, result.round_total),
        round_status_label(result),
        result.size,
        result.size,
        result.planned_steps,
        result.move_count,
        result.duration_ms,
        result.reward,
        result.remaining_after,
        reason,
    )
}

fn append_line(log_dir: &Path, email: &str, content: &str) -> io::Result<()> {
    fs::create_dir_all(log_dir)?;
    let path = log_file_path(log_dir, email);
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    file.write_all(content.as_bytes())?;
    file.flush()
}

fn log_file_path(log_dir: &Path, email: &str) -> PathBuf {
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

fn join_clauses(parts: &[String]) -> String {
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

fn round_mode_label(continued: bool) -> &'static str {
    if continued { "续玩" } else { "新开局" }
}

fn round_status_label(result: &Puzzle15RoundSummary) -> String {
    if !result.error_message.trim().is_empty() {
        return "失败".to_string();
    }
    match result.status.trim().to_ascii_lowercase().as_str() {
        "won" => "成功通关".to_string(),
        "pending" => "已暂停".to_string(),
        "game_over" | "lost" | "failed" => "未通关".to_string(),
        _ if result.status.trim().is_empty() => "已结束".to_string(),
        _ => result.status.clone(),
    }
}

fn format_round_progress(current: i32, total: i32) -> String {
    format!("今天第 {}/{} 局", current.max(1), total.max(current.max(1)))
}

fn format_reason_clause(error_message: &str) -> String {
    let error_message = error_message.trim();
    if error_message.is_empty() {
        return String::new();
    }
    format!("原因：{}", error_message)
}

fn beijing_time(when_unix_ms: i64) -> chrono::DateTime<chrono::FixedOffset> {
    Utc.timestamp_millis_opt(when_unix_ms)
        .single()
        .unwrap_or_else(|| {
            Utc.timestamp_millis_opt(super::current_unix_ms())
                .single()
                .unwrap()
        })
        .with_timezone(&chrono::FixedOffset::east_opt(8 * 60 * 60).unwrap())
}
