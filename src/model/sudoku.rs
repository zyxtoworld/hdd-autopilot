use std::collections::HashMap;

use serde::{Deserialize, Serialize};

pub const SUDOKU_DIFFICULTY_EASY: &str = "easy";
pub const SUDOKU_DIFFICULTY_NORMAL: &str = "normal";
pub const SUDOKU_DIFFICULTY_HARD: &str = "hard";
pub const SUDOKU_DIFFICULTY_EXPERT: &str = "expert";
pub const SUDOKU_DIFFICULTY_ORDER: &[&str] = &[
    SUDOKU_DIFFICULTY_EASY,
    SUDOKU_DIFFICULTY_NORMAL,
    SUDOKU_DIFFICULTY_HARD,
    SUDOKU_DIFFICULTY_EXPERT,
];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct SudokuDifficultyConfig {
    #[serde(default)]
    pub daily_plays: i32,
    #[serde(default)]
    pub givens: i32,
    #[serde(default)]
    pub holes: i32,
    #[serde(default)]
    pub reward_amount: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct SudokuConfigResponse {
    #[serde(default)]
    pub box_size: i32,
    #[serde(default)]
    pub difficulties: HashMap<String, SudokuDifficultyConfig>,
    #[serde(default)]
    pub max_active_sessions: i32,
    #[serde(default)]
    pub min_interval_ms: i32,
    #[serde(default)]
    pub size: i32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct SudokuSession {
    #[serde(default)]
    pub conflicts: Vec<serde_json::Value>,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub difficulty: String,
    #[serde(default)]
    pub ended_at_ms: Option<i64>,
    #[serde(default)]
    pub givens: Vec<i32>,
    #[serde(default)]
    pub move_count: i32,
    #[serde(default)]
    pub reward_amount: f64,
    #[serde(default)]
    pub schema_version: i32,
    #[serde(default)]
    pub server_seed_hash: String,
    #[serde(default)]
    pub session_id: i32,
    #[serde(default)]
    pub started_at_ms: i64,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub user_board: Vec<i32>,
    #[serde(default)]
    pub won: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct SudokuHistoryResponse {
    #[serde(default)]
    pub items: Vec<SudokuSession>,
    #[serde(default)]
    pub server_now_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct SudokuMeResponse {
    #[serde(default)]
    pub active_session: Option<SudokuSession>,
    #[serde(default)]
    pub authenticated: bool,
    #[serde(default)]
    pub daily_plays_remaining: HashMap<String, i32>,
    #[serde(default)]
    pub daily_plays_used: HashMap<String, i32>,
    #[serde(default)]
    pub server_now_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct SudokuStartRequest {
    pub difficulty: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct SudokuStartResponse {
    #[serde(default)]
    pub conflicts: Vec<serde_json::Value>,
    #[serde(default)]
    pub daily_plays_remaining: HashMap<String, i32>,
    #[serde(default)]
    pub difficulty: String,
    #[serde(default)]
    pub givens: Vec<i32>,
    #[serde(default)]
    pub move_count: i32,
    #[serde(default)]
    pub ok: bool,
    #[serde(default)]
    pub reward_amount_if_won: f64,
    #[serde(default)]
    pub server_now_ms: i64,
    #[serde(default)]
    pub server_seed_hash: String,
    #[serde(default)]
    pub session_id: i32,
    #[serde(default)]
    pub started_at_ms: i64,
    #[serde(default)]
    pub user_board: Vec<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct SudokuFillRequest {
    pub session_id: i32,
    pub row: i32,
    pub col: i32,
    pub value: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct SudokuFillResponse {
    #[serde(default)]
    pub col: i32,
    #[serde(default)]
    pub complete: bool,
    #[serde(default)]
    pub conflicts: Vec<serde_json::Value>,
    #[serde(default)]
    pub move_count: i32,
    #[serde(default)]
    pub ok: bool,
    #[serde(default)]
    pub resolution: String,
    #[serde(default)]
    pub reward_amount: f64,
    #[serde(default)]
    pub row: i32,
    #[serde(default)]
    pub server_now_ms: i64,
    #[serde(default)]
    pub session: SudokuSession,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub user_board: Vec<i32>,
    #[serde(default)]
    pub value: Option<i32>,
    #[serde(default)]
    pub won: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_response_accepts_api_payload() {
        let response: SudokuConfigResponse = serde_json::from_str(
            r#"{
                "box_size": 3,
                "difficulties": {
                    "easy": {"daily_plays": 4, "givens": 45, "holes": 36, "reward_amount": 0.3},
                    "expert": {"daily_plays": 1, "givens": 23, "holes": 58, "reward_amount": 6}
                },
                "max_active_sessions": 1,
                "min_interval_ms": 30,
                "size": 9
            }"#,
        )
        .unwrap();

        assert_eq!(response.size, 9);
        assert_eq!(response.box_size, 3);
        assert_eq!(response.difficulties[SUDOKU_DIFFICULTY_EASY].holes, 36);
    }

    #[test]
    fn start_fill_and_history_responses_accept_api_payloads() {
        let start: SudokuStartResponse = serde_json::from_str(
            r#"{"conflicts":[],"difficulty":"easy","givens":[1,0,0,0],"move_count":0,"ok":true,"reward_amount_if_won":0.3,"server_now_ms":1,"server_seed_hash":"hash","session_id":7,"started_at_ms":1,"user_board":[1,0,0,0]}"#,
        )
        .unwrap();
        let fill: SudokuFillResponse = serde_json::from_str(
            r#"{"col":1,"complete":false,"conflicts":[],"move_count":1,"ok":true,"resolution":"pending","row":0,"server_now_ms":2,"session":{"conflicts":[],"difficulty":"easy","givens":[1,0,0,0],"move_count":1,"reward_amount":0,"session_id":7,"started_at_ms":1,"status":"pending","user_board":[1,2,0,0],"won":false},"user_board":[1,2,0,0],"value":2,"won":false}"#,
        )
        .unwrap();
        let history: SudokuHistoryResponse = serde_json::from_str(
            r#"{"items":[{"conflicts":[],"difficulty":"easy","givens":[1,0,0,0],"move_count":1,"reward_amount":0,"session_id":7,"started_at_ms":1,"status":"pending","user_board":[1,2,0,0],"won":false}],"server_now_ms":2}"#,
        )
        .unwrap();

        assert!(start.ok);
        assert_eq!(fill.session.user_board[1], 2);
        assert_eq!(history.items[0].session_id, 7);
    }
}
