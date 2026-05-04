use std::io;
use std::path::Path;

use crate::ui;
use crate::workflows::common::{
    append_account_log_line as append_line, beijing_time, format_amount, format_duration_ms,
    join_log_clauses as join_clauses, reason_clause as format_reason_clause, round_mode_label,
    round_progress_label as format_round_progress,
};

use super::types::{MinesweeperDifficultySummary, MinesweeperRoundSummary};

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

pub(super) fn append_round_result(
    log_dir: &Path,
    result: &MinesweeperRoundSummary,
) -> io::Result<()> {
    append_line(log_dir, &result.email, &format_round_result_line(result))
}

pub(super) fn append_difficulty_summary(
    log_dir: &Path,
    summary: &MinesweeperDifficultySummary,
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
    summaries: &[MinesweeperDifficultySummary],
) -> io::Result<()> {
    let total_played: i32 = summaries.iter().map(|item| item.played).sum();
    let total_won: i32 = summaries.iter().map(|item| item.won).sum();
    let total_lost: i32 = summaries.iter().map(|item| item.lost).sum();
    let total_failed: i32 = summaries.iter().map(|item| item.failed).sum();
    let total_reward: f64 = summaries.iter().map(|item| item.total_reward).sum();
    append_line(
        log_dir,
        email,
        &join_clauses(&[
            format!(
                "[{}] 账号 {} 的扫雷汇总：一共玩了 {} 局",
                beijing_time(when_unix_ms).format("%Y-%m-%d %H:%M:%S"),
                email,
                total_played
            ),
            format!("成功 {} 局", total_won),
            format!("踩雷 {} 局", total_lost),
            format!("异常 {} 局", total_failed),
            format!("总收益 {}", format_amount(total_reward)),
        ]),
    )
}

pub(super) fn log_round_result(log: &ui::TaskLog, result: &MinesweeperRoundSummary) {
    log.line(format_runtime_round_result_line(result));
}

pub(super) fn localized_difficulty(difficulty: &str) -> String {
    match difficulty.trim().to_ascii_lowercase().as_str() {
        crate::model::MINESWEEPER_DIFFICULTY_BEGINNER => "初级".to_string(),
        crate::model::MINESWEEPER_DIFFICULTY_INTERMEDIATE => "中级".to_string(),
        crate::model::MINESWEEPER_DIFFICULTY_EXPERT => "高级".to_string(),
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

pub(super) fn remaining_after_clause(remaining_after: i32) -> String {
    if remaining_after < 0 {
        "今天剩余次数未知".to_string()
    } else {
        format!("今天还剩 {} 局", remaining_after)
    }
}

fn format_round_result_line(result: &MinesweeperRoundSummary) -> String {
    join_clauses(&[
        format!(
            "[{}] 账号 {} 的扫雷{}难度第 {} 局（{}，对局 {}）已结算：{}",
            beijing_time(result.when_unix_ms).format("%Y-%m-%d %H:%M:%S"),
            result.email,
            localized_difficulty(&result.difficulty),
            result.round_index.max(1),
            round_mode_label(result.continued),
            result.play_id,
            round_status_label(result)
        ),
        format!("收益 {}", format_amount(result.reward)),
        remaining_after_clause(result.remaining_after),
        format!(
            "棋盘 {}x{}，{} 雷",
            result.rows, result.cols, result.mine_count
        ),
        format!("执行 {} 步", result.executed_moves),
        format!("翻开 {} 步", result.safe_reveals),
        format!("标旗 {} 步", result.flags),
        format!("双击 {} 步", result.chords),
        format!("概率猜测 {} 步", result.guesses),
        format!("耗时 {}", format_duration_ms(result.duration_ms)),
        format_reason_clause(&result.error_message),
    ])
}

fn format_difficulty_summary_line(summary: &MinesweeperDifficultySummary) -> String {
    join_clauses(&[
        format!(
            "[{}] 账号 {} 的扫雷{}难度已跑完：一共玩了 {} 局",
            beijing_time(summary.when_unix_ms).format("%Y-%m-%d %H:%M:%S"),
            summary.email,
            localized_difficulty(&summary.difficulty),
            summary.played
        ),
        format!("成功 {} 局", summary.won),
        format!("踩雷 {} 局", summary.lost),
        format!("异常 {} 局", summary.failed),
        format!("总收益 {}", format_amount(summary.total_reward)),
        remaining_after_clause(summary.remaining_after),
        format_reason_clause(&summary.error_message),
    ])
}

fn format_runtime_round_result_line(result: &MinesweeperRoundSummary) -> String {
    let reason = if result.error_message.trim().is_empty() {
        String::new()
    } else {
        format!("，原因：{}", result.error_message.trim())
    };
    format!(
        "账号 {} 的扫雷{}难度{}结果：{}，棋盘 {}x{}，{} 雷，执行 {} 步（翻开 {}、标旗 {}、双击 {}、猜测 {}），耗时 {}，奖励 {}，{}{}。",
        result.email,
        localized_difficulty(&result.difficulty),
        format_round_progress(result.round_index, result.round_total),
        round_status_label(result),
        result.rows,
        result.cols,
        result.mine_count,
        result.executed_moves,
        result.safe_reveals,
        result.flags,
        result.chords,
        result.guesses,
        format_duration_ms(result.duration_ms),
        format_amount(result.reward),
        remaining_after_clause(result.remaining_after),
        reason,
    )
}

fn round_status_label(result: &MinesweeperRoundSummary) -> String {
    if !result.error_message.trim().is_empty() {
        return "异常".to_string();
    }
    match result.status.trim().to_ascii_lowercase().as_str() {
        "won" => "成功通关".to_string(),
        "lost" | "failed" | "game_over" => "踩雷未通关".to_string(),
        "pending" | "running" | "active" => "残局未结算".to_string(),
        _ if result.status.trim().is_empty() => "已结束".to_string(),
        _ => result.status.clone(),
    }
}
