use std::collections::HashMap;

use serde::{Deserialize, Serialize};

pub const PUZZLE_2048_DIFFICULTY_MINI: &str = "mini";
pub const PUZZLE_2048_DIFFICULTY_CLASSIC: &str = "classic";
pub const PUZZLE_2048_DIFFICULTY_JUMBO: &str = "jumbo";
pub const PUZZLE_2048_DIFFICULTY_ORDER: &[&str] = &[
    PUZZLE_2048_DIFFICULTY_MINI,
    PUZZLE_2048_DIFFICULTY_CLASSIC,
    PUZZLE_2048_DIFFICULTY_JUMBO,
];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Puzzle2048DifficultyConfig {
    #[serde(default)]
    pub daily_plays: i32,
    #[serde(default)]
    pub reward_amount: f64,
    #[serde(default)]
    pub size: i32,
    #[serde(default)]
    pub target_tile: i32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Puzzle2048ConfigResponse {
    #[serde(default)]
    pub difficulties: HashMap<String, Puzzle2048DifficultyConfig>,
    #[serde(default)]
    pub directions: Vec<String>,
    #[serde(default)]
    pub four_ratio: f64,
    #[serde(default)]
    pub max_active_sessions: i32,
    #[serde(default)]
    pub min_interval_ms: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Puzzle2048SpawnedTile {
    #[serde(default)]
    pub c: i32,
    #[serde(default)]
    pub r: i32,
    #[serde(default)]
    pub v: i32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Puzzle2048HistoryItem {
    #[serde(default)]
    pub board: Vec<Vec<i32>>,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub difficulty: String,
    #[serde(default)]
    pub ended_at_ms: Option<i64>,
    #[serde(default)]
    pub game_over: bool,
    #[serde(default)]
    pub last_spawn: Option<Puzzle2048SpawnedTile>,
    #[serde(default)]
    pub max_tile: i32,
    #[serde(default)]
    pub move_count: i32,
    #[serde(default)]
    pub reward_amount: f64,
    #[serde(default)]
    pub schema_version: i32,
    #[serde(default)]
    pub score: i32,
    #[serde(default)]
    pub server_seed: Option<String>,
    #[serde(default)]
    pub server_seed_hash: String,
    #[serde(default)]
    pub session_id: i32,
    #[serde(default)]
    pub size: i32,
    #[serde(default)]
    pub started_at_ms: i64,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub target_tile: i32,
    #[serde(default)]
    pub won: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Puzzle2048HistoryResponse {
    #[serde(default)]
    pub items: Vec<Puzzle2048HistoryItem>,
    #[serde(default)]
    pub server_now_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Puzzle2048MeResponse {
    #[serde(default)]
    pub active_session: Option<Puzzle2048HistoryItem>,
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
pub struct Puzzle2048StartRequest {
    pub difficulty: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Puzzle2048StartResponse {
    #[serde(default)]
    pub board: Vec<Vec<i32>>,
    #[serde(default)]
    pub daily_plays_remaining: HashMap<String, i32>,
    #[serde(default)]
    pub difficulty: String,
    #[serde(default)]
    pub max_tile: i32,
    #[serde(default)]
    pub move_count: i32,
    #[serde(default)]
    pub ok: bool,
    #[serde(default)]
    pub reward_amount_if_won: f64,
    #[serde(default)]
    pub score: i32,
    #[serde(default)]
    pub server_now_ms: i64,
    #[serde(default)]
    pub server_seed_hash: String,
    #[serde(default)]
    pub session_id: i32,
    #[serde(default)]
    pub size: i32,
    #[serde(default)]
    pub spawned_initial: Vec<Puzzle2048SpawnedTile>,
    #[serde(default)]
    pub started_at_ms: i64,
    #[serde(default)]
    pub target_tile: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Puzzle2048MoveRequest {
    pub session_id: i32,
    pub direction: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Puzzle2048AbandonRequest {
    pub session_id: i32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Puzzle2048MoveResponse {
    #[serde(default)]
    pub board: Vec<Vec<i32>>,
    #[serde(default)]
    pub changed: bool,
    #[serde(default)]
    pub direction: String,
    #[serde(default)]
    pub ended_at_ms: Option<i64>,
    #[serde(default)]
    pub game_over: bool,
    #[serde(default)]
    pub max_tile: i32,
    #[serde(default)]
    pub move_count: i32,
    #[serde(default)]
    pub ok: bool,
    #[serde(default)]
    pub resolution: String,
    #[serde(default)]
    pub reward_amount: f64,
    #[serde(default)]
    pub score: i32,
    #[serde(default)]
    pub score_delta: i32,
    #[serde(default)]
    pub server_now_ms: i64,
    #[serde(default)]
    pub server_seed: Option<String>,
    #[serde(default)]
    pub spawned: Option<Puzzle2048SpawnedTile>,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub won: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_response_accepts_api_payload() {
        let response: Puzzle2048ConfigResponse = serde_json::from_str(
            r#"{"difficulties":{"classic":{"daily_plays":3,"reward_amount":1.5,"size":4,"target_tile":2048},"jumbo":{"daily_plays":1,"reward_amount":14,"size":5,"target_tile":4096},"mini":{"daily_plays":5,"reward_amount":0.3,"size":3,"target_tile":512}},"directions":["up","down","left","right"],"four_ratio":0.1,"max_active_sessions":1,"min_interval_ms":40}"#,
        )
        .unwrap();

        assert_eq!(response.difficulties[PUZZLE_2048_DIFFICULTY_MINI].size, 3);
        assert_eq!(
            response.difficulties[PUZZLE_2048_DIFFICULTY_CLASSIC].target_tile,
            2048
        );
        assert_eq!(response.directions, ["up", "down", "left", "right"]);
    }

    #[test]
    fn history_response_accepts_nullable_and_sparse_fields() {
        let response: Puzzle2048HistoryResponse = serde_json::from_str(
            r#"{"items":[{"board":[[4,0,0],[8,0,0],[2,0,0]],"difficulty":"mini","ended_at_ms":1777131555397,"game_over":false,"last_spawn":{"c":0,"r":2,"v":2},"max_tile":8,"move_count":5,"reward_amount":0,"score":20,"server_seed":"seed","session_id":67693,"size":3,"started_at_ms":1777055690532,"status":"abandoned","target_tile":512,"won":false}],"server_now_ms":1777451545943}"#,
        )
        .unwrap();

        let item = &response.items[0];
        assert_eq!(item.board[1][0], 8);
        assert_eq!(item.last_spawn.as_ref().unwrap().v, 2);
        assert_eq!(item.ended_at_ms, Some(1777131555397));
    }

    #[test]
    fn start_and_move_responses_accept_api_payloads() {
        let start: Puzzle2048StartResponse = serde_json::from_str(
            r#"{"board":[[0,0,0],[0,2,0],[0,4,0]],"daily_plays_remaining":{"classic":3,"jumbo":1,"mini":4},"difficulty":"mini","max_tile":4,"move_count":0,"ok":true,"reward_amount_if_won":0.3,"score":0,"server_now_ms":1777451819728,"server_seed_hash":"hash","session_id":107492,"size":3,"spawned_initial":[{"c":1,"r":1,"v":2},{"c":1,"r":2,"v":4}],"started_at_ms":1777451819728,"target_tile":512}"#,
        )
        .unwrap();
        let step: Puzzle2048MoveResponse = serde_json::from_str(
            r#"{"board":[[0,0,0],[2,2,0],[4,0,0]],"changed":true,"direction":"left","ended_at_ms":null,"game_over":false,"max_tile":4,"move_count":1,"ok":true,"resolution":"pending","reward_amount":0,"score":0,"score_delta":0,"server_now_ms":1777452053489,"server_seed":null,"spawned":{"c":1,"r":1,"v":2},"status":"pending","won":false}"#,
        )
        .unwrap();

        assert_eq!(start.daily_plays_remaining[PUZZLE_2048_DIFFICULTY_MINI], 4);
        assert_eq!(step.spawned.as_ref().unwrap().v, 2);
        assert!(step.changed);
    }

    #[test]
    fn me_response_accepts_remaining_counts_and_active_session() {
        let response: Puzzle2048MeResponse = serde_json::from_str(
            r#"{"active_session":null,"authenticated":true,"daily_plays_remaining":{"classic":0,"jumbo":0,"mini":0},"daily_plays_used":{"classic":3,"jumbo":1,"mini":5},"server_now_ms":1777540011408,"user":{"balance":684.13747521,"email":"demo@example.com","id":889,"status":"active"}}"#,
        )
        .unwrap();

        assert!(response.authenticated);
        assert_eq!(
            response.daily_plays_remaining[PUZZLE_2048_DIFFICULTY_CLASSIC],
            0
        );
        assert_eq!(response.daily_plays_used[PUZZLE_2048_DIFFICULTY_MINI], 5);
    }
}
