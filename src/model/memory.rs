use std::collections::HashMap;

use serde::{Deserialize, Serialize};

pub const MEMORY_DIFFICULTY_EASY: &str = "easy";
pub const MEMORY_DIFFICULTY_NORMAL: &str = "normal";
pub const MEMORY_DIFFICULTY_HARD: &str = "hard";
pub const MEMORY_DIFFICULTY_HELL: &str = "hell";
pub const MEMORY_DIFFICULTY_ORDER: &[&str] = &[
    MEMORY_DIFFICULTY_EASY,
    MEMORY_DIFFICULTY_NORMAL,
    MEMORY_DIFFICULTY_HARD,
    MEMORY_DIFFICULTY_HELL,
];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct MemoryDifficultyConfig {
    #[serde(default)]
    pub cols: i32,
    #[serde(default)]
    pub daily_plays: i32,
    #[serde(default)]
    pub pairs: i32,
    #[serde(default)]
    pub peek_limit: i32,
    #[serde(default)]
    pub reward_amount: f64,
    #[serde(default)]
    pub rows: i32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct MemoryConfigResponse {
    #[serde(default)]
    pub difficulties: HashMap<String, MemoryDifficultyConfig>,
    #[serde(default)]
    pub max_active_sessions: i32,
    #[serde(default)]
    pub min_interval_ms: i32,
    #[serde(default)]
    pub symbols: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct MemoryCard {
    #[serde(default)]
    pub index: i32,
    #[serde(default)]
    pub symbol: i32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct MemorySession {
    #[serde(default)]
    pub cols: i32,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub currently_revealed: Vec<MemoryCard>,
    #[serde(default)]
    pub difficulty: String,
    #[serde(default)]
    pub ended_at_ms: Option<i64>,
    #[serde(default)]
    pub game_over: bool,
    #[serde(default)]
    pub match_count: i32,
    #[serde(default)]
    pub matched_indices: Vec<i32>,
    #[serde(default)]
    pub matched_symbols: Vec<i32>,
    #[serde(default)]
    pub pairs: i32,
    #[serde(default)]
    pub peek_count: i32,
    #[serde(default)]
    pub peek_limit: i32,
    #[serde(default)]
    pub reward_amount: f64,
    #[serde(default)]
    pub rows: i32,
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
    pub won: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct MemoryHistoryResponse {
    #[serde(default)]
    pub items: Vec<MemorySession>,
    #[serde(default)]
    pub server_now_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct MemoryMeResponse {
    #[serde(default)]
    pub active_session: Option<MemorySession>,
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
pub struct MemoryStartRequest {
    pub difficulty: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct MemoryStartResponse {
    #[serde(default)]
    pub cols: i32,
    #[serde(default)]
    pub difficulty: String,
    #[serde(default)]
    pub ok: bool,
    #[serde(default)]
    pub pairs: i32,
    #[serde(default)]
    pub peek_limit: i32,
    #[serde(default)]
    pub reward_amount_if_won: f64,
    #[serde(default)]
    pub rows: i32,
    #[serde(default)]
    pub server_now_ms: i64,
    #[serde(default)]
    pub server_seed_hash: String,
    #[serde(default)]
    pub session_id: i32,
    #[serde(default)]
    pub started_at_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct MemoryFlipRequest {
    pub session_id: i32,
    pub index: i32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct MemoryFlipResponse {
    #[serde(default)]
    pub cleared_last_mismatch: bool,
    #[serde(default)]
    pub currently_revealed: Vec<MemoryCard>,
    #[serde(default)]
    pub game_over: bool,
    #[serde(default)]
    pub index: i32,
    #[serde(default)]
    pub match_count: i32,
    #[serde(default)]
    pub matched_indices: Vec<i32>,
    #[serde(default)]
    pub matched_symbols: Vec<i32>,
    #[serde(default)]
    pub ok: bool,
    #[serde(default)]
    pub other: Option<MemoryCard>,
    #[serde(default)]
    pub peek_count: i32,
    #[serde(default)]
    pub resolution: String,
    #[serde(default)]
    pub reward_amount: f64,
    #[serde(default)]
    pub server_now_ms: i64,
    #[serde(default)]
    pub session: MemorySession,
    #[serde(default)]
    pub state: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub symbol: i32,
    #[serde(default)]
    pub won: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_response_accepts_api_payload() {
        let response: MemoryConfigResponse = serde_json::from_str(
            r#"{"difficulties":{"easy":{"cols":4,"daily_plays":5,"pairs":6,"peek_limit":24,"reward_amount":0.1,"rows":3}},"max_active_sessions":1,"min_interval_ms":80,"symbols":["🍎","🍉"]}"#,
        )
        .unwrap();

        assert_eq!(response.difficulties[MEMORY_DIFFICULTY_EASY].pairs, 6);
        assert_eq!(response.symbols.len(), 2);
    }

    #[test]
    fn start_flip_and_history_responses_accept_api_payloads() {
        let start: MemoryStartResponse = serde_json::from_str(
            r#"{"cols":4,"difficulty":"easy","ok":true,"pairs":6,"peek_limit":24,"reward_amount_if_won":0.1,"rows":3,"server_now_ms":1777478683597,"server_seed_hash":"hash","session_id":108467,"started_at_ms":1777478683597}"#,
        )
        .unwrap();
        let flip: MemoryFlipResponse = serde_json::from_str(
            r#"{"cleared_last_mismatch":false,"currently_revealed":[{"index":0,"symbol":3}],"game_over":false,"index":0,"match_count":0,"matched_indices":[],"matched_symbols":[],"ok":true,"other":null,"peek_count":1,"resolution":"pending","server_now_ms":1777478718042,"session":{"cols":4,"created_at":"2026-04-30T00:04:43.597664+08:00","currently_revealed":[{"index":0,"symbol":3}],"difficulty":"easy","ended_at_ms":null,"game_over":false,"match_count":0,"matched_indices":[],"matched_symbols":[],"pairs":6,"peek_count":1,"peek_limit":24,"reward_amount":0,"rows":3,"schema_version":1,"server_seed_hash":"hash","session_id":108467,"started_at_ms":1777478683597,"status":"pending","won":false},"state":"first_revealed","symbol":3,"won":false}"#,
        )
        .unwrap();
        let history: MemoryHistoryResponse = serde_json::from_str(
            r#"{"items":[{"cols":4,"currently_revealed":[{"index":2,"symbol":2},{"index":3,"symbol":0}],"difficulty":"easy","game_over":false,"match_count":1,"matched_indices":[0,1],"matched_symbols":[3],"pairs":6,"peek_count":4,"peek_limit":24,"reward_amount":0,"rows":3,"session_id":108467,"started_at_ms":1777478683597,"status":"pending","won":false}],"server_now_ms":1777478890452}"#,
        )
        .unwrap();

        assert!(start.ok);
        assert_eq!(flip.currently_revealed[0].symbol, 3);
        assert_eq!(history.items[0].matched_indices, vec![0, 1]);
    }
}
