use std::collections::HashSet;
use std::io;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use crate::model::{MemoryConfigResponse, MemoryFlipResponse, MemorySession, MemoryStartResponse};
use crate::solver::memory::MemorySolver;
use crate::ui;
use crate::workflows::common::{
    AccountRuntime, BatchState, current_unix_ms, is_pending_round_status,
    retry_operation_with_step, with_auth_retry_api_until_success,
};

use super::types::{MemoryDifficultySummary, MemoryRoundSummary, MemorySnapshot, RoundProgress};

pub(super) fn difficulty_order(config: &MemoryConfigResponse) -> Vec<String> {
    let mut ordered = Vec::new();
    for difficulty in crate::model::MEMORY_DIFFICULTY_ORDER {
        if config.difficulties.contains_key(*difficulty) {
            ordered.push((*difficulty).to_string());
        }
    }
    let seen = ordered.iter().cloned().collect::<HashSet<_>>();
    let mut extra = config
        .difficulties
        .keys()
        .filter(|difficulty| !seen.contains(*difficulty))
        .cloned()
        .collect::<Vec<_>>();
    extra.sort();
    ordered.extend(extra);
    ordered
}

pub(super) fn is_pending_session(session: &MemorySession) -> bool {
    if session.session_id <= 0 || session.won || session.game_over {
        return false;
    }
    if session.pairs > 0 && session.match_count >= session.pairs {
        return false;
    }
    let status = session.status.trim().to_ascii_lowercase();
    status.is_empty() || matches!(status.as_str(), "pending" | "running" | "active")
}

pub(super) fn normalize_round_total(current: i32, total: i32) -> i32 {
    total.max(current.max(1))
}

pub(super) fn play_round(
    cancel_flag: &ui::CancelFlag,
    state: &Arc<Mutex<BatchState>>,
    runtime: &mut AccountRuntime,
    start: MemorySnapshot,
    continued: bool,
    progress: RoundProgress,
) -> io::Result<MemoryRoundSummary> {
    let started = Instant::now();
    let mut snapshot = start;
    let mut solver = MemorySolver::new();
    solver.remember_many(&snapshot.currently_revealed);
    let mut consecutive_fail = 0;

    loop {
        ui::check_cancel(cancel_flag)?;
        if is_finished(&snapshot) {
            return Ok(build_round_summary(
                runtime.email(),
                &snapshot,
                continued,
                &progress,
                started,
                String::new(),
            ));
        }
        if snapshot.peek_limit > 0 && snapshot.peek_count >= snapshot.peek_limit {
            return Ok(build_round_summary(
                runtime.email(),
                &snapshot,
                continued,
                &progress,
                started,
                "已达到翻牌次数上限".to_string(),
            ));
        }

        let Some(index) = solver.choose_next(
            snapshot.total_cards(),
            &snapshot.matched_indices,
            &snapshot.currently_revealed,
        ) else {
            return Ok(build_round_summary(
                runtime.email(),
                &snapshot,
                continued,
                &progress,
                started,
                "没有可继续翻开的格子".to_string(),
            ));
        };

        match flip_once(
            cancel_flag,
            state,
            runtime,
            snapshot.session_id,
            index,
            snapshot.peek_count + 1,
        ) {
            Ok(step) => {
                if !step.ok {
                    consecutive_fail += 1;
                    if consecutive_fail >= 3 {
                        return Ok(build_round_summary(
                            runtime.email(),
                            &snapshot,
                            continued,
                            &progress,
                            started,
                            "连续翻牌失败".to_string(),
                        ));
                    }
                    continue;
                }
                consecutive_fail = 0;
                remember_flip_response(&mut solver, &step);
                snapshot = snapshot_from_flip_response(&snapshot, &step);
                solver.remember_many(&snapshot.currently_revealed);
            }
            Err(error) => {
                if error.kind() == io::ErrorKind::TimedOut {
                    return Err(error);
                }
                if is_game_already_finished_error(&error.to_string()) {
                    return Ok(build_round_summary(
                        runtime.email(),
                        &snapshot,
                        continued,
                        &progress,
                        started,
                        error.to_string(),
                    ));
                }
                consecutive_fail += 1;
                if consecutive_fail >= 3 {
                    return Ok(build_round_summary(
                        runtime.email(),
                        &snapshot,
                        continued,
                        &progress,
                        started,
                        format!("连续翻牌失败：{}", error),
                    ));
                }
            }
        }
    }
}

