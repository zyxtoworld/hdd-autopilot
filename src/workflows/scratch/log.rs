use std::io;
use std::path::Path;

use crate::model::{
    SCRATCH_GAME_TYPE_ICON_MATCH, SCRATCH_GAME_TYPE_LUCKY_NUMBERS, SCRATCH_GAME_TYPE_PROGRESS_RUN,
    SCRATCH_GAME_TYPE_THREE_KIND, SCRATCH_GAME_TYPE_TREASURE_CHEST, ScratchRoundResult,
};
use crate::ui;
use crate::workflows::common::{
    append_account_log_line, format_amount, format_duration_ms as common_format_duration_ms,
};

pub(super) fn append_scratch_round_log(
    log_dir: &Path,
    email: &str,
    result: &ScratchRoundResult,
    total_cost: f64,
    total_reward: f64,
) -> io::Result<()> {
    append_account_log_line(
        log_dir,
        email,
        &format_scratch_round_log_line(result, total_cost, total_reward),
    )
}

fn format_scratch_round_log_line(
    result: &ScratchRoundResult,
    total_cost: f64,
    total_reward: f64,
) -> String {
    let mut lines = Vec::new();
    lines.push(format!(
        "===== 第 {} 轮结束，用时 {} =====",
        result.round,
        format_duration_ms(result.duration_ms)
    ));

    if !result.play_error_message.is_empty() {
        lines.push(format!("这一轮开局失败了：{}", result.play_error_message));
        lines.push(format_stats_line(
            total_cost,
            total_reward,
            round_balance_suffix(result),
        ));
        return lines.join("\n") + "\n";
    }

    if let Some(play_resp) = &result.play_resp {
        lines.push(format!(
            "这一轮已经开局：玩法是{}，对局编号是 {}，目前状态是{}，这一局花了 {}，当前余额 {}。",
            game_type_label(&play_resp.game_type),
            play_resp.play_id,
            play_status_label(&play_resp.status),
            format_amount(play_resp.cost_amount),
            format_amount(play_resp.balance),
        ));
    } else if let Some(play_history_item) = &result.play_history_item {
        lines.push(format!(
            "这一轮先补开之前没处理完的旧对局：对局编号 {}，原来状态是{}，这一局花了 {}。",
            play_history_item.id,
            play_status_label(&play_history_item.status),
            format_amount(play_history_item.cost_amount.unwrap_or(0.0)),
        ));
    }

    if !result.play_history_error_message.is_empty() {
        lines.push(format!(
            "开局记录同步失败：{}",
            result.play_history_error_message
        ));
        lines.push(format_stats_line(
            total_cost,
            total_reward,
            round_balance_suffix(result),
        ));
        return lines.join("\n") + "\n";
    }
    if !result.reveal_error_message.is_empty() {
        lines.push(format!("这一轮开奖失败了：{}", result.reveal_error_message));
        lines.push(format_stats_line(
            total_cost,
            total_reward,
            round_balance_suffix(result),
        ));
        return lines.join("\n") + "\n";
    }
    if let Some(reveal_resp) = &result.reveal_resp {
        lines.push(format!(
            "这一轮已经开奖：玩法是{}，对局编号是 {}，结果是{}，奖金 {}，净收益 {}，当前余额 {}。",
            game_type_label(&reveal_resp.game_type),
            reveal_resp.play_id,
            round_outcome_label(result),
            format_amount(reveal_resp.reward_amount),
            format_amount(reveal_resp.net_amount),
            format_amount(reveal_resp.balance),
        ));
    }
    if !result.reveal_history_error_message.is_empty() {
        lines.push(format!(
            "开奖记录同步失败：{}",
            result.reveal_history_error_message
        ));
    }
    lines.push(format_stats_line(
        total_cost,
        total_reward,
        round_balance_suffix(result),
    ));
    lines.join("\n") + "\n"
}

pub(super) fn log_round_result(
    log: &ui::TaskLog,
    result: &ScratchRoundResult,
    total_cost: f64,
    total_reward: f64,
) {
    log.line(format_scratch_round_log_line(
        result,
        total_cost,
        total_reward,
    ));
}

fn format_stats_line(total_cost: f64, total_reward: f64, balance_suffix: String) -> String {
    format!(
        "累计情况：到现在一共花了 {}，一共中了 {}，净收益 {}{}。",
        format_amount(total_cost),
        format_amount(total_reward),
        format_amount(total_reward - total_cost),
        balance_suffix
    )
}

fn round_balance_suffix(result: &ScratchRoundResult) -> String {
    match round_balance(result) {
        Some(balance) => format!("，当前余额 {}", format_amount(balance)),
        None => String::new(),
    }
}

fn round_balance(result: &ScratchRoundResult) -> Option<f64> {
    if let Some(reveal_resp) = &result.reveal_resp {
        return Some(reveal_resp.balance);
    }
    if let Some(play_resp) = &result.play_resp {
        return Some(play_resp.balance);
    }
    None
}

fn round_reward(result: &ScratchRoundResult) -> f64 {
    if let Some(history) = &result.reveal_history_item {
        return history.reward_amount.unwrap_or(0.0);
    }
    if let Some(reveal) = &result.reveal_resp {
        return reveal.reward_amount;
    }
    0.0
}

fn round_outcome_label(result: &ScratchRoundResult) -> &'static str {
    if result.reveal_resp.is_none() && result.reveal_history_item.is_none() {
        return "本局未完成";
    }
    if round_reward(result) > 0.0 {
        return "中奖了";
    }
    "未中奖"
}

fn game_type_label(game_type: &str) -> &str {
    match game_type.trim() {
        SCRATCH_GAME_TYPE_LUCKY_NUMBERS => "幸运数字",
        SCRATCH_GAME_TYPE_THREE_KIND => "三连相同",
        SCRATCH_GAME_TYPE_ICON_MATCH => "图标配对",
        SCRATCH_GAME_TYPE_TREASURE_CHEST => "宝箱开奖",
        SCRATCH_GAME_TYPE_PROGRESS_RUN => "进度冲刺",
        _ if game_type.trim().is_empty() => "未知玩法",
        _ => game_type.trim(),
    }
}

fn play_status_label(status: &str) -> &str {
    match status.trim().to_ascii_lowercase().as_str() {
        "pending" => "等待开奖",
        "played" => "已开局",
        "revealed" | "done" | "granted" => "已开奖",
        _ if status.trim().is_empty() => "状态未知",
        _ => status.trim(),
    }
}

fn format_duration_ms(duration_ms: i64) -> String {
    common_format_duration_ms(duration_ms)
}
