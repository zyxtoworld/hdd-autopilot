use std::io;
use std::path::Path;

use crate::ui;
use crate::workflows::common::{
    append_account_log_line as append_line, beijing_time, join_log_clauses as join_clauses,
    reason_clause as format_reason_clause, round_mode_label,
    round_progress_label as format_round_progress,
};

use super::types::{SudokuDifficultySummary, SudokuRoundSummary};

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

pub(super) fn append_round_result(log_dir: &Path, result: &SudokuRoundSummary) -> io::Result<()> {
    append_line(log_dir, &result.email, &format_round_result_line(result))
}

pub(super) fn append_difficulty_summary(
    log_dir: &Path,
    summary: &SudokuDifficultySummary,
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
    summaries: &[SudokuDifficultySummary],
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
                "[{}] 账号 {} 的数独全部难度汇总：一共玩了 {} 局",
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

pub(super) fn log_round_result(log: &ui::TaskLog, result: &SudokuRoundSummary) {
    log.line(format_runtime_round_result_line(result));
}

pub(super) fn localized_difficulty(difficulty: &str) -> String {
    match difficulty.trim().to_ascii_lowercase().as_str() {
        crate::model::SUDOKU_DIFFICULTY_EASY => "入门".to_string(),
        crate::model::SUDOKU_DIFFICULTY_NORMAL => "经典".to_string(),
        crate::model::SUDOKU_DIFFICULTY_HARD => "挑战".to_string(),
        crate::model::SUDOKU_DIFFICULTY_EXPERT => "专家".to_string(),
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

fn format_round_result_line(result: &SudokuRoundSummary) -> String {
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
        format!(
            "棋盘 {}x{}，{}x{} 宫",
            result.size, result.size, result.box_size, result.box_size
        ),
        format!("计划填 {} 格", result.planned_fills),
        format!("实际填 {} 格", result.filled_cells),
        format!("累计操作 {} 次", result.move_count),
        format!("耗时 {}ms", result.duration_ms),
        format_reason_clause(&result.error_message),
    ])
}

fn format_difficulty_summary_line(summary: &SudokuDifficultySummary) -> String {
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

fn format_runtime_round_result_line(result: &SudokuRoundSummary) -> String {
    let reason = if result.error_message.trim().is_empty() {
        String::new()
    } else {
        format!("，原因：{}", result.error_message.trim())
    };
    format!(
        "账号 {} 的数独{}难度{}结果：{}，棋盘 {}x{}，计划填 {} 格，实际填 {} 格，累计操作 {} 次，耗时 {}ms，奖励 {:.8}，今天还剩 {} 次{}。",
        result.email,
        localized_difficulty(&result.difficulty),
        format_round_progress(result.round_index, result.round_total),
        round_status_label(result),
        result.size,
        result.size,
        result.planned_fills,
        result.filled_cells,
        result.move_count,
        result.duration_ms,
        result.reward,
        result.remaining_after,
        reason,
    )
}

fn round_status_label(result: &SudokuRoundSummary) -> String {
    if !result.error_message.trim().is_empty() {
        return "失败".to_string();
    }
    match result.status.trim().to_ascii_lowercase().as_str() {
        "won" => "成功通关".to_string(),
        "pending" | "running" | "active" => "残局未结算".to_string(),
        "game_over" | "lost" | "failed" => "未通关".to_string(),
        _ if result.status.trim().is_empty() => "已结束".to_string(),
        _ => result.status.clone(),
    }
}
