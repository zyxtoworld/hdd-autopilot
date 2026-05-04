use std::collections::HashMap;

use serde::{Deserialize, Serialize};

pub const MINESWEEPER_DIFFICULTY_BEGINNER: &str = "beginner";
pub const MINESWEEPER_DIFFICULTY_INTERMEDIATE: &str = "intermediate";
pub const MINESWEEPER_DIFFICULTY_EXPERT: &str = "expert";
pub const MINESWEEPER_DIFFICULTY_ORDER: &[&str] = &[
    MINESWEEPER_DIFFICULTY_EXPERT,
    MINESWEEPER_DIFFICULTY_INTERMEDIATE,
    MINESWEEPER_DIFFICULTY_BEGINNER,
];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct MinesweeperDifficultyConfig {
    #[serde(default)]
    pub cols: i32,
    #[serde(default)]
    pub mines: i32,
    #[serde(default)]
    pub rows: i32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct MinesweeperConfigResponse {
    #[serde(default)]
    pub actions: Vec<String>,
    #[serde(default)]
    pub difficulties: HashMap<String, MinesweeperDifficultyConfig>,
    #[serde(default)]
    pub max_plays_per_day: i32,
    #[serde(default)]
    pub min_interval_ms: i32,
    #[serde(default)]
    pub minesweeper_hmac_prefix: String,
    #[serde(default)]
    pub rewards: HashMap<String, f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct MinesweeperUser {
    #[serde(default)]
    pub balance: f64,
    #[serde(default)]
    pub email: String,
    #[serde(default)]
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct MinesweeperSession {
    #[serde(default)]
    pub cols: i32,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub difficulty: String,
    #[serde(default)]
    pub ended_at_ms: Option<i64>,
    #[serde(default)]
    pub first_click: Option<[i32; 2]>,
    #[serde(default)]
    pub flagged: Vec<Vec<bool>>,
    #[serde(default)]
    pub mine_count: i32,
    #[serde(default)]
    pub mines: Vec<[i32; 2]>,
    #[serde(default)]
    pub play_id: i32,
    #[serde(default)]
    pub resolution: String,
    #[serde(default)]
    pub revealed: Vec<Vec<bool>>,
    #[serde(default)]
    pub reward_amount: f64,
    #[serde(default)]
    pub rows: i32,
    #[serde(default)]
    pub server_seed_hash: String,
    #[serde(default)]
    pub started_at_ms: i64,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub trace_count: i32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct MinesweeperHistoryResponse {
    #[serde(default)]
    pub items: Vec<MinesweeperSession>,
    #[serde(default)]
    pub server_now_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct MinesweeperMeResponse {
    #[serde(default)]
    pub active_round: Option<MinesweeperSession>,
    #[serde(default)]
    pub daily_plays_remaining: HashMap<String, i32>,
    #[serde(default)]
    pub daily_plays_used: HashMap<String, i32>,
    #[serde(default)]
    pub ok: bool,
    #[serde(default)]
    pub server_now_ms: i64,
    #[serde(default)]
    pub user: MinesweeperUser,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct MinesweeperStartRequest {
    pub difficulty: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct MinesweeperStartResponse {
    #[serde(flatten)]
    pub session: MinesweeperSession,
    #[serde(default)]
    pub daily_plays_remaining: HashMap<String, i32>,
    #[serde(default)]
    pub ok: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MinesweeperClickRequest {
    pub play_id: i32,
    pub action: String,
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct MinesweeperClickDelta {
    #[serde(default)]
    pub first_click: Option<[i32; 2]>,
    #[serde(default)]
    pub flagged_cells: Vec<serde_json::Value>,
    #[serde(default)]
    pub hit_mine: bool,
    #[serde(default)]
    pub lost: bool,
    #[serde(default)]
    pub revealed_cells: Vec<[i32; 3]>,
    #[serde(default)]
    pub won: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct MinesweeperClickResponse {
    #[serde(flatten)]
    pub session: MinesweeperSession,
    #[serde(default)]
    pub delta: MinesweeperClickDelta,
    #[serde(default)]
    pub ok: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_response_accepts_api_payload() {
        let response: MinesweeperConfigResponse = serde_json::from_str(
            r#"{
                "actions":["reveal","flag","unflag","chord"],
                "difficulties":{"beginner":{"cols":8,"mines":10,"rows":8}},
                "max_plays_per_day":100,
                "min_interval_ms":50,
                "minesweeper_hmac_prefix":"minesweeper:v1:",
                "rewards":{"beginner":0.5}
            }"#,
        )
        .unwrap();

        assert_eq!(response.difficulties["beginner"].cols, 8);
        assert_eq!(response.rewards["beginner"], 0.5);
    }

    #[test]
    fn start_click_and_me_responses_accept_api_payloads() {
        let start: MinesweeperStartResponse = serde_json::from_str(
            r#"{"cols":8,"difficulty":"beginner","flagged":[[false]],"mine_count":10,"ok":true,"play_id":7,"revealed":[[false]],"rows":8,"status":"pending"}"#,
        )
        .unwrap();
        let click: MinesweeperClickResponse = serde_json::from_str(
            r#"{"cols":8,"delta":{"flagged_cells":[[0,2,true]],"hit_mine":false,"lost":false,"revealed_cells":[[0,0,0]],"won":false},"difficulty":"beginner","flagged":[[false]],"mine_count":10,"ok":true,"play_id":7,"revealed":[[true]],"rows":8,"status":"pending"}"#,
        )
        .unwrap();
        let me: MinesweeperMeResponse = serde_json::from_str(
            r#"{"active_round":{"cols":8,"difficulty":"beginner","flagged":[[false]],"mine_count":10,"play_id":7,"revealed":[[true]],"rows":8,"status":"pending"},"ok":true,"server_now_ms":1,"user":{"balance":12.3,"email":"a@example.com","status":"active"}}"#,
        )
        .unwrap();

        assert!(start.ok);
        assert_eq!(click.delta.revealed_cells[0], [0, 0, 0]);
        assert_eq!(me.active_round.unwrap().play_id, 7);
    }
}
