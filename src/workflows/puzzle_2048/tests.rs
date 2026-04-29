use crate::model::{
    Puzzle2048ConfigResponse, Puzzle2048DifficultyConfig, Puzzle2048HistoryItem,
    Puzzle2048HistoryResponse,
};

use super::round;

#[test]
fn detects_pending_sessions_from_history() {
    let item = Puzzle2048HistoryItem {
        session_id: 42,
        board: vec![vec![2, 0, 0], vec![0, 0, 0], vec![0, 0, 0]],
        status: "active".to_string(),
        ..Puzzle2048HistoryItem::default()
    };
    let game_over = Puzzle2048HistoryItem {
        game_over: true,
        ..item.clone()
    };

    assert!(round::is_pending_item(&item));
    assert!(!round::is_pending_item(&game_over));
}

#[test]
fn counts_today_played_rounds_by_difficulty() {
    let history = Puzzle2048HistoryResponse {
        server_now_ms: 86_400_000 * 20 + 1_000,
        items: vec![
            Puzzle2048HistoryItem {
                difficulty: "mini".to_string(),
                started_at_ms: 86_400_000 * 20 + 500,
                ..Puzzle2048HistoryItem::default()
            },
            Puzzle2048HistoryItem {
                difficulty: "classic".to_string(),
                started_at_ms: 86_400_000 * 20 + 600,
                ..Puzzle2048HistoryItem::default()
            },
            Puzzle2048HistoryItem {
                difficulty: "mini".to_string(),
                started_at_ms: 86_400_000 * 19 + 500,
                ..Puzzle2048HistoryItem::default()
            },
        ],
    };
    let used = round::used_today_by_difficulty(&history);

    assert_eq!(used["mini"], 1);
    assert_eq!(used["classic"], 1);
}

#[test]
fn difficulty_order_uses_mini_classic_jumbo() {
    let mut config = Puzzle2048ConfigResponse::default();
    for difficulty in ["classic", "jumbo", "mini"] {
        config.difficulties.insert(
            difficulty.to_string(),
            Puzzle2048DifficultyConfig::default(),
        );
    }

    assert_eq!(
        round::difficulty_order(&config),
        vec!["mini", "classic", "jumbo"]
    );
}

#[test]
fn only_wedged_round_errors_stop_difficulty_loop() {
    assert!(super::should_stop_after_round_error("wedged"));
    assert!(super::should_stop_after_round_error("wedged: 请求失败"));
    assert!(!super::should_stop_after_round_error("deadlock"));
    assert!(!super::should_stop_after_round_error("max_steps"));
    assert!(!super::should_stop_after_round_error("sim_drift"));
    assert!(!super::should_stop_after_round_error(""));
}

#[test]
fn start_failed_error_breaks_current_difficulty() {
    assert!(super::is_start_failed_error(
        "start_failed: 谜题2048开局接口返回 ok=false"
    ));
    assert!(!super::is_start_failed_error("当前还有未结束对局"));
}
