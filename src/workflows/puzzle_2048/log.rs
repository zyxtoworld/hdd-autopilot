use std::io;
use std::path::Path;

use crate::ui;
use crate::workflows::common::{
    append_account_log_line as append_line, format_amount, format_duration_ms, format_log_time,
    join_log_clauses as join_clauses, reason_clause as format_reason_clause, round_mode_label,
    round_progress_label as format_round_progress,
};

use super::types::{PuzzleDifficultySummary, PuzzleRoundSummary};

pub(super) fn append_run_header(log_dir: &Path, email: &str, when_unix_ms: i64) -> io::Result<()> {
    let when = format_log_time(when_unix_ms);
    append_line(
        log_dir,
        email,
        &format!("[{}] 开始运行，正在处理账号 {}\n", when, email),
    )
}

pub(super) fn append_round_result(log_dir: &Path, result: &PuzzleRoundSummary) -> io::Result<()> {
    append_line(log_dir, &result.email, &format_round_result_line(result))
}

pub(super) fn append_difficulty_summary(
    log_dir: &Path,
    summary: &PuzzleDifficultySummary,
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
    summaries: &[PuzzleDifficultySummary],
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
                "[{}] 账号 {} 的谜题2048全部难度汇总：一共玩了 {} 局",
                format_log_time(when_unix_ms),
                email,
                total_played
            ),
            format!("成功 {} 局", total_won),
            format!("失败 {} 局", total_failed),
            format!("总收益 {}", format_amount(total_reward)),
        ]),
    )
}

pub(super) fn log_round_result(log: &ui::TaskLog, result: &PuzzleRoundSummary) {
    log.line(format_runtime_round_result_line(result));
}

pub(super) fn localized_difficulty(difficulty: &str) -> String {
    match difficulty.trim().to_ascii_lowercase().as_str() {
        crate::model::PUZZLE_2048_DIFFICULTY_MINI => "入门".to_string(),
        crate::model::PUZZLE_2048_DIFFICULTY_CLASSIC => "经典".to_string(),
        crate::model::PUZZLE_2048_DIFFICULTY_JUMBO => "挑战".to_string(),
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

pub(super) fn localized_direction(direction: &str) -> String {
    match direction.trim().to_ascii_lowercase().as_str() {
        "up" => "上".to_string(),
        "down" => "下".to_string(),
        "left" => "左".to_string(),
        "right" => "右".to_string(),
        _ => direction.to_string(),
    }
}

fn format_round_result_line(result: &PuzzleRoundSummary) -> String {
    join_clauses(&[
        format!(
            "[{}] 账号 {} 的{}难度第 {} 局（{}，对局 {}）已结算：{}",
            format_log_time(result.when_unix_ms),
            result.email,
            localized_difficulty(&result.difficulty),
            result.round_index.max(1),
            round_mode_label(result.continued),
            result.session_id,
            round_status_label(result)
        ),
        format!("收益 {}", format_amount(result.reward)),
        format!("今天这个难度还剩 {} 次", result.remaining_after),
        format!("这一局走了 {} 步", result.move_count),
        format!("最大数字 {}", result.max_tile),
        format!("分数 {}", result.score),
        format!("耗时 {}", format_duration_ms(result.duration_ms)),
        format_reason_clause(&result.error_message),
    ])
}

fn format_difficulty_summary_line(summary: &PuzzleDifficultySummary) -> String {
    join_clauses(&[
        format!(
            "[{}] 账号 {} 的{}难度已跑完：一共玩了 {} 局",
            format_log_time(summary.when_unix_ms),
            summary.email,
            localized_difficulty(&summary.difficulty),
            summary.played
        ),
        format!("成功 {} 局", summary.won),
        format!("失败 {} 局", summary.failed),
        format!("总收益 {}", format_amount(summary.total_reward)),
        format!("今天这个难度还剩 {} 次", summary.remaining_after),
        format_reason_clause(&summary.error_message),
    ])
}

fn format_runtime_round_result_line(result: &PuzzleRoundSummary) -> String {
    let reason = if result.error_message.trim().is_empty() {
        String::new()
    } else {
        format!("，原因：{}", result.error_message.trim())
    };
    format!(
        "账号 {} 的谜题2048{}难度{}结果：{}，最大数字 {}，分数 {}，耗时 {}，奖励 {}，今天还剩 {} 次，走了 {} 步{}。",
        result.email,
        localized_difficulty(&result.difficulty),
        format_round_progress(result.round_index, result.round_total),
        round_status_label(result),
        result.max_tile,
        result.score,
        format_duration_ms(result.duration_ms),
        format_amount(result.reward),
        result.remaining_after,
        result.move_count,
        reason,
    )
}

fn round_status_label(result: &PuzzleRoundSummary) -> String {
    if !result.error_message.trim().is_empty() {
        return "失败".to_string();
    }
    match result.status.trim().to_ascii_lowercase().as_str() {
        "won" => "成功合成目标数字".to_string(),
        "game_over" | "lost" | "failed" => "未合成目标数字".to_string(),
        "pending" | "running" | "active" => "残局未结算".to_string(),
        _ if result.status.trim().is_empty() => "已结束".to_string(),
        _ => result.status.clone(),
    }
}
