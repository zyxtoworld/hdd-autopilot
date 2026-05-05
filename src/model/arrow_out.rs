use serde::{Deserialize, Serialize};

pub type ArrowOutPoint = [i32; 2];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ArrowOutArrow {
    #[serde(default)]
    pub body: Vec<ArrowOutPoint>,
    #[serde(default)]
    pub c: i32,
    #[serde(default)]
    pub dir: String,
    #[serde(default)]
    pub id: i32,
    #[serde(default)]
    pub r: i32,
    #[serde(default)]
    pub shape: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ArrowOutObstacle {
    #[serde(default)]
    pub c: i32,
    #[serde(default)]
    pub r: i32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ArrowOutConfigResponse {
    #[serde(default)]
    pub directions: Vec<String>,
    #[serde(default)]
    pub max_active_sessions: i32,
    #[serde(default)]
    pub max_clicks: i32,
    #[serde(default)]
    pub max_collisions: i32,
    #[serde(default)]
    pub min_interval_ms: i32,
    #[serde(default)]
    pub reward_per_clear: f64,
    #[serde(default)]
    pub schema_version: i32,
    #[serde(default)]
    pub shape_names: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ArrowOutUser {
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
pub struct ArrowOutNextStage {
    #[serde(default)]
    pub arrow_count: i32,
    #[serde(default)]
    pub height: i32,
    #[serde(default)]
    pub min_elapsed_ms: i32,
    #[serde(default)]
    pub stage: i32,
    #[serde(default)]
    pub width: i32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ArrowOutSession {
    #[serde(default)]
    pub arrows: Vec<ArrowOutArrow>,
    #[serde(default)]
    pub arrows_remaining: i32,
    #[serde(default)]
    pub collisions: i32,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub ended_at_ms: Option<i64>,
    #[serde(default)]
    pub height: i32,
    #[serde(default)]
    pub max_collisions: i32,
    #[serde(default)]
    pub min_elapsed_ms: i32,
    #[serde(default)]
    pub obstacles: Vec<ArrowOutObstacle>,
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
    pub stage: i32,
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
pub struct ArrowOutMeResponse {
    #[serde(default)]
    pub active_session: Option<ArrowOutSession>,
    #[serde(default)]
    pub authenticated: bool,
    #[serde(default)]
    pub clears_today: i32,
    #[serde(default)]
    pub next_stage: ArrowOutNextStage,
    #[serde(default)]
    pub ok: bool,
    #[serde(default)]
    pub reward_per_clear: f64,
    #[serde(default)]
    pub reward_today: f64,
    #[serde(default)]
    pub server_now_ms: i64,
    #[serde(default)]
    pub user: ArrowOutUser,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ArrowOutStartRequest {}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ArrowOutStartResponse {
    #[serde(default)]
    pub ok: bool,
    #[serde(default)]
    pub server_now_ms: i64,
    #[serde(default)]
    pub session: ArrowOutSession,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ArrowOutClick {
    pub arrow_id: i32,
    pub t_ms: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ArrowOutFinishRequest {
    pub clicks: Vec<ArrowOutClick>,
    pub result: String,
    pub session_id: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ArrowOutAbandonRequest {
    pub session_id: i32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ArrowOutFinishResponse {
    #[serde(default)]
    pub balance: f64,
    #[serde(default)]
    pub ok: bool,
    #[serde(default)]
    pub resolution: String,
    #[serde(default)]
    pub reward_amount: f64,
    #[serde(default)]
    pub server_now_ms: i64,
    #[serde(default)]
    pub session: ArrowOutSession,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub won: bool,
}

pub type ArrowOutAbandonResponse = ArrowOutFinishResponse;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ArrowOutHistoryResponse {
    #[serde(default)]
    pub items: Vec<ArrowOutSession>,
    #[serde(default)]
    pub server_now_ms: i64,
}
