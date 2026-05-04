use crate::model::{Puzzle2048ConfigResponse, Puzzle2048DifficultyConfig, Puzzle2048HistoryItem};

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
fn new_round_unavailable_is_interface_state_not_game_result() {
    assert!(super::is_new_round_unavailable_error(
        "new_round_unavailable: 谜题2048接口没有返回可玩的新局"
    ));
    assert!(!super::is_new_round_unavailable_error("当前还有未结束对局"));
}
