use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::is_zero_i32;

pub const DIFFICULTY_EASY: &str = "easy";
pub const DIFFICULTY_NORMAL: &str = "normal";
pub const DIFFICULTY_HARD: &str = "hard";
pub const DIFFICULTY_HELL: &str = "hell";
pub const DIFFICULTY_ORDER: &[&str] = &[
    DIFFICULTY_EASY,
    DIFFICULTY_NORMAL,
    DIFFICULTY_HARD,
    DIFFICULTY_HELL,
];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct TileMeUser {
    #[serde(default)]
    pub balance: f64,
    #[serde(default)]
    pub email: String,
    #[serde(default)]
    pub id: i32,
    #[serde(default)]
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct TileMeResponse {
    #[serde(default)]
    pub active_session: Option<HistoryItem>,
    #[serde(default)]
    pub authenticated: bool,
    #[serde(default)]
    pub daily_plays_remaining: HashMap<String, i32>,
    #[serde(default)]
    pub daily_plays_used: HashMap<String, i32>,
    #[serde(default)]
    pub server_now_ms: i64,
    #[serde(default)]
    pub user: TileMeUser,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Tile {
    #[serde(default, skip_serializing_if = "is_zero_i32")]
    pub gx: i32,
    #[serde(default, skip_serializing_if = "is_zero_i32")]
    pub gy: i32,
    #[serde(default)]
    pub id: i32,
    #[serde(default, skip_serializing_if = "is_zero_i32")]
    pub layer: i32,
    #[serde(default)]
    pub pattern: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Powerups {
    #[serde(default)]
    pub remove: i32,
    #[serde(default)]
    pub shuffle: i32,
    #[serde(default)]
    pub undo: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct StartRequest {
    pub difficulty: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct HistoryEntry {
    #[serde(default)]
    pub action: String,
    #[serde(default, skip_serializing_if = "is_zero_i32")]
    pub move_count: i32,
    #[serde(default, skip_serializing_if = "is_zero_i32")]
    pub tile_id: i32,
    #[serde(default)]
    pub reversible: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub prev_slots: Vec<i32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub removed: Vec<i32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub returned_ids: Vec<i32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct StartResponse {
    #[serde(default)]
    pub daily_plays_remaining: HashMap<String, i32>,
    #[serde(default)]
    pub difficulty: String,
    #[serde(default)]
    pub history: Vec<HistoryEntry>,
    #[serde(default)]
    pub move_count: i32,
    #[serde(default)]
    pub ok: bool,
    #[serde(default)]
    pub pattern_count: i32,
    #[serde(default)]
    pub powerups: Powerups,
    #[serde(default)]
    pub server_now_ms: i64,
    #[serde(default)]
    pub server_seed_hash: String,
    #[serde(default)]
    pub session_id: i32,
    #[serde(default)]
    pub slot_limit: i32,
    #[serde(default)]
    pub slots: Vec<i32>,
    #[serde(default)]
    pub slot_tiles: Vec<Tile>,
    #[serde(default)]
    pub started_at_ms: i64,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub tiles: Vec<Tile>,
    #[serde(default)]
    pub total_tiles: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct StepRequest {
    pub session_id: i32,
    pub action: String,
    #[serde(default, skip_serializing_if = "is_zero_i32")]
    pub tile_id: i32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct StepResponse {
    #[serde(default)]
    pub action: String,
    #[serde(default)]
    pub balance: f64,
    #[serde(default)]
    pub ended_at_ms: Option<i64>,
    #[serde(default)]
    pub grant_ref: String,
    #[serde(default)]
    pub history: Vec<HistoryEntry>,
    #[serde(default)]
    pub move_count: i32,
    #[serde(default)]
    pub ok: bool,
    #[serde(default)]
    pub pattern_matched: Option<String>,
    #[serde(default)]
    pub powerups: Option<Powerups>,
    #[serde(default)]
    pub removed: Vec<i32>,
    #[serde(default)]
    pub reward_amount: f64,
    #[serde(default)]
    pub schema_version: i32,
    #[serde(default)]
    pub server_now_ms: i64,
    #[serde(default)]
    pub server_seed: Option<String>,
    #[serde(default)]
    pub session_id: i32,
    #[serde(default)]
    pub slot_limit: i32,
    #[serde(default)]
    pub slots: Option<Vec<i32>>,
    #[serde(default)]
    pub started_at_ms: i64,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub tiles: Option<Vec<Tile>>,
    #[serde(default)]
    pub total_tiles: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct AbandonRequest {
    pub session_id: i32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct AbandonResponse {
    #[serde(default)]
    pub balance: Option<f64>,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub difficulty: String,
    #[serde(default)]
    pub ended_at_ms: i64,
    #[serde(default)]
    pub history: Vec<HistoryEntry>,
    #[serde(default)]
    pub move_count: i32,
    #[serde(default)]
    pub ok: bool,
    #[serde(default)]
    pub pattern_count: i32,
    #[serde(default)]
    pub powerups: Powerups,
    #[serde(default)]
    pub reward_amount: f64,
    #[serde(default)]
    pub schema_version: i32,
    #[serde(default)]
    pub server_seed: String,
    #[serde(default)]
    pub server_seed_hash: String,
    #[serde(default)]
    pub session_id: i32,
    #[serde(default)]
    pub slot_limit: i32,
    #[serde(default)]
    pub slots: Vec<i32>,
    #[serde(default)]
    pub started_at_ms: i64,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub tiles: Vec<Tile>,
    #[serde(default)]
    pub total_tiles: i32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct GameDifficultyConfig {
    pub daily_plays: i32,
    pub layers: i32,
    pub patterns: i32,
    pub reward_max: f64,
    pub reward_min: f64,
    pub tiles: i32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ConfigResponse {
    #[serde(default)]
    pub actions: Vec<String>,
    #[serde(default)]
    pub difficulties: HashMap<String, GameDifficultyConfig>,
    #[serde(default)]
    pub max_active_sessions: i32,
    #[serde(default)]
    pub min_interval_ms: i32,
    #[serde(default)]
    pub powerups_default: Powerups,
    #[serde(default)]
    pub schema_version: i32,
    #[serde(default)]
    pub slot_limit: i32,
    #[serde(default)]
    pub tile_hmac_message: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct HistoryItem {
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub difficulty: String,
    #[serde(default)]
    pub ended_at_ms: Option<i64>,
    #[serde(default)]
    pub history: Vec<HistoryEntry>,
    #[serde(default)]
    pub move_count: i32,
    #[serde(default)]
    pub pattern_count: i32,
    #[serde(default)]
    pub powerups: Powerups,
    #[serde(default)]
    pub reward_amount: f64,
    #[serde(default)]
    pub schema_version: i32,
    #[serde(default)]
    pub server_seed_hash: String,
    #[serde(default)]
    pub session_id: i32,
    #[serde(default)]
    pub slot_limit: i32,
    #[serde(default)]
    pub slots: Vec<i32>,
    #[serde(default)]
    pub slot_tiles: Vec<Tile>,
    #[serde(default)]
    pub started_at_ms: i64,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub template_digest: Option<String>,
    #[serde(default)]
    pub template_id: Option<String>,
    #[serde(default)]
    pub tiles: Vec<Tile>,
    #[serde(default)]
    pub total_tiles: i32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct HistoryResponse {
    #[serde(default)]
    pub items: Vec<HistoryItem>,
    #[serde(default)]
    pub server_now_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct SessionSnapshot {
    pub difficulty: String,
    pub session_id: i32,
    pub slot_limit: i32,
    pub powerups: Powerups,
    pub status: String,
    pub tiles: Vec<Tile>,
    pub slot_tiles: Vec<Tile>,
    pub move_count: i32,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct AccountRunSummary {
    pub email: String,
    pub difficulty: String,
    pub played: i32,
    pub won: i32,
    pub abandoned: i32,
    pub failed: i32,
    pub total_reward: f64,
    pub balance_after: Option<f64>,
    pub remaining_after: i32,
    pub when_unix_ms: i64,
    pub error_message: String,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct RoundResultSummary {
    pub email: String,
    pub difficulty: String,
    pub round_index: i32,
    pub round_total: i32,
    pub session_id: i32,
    pub continued: bool,
    pub dry_run: bool,
    pub status: String,
    pub reward: f64,
    pub balance_after: Option<f64>,
    pub remaining_after: i32,
    pub move_count: i32,
    pub used_powerups: Vec<String>,
    pub duration_ms: i64,
    pub when_unix_ms: i64,
    pub error_message: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tile_step_response_accepts_missing_state_fields() {
        let response: StepResponse = serde_json::from_str(
            r#"{"action":"click","balance":12.34,"move_count":3,"ok":true,"reward_amount":0.5,"status":"won"}"#,
        )
        .unwrap();

        assert_eq!(response.action, "click");
        assert_eq!(response.balance, 12.34);
        assert_eq!(response.move_count, 3);
        assert!(response.powerups.is_none());
        assert!(response.slots.is_none());
        assert!(response.tiles.is_none());
    }

    #[test]
    fn start_response_accepts_sparse_payload() {
        let response: StartResponse = serde_json::from_str(
            r#"{"ok":true,"session_id":123,"tiles":[],"slot_tiles":[],"slots":[],"status":"","difficulty":""}"#,
        )
        .unwrap();

        assert_eq!(response.session_id, 123);
        assert_eq!(response.difficulty, "");
        assert_eq!(response.status, "");
    }
}