fn flip_once(
    cancel_flag: &ui::CancelFlag,
    state: &Arc<Mutex<BatchState>>,
    runtime: &mut AccountRuntime,
    session_id: i32,
    index: i32,
    step_number: i32,
) -> io::Result<MemoryFlipResponse> {
    let operation = retry_operation_with_step("memory flip", step_number);
    with_auth_retry_api_until_success(
        cancel_flag,
        state,
        runtime,
        &operation,
        |client, auth_token| client.flip_memory(auth_token, session_id, index),
    )
}

fn remember_flip_response(solver: &mut MemorySolver, response: &MemoryFlipResponse) {
    solver.remember(&crate::model::MemoryCard {
        index: response.index,
        symbol: response.symbol,
    });
    if let Some(other) = &response.other {
        solver.remember(other);
    }
    solver.remember_many(&response.currently_revealed);
    solver.remember_many(&response.session.currently_revealed);
}

pub(super) fn snapshot_from_start_response(start: &MemoryStartResponse) -> MemorySnapshot {
    MemorySnapshot {
        difficulty: start.difficulty.clone(),
        session_id: start.session_id,
        rows: start.rows,
        cols: start.cols,
        pairs: start.pairs,
        peek_limit: start.peek_limit,
        status: "pending".to_string(),
        ..MemorySnapshot::default()
    }
}

pub(super) fn snapshot_from_history_item(item: &MemorySession) -> MemorySnapshot {
    MemorySnapshot {
        difficulty: item.difficulty.clone(),
        session_id: item.session_id,
        rows: item.rows,
        cols: item.cols,
        pairs: item.pairs,
        peek_limit: item.peek_limit,
        peek_count: item.peek_count,
        match_count: item.match_count,
        matched_indices: item.matched_indices.clone(),
        currently_revealed: item.currently_revealed.clone(),
        status: item.status.clone(),
        game_over: item.game_over,
        won: item.won,
        reward_amount: item.reward_amount,
    }
}

pub(super) fn merge_round_into_summary(
    summary: &mut MemoryDifficultySummary,
    result: &MemoryRoundSummary,
) {
    summary.played += 1;
    summary.total_reward += result.reward;
    summary.remaining_after = result.remaining_after;
    summary.when_unix_ms = result.when_unix_ms;
    if !result.error_message.trim().is_empty() {
        summary.failed += 1;
        summary.error_message = result.error_message.clone();
        return;
    }
    if result.status.trim().eq_ignore_ascii_case("won")
        || result.pairs > 0 && result.match_count >= result.pairs
    {
        summary.won += 1;
    } else if !is_pending_round_status(&result.status) {
        summary.failed += 1;
    }
}

fn snapshot_from_flip_response(
    previous: &MemorySnapshot,
    response: &MemoryFlipResponse,
) -> MemorySnapshot {
    let session = &response.session;
    let has_session = session.session_id > 0;
    let matched_indices = if has_session {
        session.matched_indices.clone()
    } else {
        response.matched_indices.clone()
    };
    let currently_revealed = if has_session {
        session.currently_revealed.clone()
    } else {
        response.currently_revealed.clone()
    };
    MemorySnapshot {
        difficulty: if has_session && !session.difficulty.trim().is_empty() {
            session.difficulty.clone()
        } else {
            previous.difficulty.clone()
        },
        session_id: if has_session {
            session.session_id
        } else {
            previous.session_id
        },
        rows: prefer_i32(session.rows, previous.rows),
        cols: prefer_i32(session.cols, previous.cols),
        pairs: prefer_i32(session.pairs, previous.pairs),
        peek_limit: prefer_i32(session.peek_limit, previous.peek_limit),
        peek_count: prefer_i32(
            response.peek_count,
            prefer_i32(session.peek_count, previous.peek_count),
        ),
        match_count: if has_session {
            prefer_i32(session.match_count, previous.match_count)
        } else {
            prefer_i32(response.match_count, previous.match_count)
        },
        matched_indices,
        currently_revealed,
        status: first_non_empty(&[
            response.status.as_str(),
            session.status.as_str(),
            response.resolution.as_str(),
            previous.status.as_str(),
        ]),
        game_over: response.game_over || session.game_over,
        won: response.won || session.won,
        reward_amount: prefer_f64(response.reward_amount, session.reward_amount),
    }
}

fn prefer_i32(value: i32, fallback: i32) -> i32 {
    if value != 0 { value } else { fallback }
}

fn prefer_f64(value: f64, fallback: f64) -> f64 {
    if value != 0.0 { value } else { fallback }
}

fn first_non_empty(parts: &[&str]) -> String {
    parts
        .iter()
        .map(|part| part.trim())
        .find(|part| !part.is_empty())
        .unwrap_or("")
        .to_string()
}

