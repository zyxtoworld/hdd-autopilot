use std::collections::HashMap;

use serde::{Deserialize, Serialize};

pub const PUZZLE_15_DIFFICULTY_EASY: &str = "easy";
pub const PUZZLE_15_DIFFICULTY_CLASSIC: &str = "classic";
pub const PUZZLE_15_DIFFICULTY_HARD: &str = "hard";
pub const PUZZLE_15_DIFFICULTY_ORDER: &[&str] = &[
    PUZZLE_15_DIFFICULTY_EASY,
    PUZZLE_15_DIFFICULTY_CLASSIC,
    PUZZLE_15_DIFFICULTY_HARD,
];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Puzzle15DifficultyConfig {
    #[serde(default)]
    pub daily_plays: i32,
    #[serde(default)]
    pub reward_amount: f64,
    #[serde(default)]
    pub scramble: i32,
    #[serde(default)]
    pub size: i32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Puzzle15ConfigResponse {
    #[serde(default)]
    pub difficulties: HashMap<String, Puzzle15DifficultyConfig>,
    #[serde(default)]
    pub directions: Vec<String>,
    #[serde(default)]
    pub max_active_sessions: i32,
    #[serde(default)]
    pub min_interval_ms: i32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Puzzle15Session {
    #[serde(default)]
    pub board: Vec<i32>,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub difficulty: String,
    #[serde(default)]
    pub ended_at_ms: Option<i64>,
    #[serde(default)]
    pub move_count: i32,
    #[serde(default)]
    pub reward_amount: f64,
    #[serde(default)]
    pub schema_version: i32,
    #[serde(default)]
    pub scramble: i32,
    #[serde(default)]
    pub server_seed_hash: String,
    #[serde(default)]
    pub session_id: i32,
    #[serde(default)]
    pub size: i32,
    #[serde(default)]
    pub started_at_ms: i64,
    #[serde(default)]
    pub starting_board: Vec<i32>,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub won: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Puzzle15HistoryResponse {
    #[serde(default)]
    pub items: Vec<Puzzle15Session>,
    #[serde(default)]
    pub server_now_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Puzzle15MeResponse {
    #[serde(default)]
    pub active_session: Option<Puzzle15Session>,
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
pub struct Puzzle15StartRequest {
    pub difficulty: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Puzzle15StartResponse {
    #[serde(default)]
    pub board: Vec<i32>,
    #[serde(default)]
    pub daily_plays_remaining: HashMap<String, i32>,
    #[serde(default)]
    pub difficulty: String,
    #[serde(default)]
    pub ok: bool,
    #[serde(default)]
    pub reward_amount_if_won: f64,
    #[serde(default)]
    pub scramble: i32,
    #[serde(default)]
    pub server_now_ms: i64,
    #[serde(default)]
    pub server_seed_hash: String,
    #[serde(default)]
    pub session_id: i32,
    #[serde(default)]
    pub size: i32,
    #[serde(default)]
    pub started_at_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Puzzle15MoveRequest {
    pub session_id: i32,
    pub direction: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Puzzle15MoveResponse {
    #[serde(default)]
    pub board: Vec<i32>,
    #[serde(default)]
    pub move_count: i32,
    #[serde(default)]
    pub moved_from: i32,
    #[serde(default)]
    pub moved_tile: i32,
    #[serde(default)]
    pub ok: bool,
    #[serde(default)]
    pub resolution: String,
    #[serde(default)]
    pub reward_amount: f64,
    #[serde(default)]
    pub server_now_ms: i64,
    #[serde(default)]
    pub session: Puzzle15Session,
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
        let response: Puzzle15ConfigResponse = serde_json::from_str(
            r#"{"difficulties":{"classic":{"daily_plays":3,"reward_amount":1.5,"scramble":60,"size":4},"easy":{"daily_plays":5,"reward_amount":0.3,"scramble":20,"size":3},"hard":{"daily_plays":2,"reward_amount":4,"scramble":120,"size":5}},"directions":["up","down","left","right"],"max_active_sessions":1,"min_interval_ms":50}"#,
        )
        .unwrap();

        assert_eq!(response.difficulties[PUZZLE_15_DIFFICULTY_EASY].size, 3);
        assert_eq!(
            response.difficulties[PUZZLE_15_DIFFICULTY_HARD].scramble,
            120
        );
        assert_eq!(response.directions, ["up", "down", "left", "right"]);
    }

    #[test]
    fn start_move_and_history_responses_accept_api_payloads() {
        let start: Puzzle15StartResponse = serde_json::from_str(
            r#"{"board":[8,7,1,5,0,2,4,6,3],"difficulty":"easy","ok":true,"reward_amount_if_won":0.3,"scramble":20,"server_now_ms":1777480554657,"server_seed_hash":"hash","session_id":108738,"size":3,"started_at_ms":1777480554657}"#,
        )
        .unwrap();
        let step: Puzzle15MoveResponse = serde_json::from_str(
            r#"{"board":[8,7,1,5,6,2,4,0,3],"move_count":1,"moved_from":7,"moved_tile":6,"ok":true,"resolution":"pending","server_now_ms":1777480625494,"session":{"board":[8,7,1,5,6,2,4,0,3],"created_at":"2026-04-30T00:35:54.657286+08:00","difficulty":"easy","ended_at_ms":null,"move_count":1,"reward_amount":0,"schema_version":1,"scramble":20,"server_seed_hash":"hash","session_id":108738,"size":3,"started_at_ms":1777480554657,"starting_board":[8,7,1,5,0,2,4,6,3],"status":"pending","won":false},"won":false}"#,
        )
        .unwrap();
        let history: Puzzle15HistoryResponse = serde_json::from_str(
            r#"{"items":[{"board":[8,7,1,6,0,2,5,4,3],"difficulty":"easy","ended_at_ms":null,"move_count":4,"reward_amount":0,"scramble":20,"session_id":108738,"size":3,"started_at_ms":1777480554657,"starting_board":[8,7,1,5,0,2,4,6,3],"status":"pending","won":false}],"server_now_ms":1777480748241}"#,
        )
        .unwrap();

        assert!(start.ok);
        assert_eq!(step.session.board[7], 0);
        assert_eq!(history.items[0].move_count, 4);
    }
}
