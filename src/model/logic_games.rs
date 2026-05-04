use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LogicGameKind {
    Sokoban,
    LightsOut,
    Maze,
    Nonogram,
    FlowFree,
}

impl LogicGameKind {
    pub const ALL: &'static [Self] = &[
        Self::Sokoban,
        Self::LightsOut,
        Self::Maze,
        Self::Nonogram,
        Self::FlowFree,
    ];

    pub fn slug(self) -> &'static str {
        match self {
            Self::Sokoban => "sokoban",
            Self::LightsOut => "lightsout",
            Self::Maze => "maze",
            Self::Nonogram => "nonogram",
            Self::FlowFree => "flowfree",
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Self::Sokoban => "推箱子",
            Self::LightsOut => "点灯",
            Self::Maze => "迷宫",
            Self::Nonogram => "数织",
            Self::FlowFree => "连线",
        }
    }

    pub fn config_path(self) -> &'static str {
        match self {
            Self::Sokoban => "/sokoban-api/config",
            Self::LightsOut => "/lightsout-api/config",
            Self::Maze => "/maze-api/config",
            Self::Nonogram => "/nonogram-api/config",
            Self::FlowFree => "/flowfree-api/config",
        }
    }

    pub fn me_path(self) -> &'static str {
        match self {
            Self::Sokoban => "/sokoban-api/me",
            Self::LightsOut => "/lightsout-api/me",
            Self::Maze => "/maze-api/me",
            Self::Nonogram => "/nonogram-api/me",
            Self::FlowFree => "/flowfree-api/me",
        }
    }

    pub fn start_path(self) -> &'static str {
        match self {
            Self::Sokoban => "/sokoban-api/start",
            Self::LightsOut => "/lightsout-api/start",
            Self::Maze => "/maze-api/start",
            Self::Nonogram => "/nonogram-api/start",
            Self::FlowFree => "/flowfree-api/start",
        }
    }

    pub fn action_path(self) -> &'static str {
        match self {
            Self::Sokoban => "/sokoban-api/move",
            Self::LightsOut => "/lightsout-api/click",
            Self::Maze => "/maze-api/move",
            Self::Nonogram => "/nonogram-api/click",
            Self::FlowFree => "/flowfree-api/click",
        }
    }

    pub fn history_path(self) -> &'static str {
        match self {
            Self::Sokoban => "/sokoban-api/history",
            Self::LightsOut => "/lightsout-api/history",
            Self::Maze => "/maze-api/history",
            Self::Nonogram => "/nonogram-api/history",
            Self::FlowFree => "/flowfree-api/history",
        }
    }

    pub fn referer_path(self) -> &'static str {
        match self {
            Self::Sokoban => "/sokoban",
            Self::LightsOut => "/lightsout",
            Self::Maze => "/maze",
            Self::Nonogram => "/nonogram",
            Self::FlowFree => "/flowfree",
        }
    }

    pub fn action_name(self) -> &'static str {
        match self {
            Self::Sokoban | Self::Maze => "move",
            Self::LightsOut | Self::Nonogram | Self::FlowFree => "click",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct LogicGameDifficultyConfig {
    #[serde(default)]
    pub color_count: i32,
    #[serde(default)]
    pub daily_plays: i32,
    #[serde(default)]
    pub density: f64,
    #[serde(default)]
    pub height: i32,
    #[serde(default)]
    pub level_count: i32,
    #[serde(default)]
    pub reward_amount: f64,
    #[serde(default)]
    pub scramble_count: i32,
    #[serde(default)]
    pub size: i32,
    #[serde(default)]
    pub width: i32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct LogicGameConfigResponse {
    #[serde(default)]
    pub actions: Vec<String>,
    #[serde(default)]
    pub cell_states: HashMap<String, i32>,
    #[serde(default)]
    pub difficulties: HashMap<String, LogicGameDifficultyConfig>,
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
pub struct LogicGameUser {
    #[serde(default)]
    pub balance: f64,
    #[serde(default)]
    pub email: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub user_id: String,
}

pub type LogicPoint = [i32; 2];
pub type LogicEdge = [LogicPoint; 2];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct LogicEndpoint(pub i32, pub LogicPoint, pub LogicPoint);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct LogicPath(pub i32, pub Vec<LogicPoint>);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct LogicGameSession {
    #[serde(default)]
    pub boxes: Vec<LogicPoint>,
    #[serde(default)]
    pub cells: Vec<Vec<i32>>,
    #[serde(default)]
    pub click_count: i32,
    #[serde(default)]
    pub col_clues: Vec<Vec<i32>>,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub difficulty: String,
    #[serde(default)]
    pub ended_at_ms: Option<i64>,
    #[serde(default)]
    pub endpoints: Vec<LogicEndpoint>,
    #[serde(default)]
    pub exit: LogicPoint,
    #[serde(default)]
    pub height: i32,
    #[serde(default)]
    pub level_index: i32,
    #[serde(default)]
    pub move_count: i32,
    #[serde(default)]
    pub open_edges: Vec<LogicEdge>,
    #[serde(default)]
    pub paths: Vec<LogicPath>,
    #[serde(default)]
    pub player: LogicPoint,
    #[serde(default)]
    pub push_count: i32,
    #[serde(default)]
    pub reward_amount: f64,
    #[serde(default)]
    pub row_clues: Vec<Vec<i32>>,
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
    pub starting_boxes: Vec<LogicPoint>,
    #[serde(default)]
    pub starting_cells: Vec<Vec<i32>>,
    #[serde(default)]
    pub starting_player: LogicPoint,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub targets: Vec<LogicPoint>,
    #[serde(default)]
    pub walls: Vec<LogicPoint>,
    #[serde(default)]
    pub width: i32,
    #[serde(default)]
    pub won: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct LogicGameMeResponse {
    #[serde(default)]
    pub active_session: Option<LogicGameSession>,
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
    pub user: LogicGameUser,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct LogicGameStartRequest {
    pub difficulty: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct LogicGameStartResponse {
    #[serde(default)]
    pub daily_plays_remaining: HashMap<String, i32>,
    #[serde(default)]
    pub ok: bool,
    #[serde(default)]
    pub server_now_ms: i64,
    #[serde(default)]
    pub session: LogicGameSession,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum LogicGameStep {
    Move {
        direction: String,
    },
    Click {
        r: i32,
        c: i32,
    },
    Mark {
        action: String,
        r: i32,
        c: i32,
    },
    Paint {
        action: String,
        color: i32,
        r: i32,
        c: i32,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct LogicGameActionResponse {
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
    pub session: LogicGameSession,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub won: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct LogicGameHistoryResponse {
    #[serde(default)]
    pub items: Vec<LogicGameSession>,
    #[serde(default)]
    pub server_now_ms: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_models_accept_new_game_payloads() {
        let sokoban: LogicGameStartResponse = serde_json::from_str(
            r#"{"ok":true,"session":{"boxes":[[2,3]],"difficulty":"easy","height":5,"player":[2,2],"session_id":1,"status":"pending","targets":[[2,4]],"walls":[[0,0]],"width":7}}"#,
        )
        .unwrap();
        let flow: LogicGameStartResponse = serde_json::from_str(
            r#"{"ok":true,"session":{"cells":[[1,0,2],[0,0,0],[1,0,2]],"endpoints":[[1,[0,0],[2,0]],[2,[0,2],[2,2]]],"paths":[[1,[]],[2,[]]],"height":3,"session_id":2,"width":3}}"#,
        )
        .unwrap();

        assert_eq!(sokoban.session.boxes[0], [2, 3]);
        assert_eq!(flow.session.endpoints[1].0, 2);
    }
}
