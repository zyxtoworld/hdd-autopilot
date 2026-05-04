use std::collections::HashMap;

use serde::{Deserialize, Serialize};

pub type SokobanPoint = [i32; 2];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct SokobanDifficultyConfig {
    #[serde(default)]
    pub daily_plays: i32,
    #[serde(default)]
    pub height: i32,
    #[serde(default)]
    pub level_count: i32,
    #[serde(default)]
    pub reward_amount: f64,
    #[serde(default)]
    pub width: i32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct SokobanConfigResponse {
    #[serde(default)]
    pub difficulties: HashMap<String, SokobanDifficultyConfig>,
    #[serde(default)]
    pub directions: Vec<String>,
    #[serde(default)]
    pub max_active_sessions: i32,
    #[serde(default)]
    pub max_moves: i32,
    #[serde(default)]
    pub min_interval_ms: i32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct SokobanUser {
    #[serde(default)]
    pub balance: f64,
    #[serde(default)]
    pub email: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub user_id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct SokobanSession {
    #[serde(default)]
    pub boxes: Vec<SokobanPoint>,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub difficulty: String,
    #[serde(default)]
    pub ended_at_ms: Option<i64>,
    #[serde(default)]
    pub height: i32,
    #[serde(default)]
    pub level_index: i32,
    #[serde(default)]
    pub move_count: i32,
    #[serde(default)]
    pub player: SokobanPoint,
    #[serde(default)]
    pub push_count: i32,
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
    pub started_at_ms: i64,
    #[serde(default)]
    pub starting_boxes: Vec<SokobanPoint>,
    #[serde(default)]
    pub starting_player: SokobanPoint,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub targets: Vec<SokobanPoint>,
    #[serde(default)]
    pub walls: Vec<SokobanPoint>,
    #[serde(default)]
    pub width: i32,
    #[serde(default)]
    pub won: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct SokobanMeResponse {
    #[serde(default)]
    pub active_session: Option<SokobanSession>,
    #[serde(default)]
    pub authenticated: bool,
    #[serde(default)]
    pub daily_plays_remaining: HashMap<String, i32>,
    #[serde(default)]
    pub daily_plays_used: HashMap<String, i32>,
    #[serde(default)]
    pub ok: bool,
    #[serde(default)]
    pub server_now_ms: i64,
    #[serde(default)]
    pub user: SokobanUser,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct SokobanStartRequest {
    pub difficulty: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct SokobanStartResponse {
    #[serde(default)]
    pub daily_plays_remaining: HashMap<String, i32>,
    #[serde(default)]
    pub ok: bool,
    #[serde(default)]
    pub server_now_ms: i64,
    #[serde(default)]
    pub session: SokobanSession,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct SokobanMoveRequest {
    pub session_id: i32,
    pub direction: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct SokobanMoveResponse {
    #[serde(default)]
    pub balance: f64,
    #[serde(default)]
    pub changed: bool,
    #[serde(default)]
    pub ok: bool,
    #[serde(default)]
    pub resolution: String,
    #[serde(default)]
    pub reward_amount: f64,
    #[serde(default)]
    pub server_now_ms: i64,
    #[serde(default)]
    pub session: SokobanSession,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub won: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct SokobanHistoryResponse {
    #[serde(default)]
    pub items: Vec<SokobanSession>,
    #[serde(default)]
    pub server_now_ms: i64,
}