fn build_round_summary(
    email: &str,
    snapshot: &MemorySnapshot,
    continued: bool,
    progress: &RoundProgress,
    started: Instant,
    error_message: String,
) -> MemoryRoundSummary {
    MemoryRoundSummary {
        email: email.to_string(),
        difficulty: snapshot.difficulty.clone(),
        round_index: progress.current,
        round_total: progress.total,
        session_id: snapshot.session_id,
        continued,
        status: status_for_snapshot(snapshot),
        reward: snapshot.reward_amount,
        remaining_after: 0,
        peek_count: snapshot.peek_count,
        match_count: snapshot.match_count,
        pairs: snapshot.pairs,
        duration_ms: started.elapsed().as_millis() as i64,
        when_unix_ms: current_unix_ms(),
        error_message,
    }
}

fn is_finished(snapshot: &MemorySnapshot) -> bool {
    snapshot.won
        || snapshot.pairs > 0 && snapshot.match_count >= snapshot.pairs
        || snapshot.game_over
        || matches!(
            snapshot.status.trim().to_ascii_lowercase().as_str(),
            "won" | "lost" | "failed" | "abandoned"
        )
}

fn status_for_snapshot(snapshot: &MemorySnapshot) -> String {
    if snapshot.won || snapshot.pairs > 0 && snapshot.match_count >= snapshot.pairs {
        "won".to_string()
    } else if snapshot.game_over {
        "game_over".to_string()
    } else if snapshot.status.trim().is_empty() {
        "pending".to_string()
    } else {
        snapshot.status.clone()
    }
}

fn is_game_already_finished_error(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    lower.contains("已经结束")
        || lower.contains("已结束")
        || lower.contains("game over")
        || lower.contains("already ended")
}

pub(super) fn is_daily_limit_error(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    lower.contains("次数已经用完")
        || lower.contains("次数已用完")
        || lower.contains("今日次数")
        || lower.contains("daily limit")
}

pub(super) fn is_active_session_error(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    lower.contains("未结束对局")
        || lower.contains("未结束的对局")
        || lower.contains("进行中")
        || lower.contains("active session")
        || lower.contains("max active")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{MemoryDifficultyConfig, MemoryFlipResponse};

    #[test]
    fn pending_session_requires_active_status() {
        let session = MemorySession {
            session_id: 1,
            status: "pending".to_string(),
            pairs: 6,
            match_count: 1,
            ..MemorySession::default()
        };
        let ended = MemorySession {
            status: "abandoned".to_string(),
            ..session.clone()
        };

        assert!(is_pending_session(&session));
        assert!(!is_pending_session(&ended));
    }

    #[test]
    fn difficulty_order_keeps_known_order_then_sorted_extras() {
        let mut config = MemoryConfigResponse::default();
        config
            .difficulties
            .insert("zzz".to_string(), MemoryDifficultyConfig::default());
        config.difficulties.insert(
            crate::model::MEMORY_DIFFICULTY_HARD.to_string(),
            MemoryDifficultyConfig::default(),
        );
        config.difficulties.insert(
            crate::model::MEMORY_DIFFICULTY_EASY.to_string(),
            MemoryDifficultyConfig::default(),
        );

        assert_eq!(difficulty_order(&config), vec!["easy", "hard", "zzz"]);
    }

    #[test]
    fn snapshot_from_flip_uses_session_state_when_present() {
        let previous = MemorySnapshot {
            difficulty: "easy".to_string(),
            session_id: 7,
            rows: 3,
            cols: 4,
            pairs: 6,
            peek_limit: 24,
            ..MemorySnapshot::default()
        };
        let response = MemoryFlipResponse {
            ok: true,
            index: 1,
            symbol: 2,
            peek_count: 2,
            match_count: 1,
            session: MemorySession {
                session_id: 7,
                difficulty: "easy".to_string(),
                rows: 3,
                cols: 4,
                pairs: 6,
                peek_limit: 24,
                match_count: 1,
                matched_indices: vec![0, 1],
                status: "pending".to_string(),
                ..MemorySession::default()
            },
            ..MemoryFlipResponse::default()
        };

        let snapshot = snapshot_from_flip_response(&previous, &response);

        assert_eq!(snapshot.match_count, 1);
        assert_eq!(snapshot.matched_indices, vec![0, 1]);
    }

    #[test]
    fn pending_status_is_ignored_not_failed() {
        let mut summary = MemoryDifficultySummary::default();
        let result = MemoryRoundSummary {
            status: "pending".to_string(),
            pairs: 6,
            match_count: 3,
            ..MemoryRoundSummary::default()
        };

        merge_round_into_summary(&mut summary, &result);

        assert_eq!(summary.won, 0);
        assert_eq!(summary.failed, 0);
    }
}
