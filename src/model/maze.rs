use std::collections::HashMap;

use serde::{Deserialize, Serialize};

pub type MazePoint = [i32; 2];
pub type MazeEdge = [MazePoint; 2];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct MazeDifficultyConfig {
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
pub struct MazeConfigResponse {
    #[serde(default)]
    pub difficulties: HashMap<String, MazeDifficultyConfig>,
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
pub struct MazeUser {
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
pub struct MazeSession {
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub difficulty: String,
    #[serde(default)]
    pub ended_at_ms: Option<i64>,
    #[serde(default)]
    pub exit: MazePoint,
    #[serde(default)]
    pub height: i32,
    #[serde(default)]
    pub level_index: i32,
    #[serde(default)]
    pub move_count: i32,
    #[serde(default)]
    pub open_edges: Vec<MazeEdge>,
    #[serde(default)]
    pub player: MazePoint,
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
    pub starting_player: MazePoint,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub width: i32,
    #[serde(default)]
    pub won: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct MazeMeResponse {
    #[serde(default)]
    pub active_session: Option<MazeSession>,
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
    pub user: MazeUser,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct MazeStartRequest {
    pub difficulty: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct MazeStartResponse {
    #[serde(default)]
    pub daily_plays_remaining: HashMap<String, i32>,
    #[serde(default)]
    pub ok: bool,
    #[serde(default)]
    pub server_now_ms: i64,
    #[serde(default)]
    pub session: MazeSession,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct MazeMoveRequest {
    pub session_id: i32,
    pub direction: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct MazeMoveResponse {
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
    pub session: MazeSession,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub won: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct MazeHistoryResponse {
    #[serde(default)]
    pub items: Vec<MazeSession>,
    #[serde(default)]
    pub server_now_ms: i64,
}
