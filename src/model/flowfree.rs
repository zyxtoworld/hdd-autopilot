use std::collections::HashMap;

use serde::{Deserialize, Serialize};

pub type FlowfreePoint = [i32; 2];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct FlowfreeEndpoint(pub i32, pub FlowfreePoint, pub FlowfreePoint);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct FlowfreePath(pub i32, pub Vec<FlowfreePoint>);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct FlowfreeDifficultyConfig {
    #[serde(default)]
    pub color_count: i32,
    #[serde(default)]
    pub daily_plays: i32,
    #[serde(default)]
    pub height: i32,
    #[serde(default)]
    pub reward_amount: f64,
    #[serde(default)]
    pub width: i32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct FlowfreeConfigResponse {
    #[serde(default)]
    pub actions: Vec<String>,
    #[serde(default)]
    pub difficulties: HashMap<String, FlowfreeDifficultyConfig>,
    #[serde(default)]
    pub max_active_sessions: i32,
    #[serde(default)]
    pub max_moves: i32,
    #[serde(default)]
    pub min_interval_ms: i32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct FlowfreeUser {
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
pub struct FlowfreeSession {
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
    pub endpoints: Vec<FlowfreeEndpoint>,
    #[serde(default)]
    pub height: i32,
    #[serde(default)]
    pub level_index: i32,
    #[serde(default)]
    pub paths: Vec<FlowfreePath>,
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
    pub status: String,
    #[serde(default)]
    pub width: i32,
    #[serde(default)]
    pub won: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct FlowfreeMeResponse {
    #[serde(default)]
    pub active_session: Option<FlowfreeSession>,
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
    pub user: FlowfreeUser,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct FlowfreeStartRequest {
    pub difficulty: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct FlowfreeStartResponse {
    #[serde(default)]
    pub daily_plays_remaining: HashMap<String, i32>,
    #[serde(default)]
    pub ok: bool,
    #[serde(default)]
    pub server_now_ms: i64,
    #[serde(default)]
    pub session: FlowfreeSession,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct FlowfreeClickRequest {
    pub session_id: i32,
    pub action: String,
    pub color: i32,
    pub r: i32,
    pub c: i32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct FlowfreeClickResponse {
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
    pub session: FlowfreeSession,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub won: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct FlowfreeHistoryResponse {
    #[serde(default)]
    pub items: Vec<FlowfreeSession>,
    #[serde(default)]
    pub server_now_ms: i64,
}
