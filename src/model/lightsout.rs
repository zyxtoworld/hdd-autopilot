use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct LightsoutDifficultyConfig {
    #[serde(default)]
    pub daily_plays: i32,
    #[serde(default)]
    pub reward_amount: f64,
    #[serde(default)]
    pub size: i32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct LightsoutConfigResponse {
    #[serde(default)]
    pub actions: Vec<String>,
    #[serde(default)]
    pub cell_states: HashMap<String, i32>,
    #[serde(default)]
    pub difficulties: HashMap<String, LightsoutDifficultyConfig>,
    #[serde(default)]
    pub max_active_sessions: i32,
    #[serde(default)]
    pub max_moves: i32,
    #[serde(default)]
    pub min_interval_ms: i32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct LightsoutUser {
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
pub struct LightsoutSession {
    #[serde(default)]
    pub cells: Vec<Vec<i32>>,
    #[serde(default)]
    pub click_count: i32,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub difficulty: String,
    #[serde(default)]
    pub ended_at_ms: Option<i64>,
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
    pub starting_cells: Vec<Vec<i32>>,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub won: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct LightsoutMeResponse {
    #[serde(default)]
    pub active_session: Option<LightsoutSession>,
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
    pub user: LightsoutUser,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct LightsoutStartRequest {
    pub difficulty: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct LightsoutStartResponse {
    #[serde(default)]
    pub daily_plays_remaining: HashMap<String, i32>,
    #[serde(default)]
    pub ok: bool,
    #[serde(default)]
    pub server_now_ms: i64,
    #[serde(default)]
    pub session: LightsoutSession,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct LightsoutClickRequest {
    pub session_id: i32,
    pub r: i32,
    pub c: i32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct LightsoutClickResponse {
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
    pub session: LightsoutSession,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub won: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct LightsoutHistoryResponse {
    #[serde(default)]
    pub items: Vec<LightsoutSession>,
    #[serde(default)]
    pub server_now_ms: i64,
}
