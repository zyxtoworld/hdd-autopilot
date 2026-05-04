use std::collections::HashSet;
use std::io;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use crate::model::{
    Puzzle15ConfigResponse, Puzzle15MoveResponse, Puzzle15Session, Puzzle15StartResponse,
};
use crate::solver::puzzle_15;
use crate::ui;
use crate::workflows::common::{
    AccountRuntime, BatchState, current_unix_ms, is_pending_round_status,
    retry_operation_with_step, with_auth_retry_api_until_success,
};

use super::types::{
    Puzzle15DifficultySummary, Puzzle15RoundSummary, Puzzle15Snapshot, RoundProgress,
};

pub(super) fn difficulty_order(config: &Puzzle15ConfigResponse) -> Vec<String> {
    let mut ordered = Vec::new();
    for difficulty in crate::model::PUZZLE_15_DIFFICULTY_ORDER {
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

pub(super) fn is_pending_session(session: &Puzzle15Session) -> bool {
    if session.session_id <= 0 || session.board.is_empty() || session.won {
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
    start: Puzzle15Snapshot,
    continued: bool,
    progress: RoundProgress,
) -> io::Result<Puzzle15RoundSummary> {
    let started = Instant::now();
    let mut snapshot = start;
    let path = match puzzle_15::solve(&snapshot.board, snapshot.size) {
        Ok(path) => path,
        Err(error) => {
            return Ok(build_round_summary(
                runtime.email(),
                &snapshot,
                continued,
                &progress,
                started,
                0,
                format!("求解失败：{}", error),
            ));
        }
    };
    let planned_steps = path.len().min(i32::MAX as usize) as i32;

    for direction in path {
        ui::check_cancel(cancel_flag)?;
        let step = move_once(
            cancel_flag,
            state,
            runtime,
            snapshot.session_id,
            direction.as_api_str(),
            snapshot.move_count + 1,
        )?;
        if !step.ok {
            return Ok(build_round_summary(
                runtime.email(),
                &snapshot,
                continued,
                &progress,
                started,
                planned_steps,
                "移动接口返回 ok=false".to_string(),
            ));
        }
        snapshot = snapshot_from_move_response(&snapshot, &step);
        if is_finished(&snapshot) {
            break;
        }
    }

    Ok(build_round_summary(
        runtime.email(),
        &snapshot,
        continued,
        &progress,
        started,
        planned_steps,
        String::new(),
    ))
}

fn move_once(
    cancel_flag: &ui::CancelFlag,
    state: &Arc<Mutex<BatchState>>,
    runtime: &mut AccountRuntime,
    session_id: i32,
    direction: &str,
    step_number: i32,
) -> io::Result<Puzzle15MoveResponse> {
    let operation = retry_operation_with_step("puzzle15 move", step_number);
    with_auth_retry_api_until_success(
        cancel_flag,
        state,
        runtime,
        &operation,
        |client, auth_token| client.move_puzzle_15(auth_token, session_id, direction),
    )
}

pub(super) fn snapshot_from_start_response(start: &Puzzle15StartResponse) -> Puzzle15Snapshot {
    Puzzle15Snapshot {
        board: start.board.clone(),
        difficulty: start.difficulty.clone(),
        session_id: start.session_id,
        size: start.size,
        scramble: start.scramble,
        status: "pending".to_string(),
        ..Puzzle15Snapshot::default()
    }
}

pub(super) fn snapshot_from_history_item(item: &Puzzle15Session) -> Puzzle15Snapshot {
    Puzzle15Snapshot {
        board: item.board.clone(),
        difficulty: item.difficulty.clone(),
        session_id: item.session_id,
        size: item.size,
        move_count: item.move_count,
        scramble: item.scramble,
        status: item.status.clone(),
        won: item.won,
        reward_amount: item.reward_amount,
    }
}

fn snapshot_from_move_response(
    previous: &Puzzle15Snapshot,
    response: &Puzzle15MoveResponse,
) -> Puzzle15Snapshot {
    let session = &response.session;
    let has_session = session.session_id > 0;
    Puzzle15Snapshot {
        board: if has_session && !session.board.is_empty() {
            session.board.clone()
        } else if !response.board.is_empty() {
            response.board.clone()
        } else {
            previous.board.clone()
        },
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
        size: prefer_i32(session.size, previous.size),
        move_count: prefer_i32(
            response.move_count,
            prefer_i32(session.move_count, previous.move_count),
        ),
        scramble: prefer_i32(session.scramble, previous.scramble),
        status: first_non_empty(&[
            response.status.as_str(),
            session.status.as_str(),
            response.resolution.as_str(),
            previous.status.as_str(),
        ]),
        won: response.won || session.won,
        reward_amount: prefer_f64(response.reward_amount, session.reward_amount),
    }
}

pub(super) fn merge_round_into_summary(
    summary: &mut Puzzle15DifficultySummary,
    result: &Puzzle15RoundSummary,
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
    if result.status.trim().eq_ignore_ascii_case("won") {
        summary.won += 1;
    } else if !is_pending_round_status(&result.status) {
        summary.failed += 1;
    }
}

fn build_round_summary(
    email: &str,
    snapshot: &Puzzle15Snapshot,
    continued: bool,
    progress: &RoundProgress,
    started: Instant,
    planned_steps: i32,
    error_message: String,
) -> Puzzle15RoundSummary {
    Puzzle15RoundSummary {
        email: email.to_string(),
        difficulty: snapshot.difficulty.clone(),
        round_index: progress.current,
        round_total: progress.total,
        session_id: snapshot.session_id,
        continued,
        status: status_for_snapshot(snapshot),
        reward: snapshot.reward_amount,
        remaining_after: 0,
        move_count: snapshot.move_count,
        planned_steps,
        size: snapshot.size,
        duration_ms: started.elapsed().as_millis() as i64,
        when_unix_ms: current_unix_ms(),
        error_message,
    }
}

fn status_for_snapshot(snapshot: &Puzzle15Snapshot) -> String {
    if snapshot.won {
        "won".to_string()
    } else if snapshot.status.trim().is_empty() {
        "pending".to_string()
    } else {
        snapshot.status.clone()
    }
}

fn is_finished(snapshot: &Puzzle15Snapshot) -> bool {
    snapshot.won
        || matches!(
            snapshot.status.trim().to_ascii_lowercase().as_str(),
            "won" | "lost" | "failed" | "game_over" | "abandoned"
        )
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
    use crate::model::Puzzle15DifficultyConfig;

    #[test]
    fn pending_session_requires_active_status_and_board() {
        let session = Puzzle15Session {
            session_id: 1,
            board: vec![1, 2, 3, 4, 5, 6, 7, 0, 8],
            status: "pending".to_string(),
            ..Puzzle15Session::default()
        };
        let ended = Puzzle15Session {
            status: "won".to_string(),
            won: true,
            ..session.clone()
        };

        assert!(is_pending_session(&session));
        assert!(!is_pending_session(&ended));
    }

    #[test]
    fn difficulty_order_keeps_known_order_then_sorted_extras() {
        let mut config = Puzzle15ConfigResponse::default();
        config
            .difficulties
            .insert("zzz".to_string(), Puzzle15DifficultyConfig::default());
        config.difficulties.insert(
            crate::model::PUZZLE_15_DIFFICULTY_HARD.to_string(),
            Puzzle15DifficultyConfig::default(),
        );
        config.difficulties.insert(
            crate::model::PUZZLE_15_DIFFICULTY_EASY.to_string(),
            Puzzle15DifficultyConfig::default(),
        );

        assert_eq!(difficulty_order(&config), vec!["easy", "hard", "zzz"]);
    }

    #[test]
    fn lost_status_is_terminal_and_counted_failed() {
        let snapshot = Puzzle15Snapshot {
            status: "lost".to_string(),
            ..Puzzle15Snapshot::default()
        };
        let mut summary = Puzzle15DifficultySummary::default();
        let result = Puzzle15RoundSummary {
            status: status_for_snapshot(&snapshot),
            ..Puzzle15RoundSummary::default()
        };

        assert!(is_finished(&snapshot));
        merge_round_into_summary(&mut summary, &result);
        assert_eq!(summary.won, 0);
        assert_eq!(summary.failed, 1);
    }

    #[test]
    fn pending_status_is_ignored_not_failed() {
        let mut summary = Puzzle15DifficultySummary::default();
        let result = Puzzle15RoundSummary {
            status: "pending".to_string(),
            ..Puzzle15RoundSummary::default()
        };

        merge_round_into_summary(&mut summary, &result);

        assert_eq!(summary.won, 0);
        assert_eq!(summary.failed, 0);
    }
}
