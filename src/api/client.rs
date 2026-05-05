use std::time::Duration;

use crate::model::{
    AbandonRequest, AbandonResponse, AuthMeResponse, CheckinClaimResponse, CheckinMeResponse,
    CheckinTodayResponse, ConfigResponse, FlowfreeAbandonRequest, FlowfreeAbandonResponse,
    FlowfreeConfigResponse, FlowfreeFinishRequest, FlowfreeFinishResponse, FlowfreeHistoryResponse,
    FlowfreeMeResponse, FlowfreeMove, FlowfreeStartRequest, FlowfreeStartResponse, HistoryResponse,
    LightsoutClickRequest, LightsoutClickResponse, LightsoutConfigResponse,
    LightsoutHistoryResponse, LightsoutMeResponse, LightsoutStartRequest, LightsoutStartResponse,
    LoginRequest, LoginResponse, MazeConfigResponse, MazeHistoryResponse, MazeMeResponse,
    MazeMoveRequest, MazeMoveResponse, MazeStartRequest, MazeStartResponse, MemoryConfigResponse,
    MemoryFlipRequest, MemoryFlipResponse, MemoryHistoryResponse, MemoryMeResponse,
    MemoryStartRequest, MemoryStartResponse, MinesweeperClickRequest, MinesweeperClickResponse,
    MinesweeperConfigResponse, MinesweeperHistoryResponse, MinesweeperMeResponse,
    MinesweeperStartRequest, MinesweeperStartResponse, NonogramClickRequest, NonogramClickResponse,
    NonogramConfigResponse, NonogramFinishRequest, NonogramFinishResponse, NonogramHistoryResponse,
    NonogramMeResponse, NonogramMove, NonogramStartRequest, NonogramStartResponse,
    Puzzle15ConfigResponse, Puzzle15HistoryResponse, Puzzle15MeResponse, Puzzle15MoveRequest,
    Puzzle15MoveResponse, Puzzle15StartRequest, Puzzle15StartResponse, Puzzle2048AbandonRequest,
    Puzzle2048ConfigResponse, Puzzle2048HistoryResponse, Puzzle2048MeResponse,
    Puzzle2048MoveRequest, Puzzle2048MoveResponse, Puzzle2048StartRequest, Puzzle2048StartResponse,
    ScratchHistoryResponse, ScratchPlayRequest, ScratchPlayResponse, ScratchRevealRequest,
    ScratchRevealResponse, SessionCookie, SokobanConfigResponse, SokobanHistoryResponse,
    SokobanMeResponse, SokobanMoveRequest, SokobanMoveResponse, SokobanStartRequest,
    SokobanStartResponse, StartRequest, StartResponse, StepRequest, StepResponse,
    SudokuConfigResponse, SudokuFillRequest, SudokuFillResponse, SudokuHistoryResponse,
    SudokuMeResponse, SudokuStartRequest, SudokuStartResponse, TileMeResponse,
};
use crate::storage::{build_authorization, normalize_base_url};
use reqwest::blocking::Client;
use reqwest::header::{
    ACCEPT, ACCEPT_LANGUAGE, AUTHORIZATION, CONTENT_TYPE, COOKIE, ORIGIN, REFERER, SET_COOKIE,
    USER_AGENT,
};
use reqwest::{Method, StatusCode};
use serde::de::DeserializeOwned;

use super::cookies::{cookie_header_value, merge_session_cookies, normalize_session_cookies};
use super::endpoints::{api_label_for_path, localized_status_message};
use super::{
    AUTH_ME_PATH, ApiClient, ApiError, CHECKIN_CLAIM_PATH, CHECKIN_ME_PATH, CHECKIN_TODAY_PATH,
    DEFAULT_BASE_URL, DEFAULT_USER_AGENT, FLOWFREE_ABANDON_PATH, FLOWFREE_CONFIG_PATH,
    FLOWFREE_FINISH_PATH, FLOWFREE_HISTORY_PATH, FLOWFREE_ME_PATH, FLOWFREE_START_PATH,
    LIGHTSOUT_CLICK_PATH, LIGHTSOUT_CONFIG_PATH, LIGHTSOUT_HISTORY_PATH, LIGHTSOUT_ME_PATH,
    LIGHTSOUT_START_PATH, LOGIN_PATH, MAZE_CONFIG_PATH, MAZE_HISTORY_PATH, MAZE_ME_PATH,
    MAZE_MOVE_PATH, MAZE_START_PATH, MEMORY_CONFIG_PATH, MEMORY_FLIP_PATH, MEMORY_HISTORY_PATH,
    MEMORY_ME_PATH, MEMORY_START_PATH, MINESWEEPER_CLICK_PATH, MINESWEEPER_CONFIG_PATH,
    MINESWEEPER_HISTORY_PATH, MINESWEEPER_ME_PATH, MINESWEEPER_START_PATH, NONOGRAM_CLICK_PATH,
    NONOGRAM_CONFIG_PATH, NONOGRAM_FINISH_PATH, NONOGRAM_HISTORY_PATH, NONOGRAM_ME_PATH,
    NONOGRAM_START_PATH, PUZZLE_15_CONFIG_PATH, PUZZLE_15_HISTORY_PATH, PUZZLE_15_ME_PATH,
    PUZZLE_15_MOVE_PATH, PUZZLE_15_START_PATH, PUZZLE_2048_ABANDON_PATH, PUZZLE_2048_CONFIG_PATH,
    PUZZLE_2048_HISTORY_PATH, PUZZLE_2048_ME_PATH, PUZZLE_2048_MOVE_PATH, PUZZLE_2048_START_PATH,
    SCRATCH_HISTORY_PATH, SCRATCH_PLAY_PATH, SCRATCH_REVEAL_PATH, SOKOBAN_CONFIG_PATH,
    SOKOBAN_HISTORY_PATH, SOKOBAN_ME_PATH, SOKOBAN_MOVE_PATH, SOKOBAN_START_PATH,
    SUDOKU_CONFIG_PATH, SUDOKU_FILL_PATH, SUDOKU_HISTORY_PATH, SUDOKU_ME_PATH, SUDOKU_START_PATH,
    TILE_ABANDON_PATH, TILE_CONFIG_PATH, TILE_HISTORY_PATH, TILE_ME_PATH, TILE_START_PATH,
    TILE_STEP_PATH, UnauthorizedError,
};

impl ApiClient {
    pub fn new(base_url: impl AsRef<str>) -> Self {
        let base_url = normalize_base_url(base_url.as_ref());
        let base_url = if base_url.is_empty() {
            DEFAULT_BASE_URL.to_string()
        } else {
            base_url
        };
        let http_client = Client::builder()
            .timeout(Duration::from_secs(30))
            .connect_timeout(Duration::from_secs(10))
            .build()
            .expect("failed to build reqwest client");
        Self {
            base_url,
            http_client,
            session_cookies: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
        }
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    pub fn export_session_cookies(&self) -> Vec<SessionCookie> {
        self.session_cookies.lock().unwrap().clone()
    }

    pub fn clear_session_cookies(&mut self) {
        self.session_cookies.lock().unwrap().clear();
    }

    pub fn load_session_cookies(&mut self, cookies: &[SessionCookie]) -> Result<(), ApiError> {
        let mut stored = self.session_cookies.lock().unwrap();
        *stored = normalize_session_cookies(cookies.to_vec());
        Ok(())
    }

    pub fn validate_auth_token(&self, auth_token: &str) -> Result<AuthMeResponse, ApiError> {
        self.get_json(
            Method::GET,
            AUTH_ME_PATH,
            auth_token,
            &(self.base_url.clone() + "/dashboard"),
            Option::<&()>::None,
        )
    }

    pub fn do_login(
        &self,
        email: &str,
        password: &str,
    ) -> Result<(LoginResponse, String), ApiError> {
        let response: LoginResponse = self.get_json(
            Method::POST,
            LOGIN_PATH,
            "",
            &(self.base_url.clone() + "/dashboard"),
            Some(&LoginRequest {
                email: email.to_string(),
                password: password.to_string(),
            }),
        )?;
        if response.code != 0 {
            let message = if response.reason == "INVALID_CREDENTIALS"
                || response
                    .message
                    .eq_ignore_ascii_case("invalid email or password")
            {
                "邮箱或密码错误".to_string()
            } else if response.message.trim().is_empty() {
                format!("登录失败，服务端返回的错误码是 {}", response.code)
            } else {
                format!("登录失败：{}", response.message.trim())
            };
            return Err(ApiError::Unauthorized(UnauthorizedError::new(message)));
        }
        let auth_token =
            build_authorization(&response.data.token_type, &response.data.access_token);
        if auth_token.is_empty() {
            return Err(ApiError::Message("登录返回的令牌为空".to_string()));
        }
        Ok((response, auth_token))
    }

    pub fn get_checkin_me(&self, auth_token: &str) -> Result<CheckinMeResponse, ApiError> {
        let response: CheckinMeResponse = self.get_json(
            Method::GET,
            CHECKIN_ME_PATH,
            auth_token,
            &(self.base_url.clone() + "/checkin"),
            Option::<&()>::None,
        )?;
        if !response.authenticated {
            return Err(ApiError::Unauthorized(UnauthorizedError::new(
                "获取签到账号信息失败：登录状态已失效，请重新登录",
            )));
        }
        Ok(response)
    }

    pub fn get_checkin_today(&self, auth_token: &str) -> Result<CheckinTodayResponse, ApiError> {
        self.get_json(
            Method::GET,
            CHECKIN_TODAY_PATH,
            auth_token,
            &(self.base_url.clone() + "/checkin"),
            Option::<&()>::None,
        )
    }

    pub fn claim_checkin_today(&self, auth_token: &str) -> Result<CheckinClaimResponse, ApiError> {
        self.get_json(
            Method::POST,
            CHECKIN_CLAIM_PATH,
            auth_token,
            &(self.base_url.clone() + "/checkin"),
            Some(&serde_json::json!({})),
        )
    }

    pub fn play_scratch(
        &self,
        auth_token: &str,
        game_type: &str,
    ) -> Result<ScratchPlayResponse, ApiError> {
        let mut response: ScratchPlayResponse = self.get_json(
            Method::POST,
            SCRATCH_PLAY_PATH,
            auth_token,
            &(self.base_url.clone() + "/scratch"),
            Some(&ScratchPlayRequest {
                game_type: game_type.to_string(),
            }),
        )?;
        if response.game_type.trim().is_empty() {
            response.game_type = game_type.to_string();
        }
        Ok(response)
    }

    pub fn reveal_scratch(
        &self,
        auth_token: &str,
        play_id: i32,
        reveal_token: &str,
    ) -> Result<ScratchRevealResponse, ApiError> {
        self.get_json(
            Method::POST,
            SCRATCH_REVEAL_PATH,
            auth_token,
            &(self.base_url.clone() + "/scratch"),
            Some(&ScratchRevealRequest {
                play_id,
                reveal_token: reveal_token.to_string(),
            }),
        )
    }

    pub fn get_scratch_history(
        &self,
        auth_token: &str,
    ) -> Result<ScratchHistoryResponse, ApiError> {
        self.get_json(
            Method::GET,
            SCRATCH_HISTORY_PATH,
            auth_token,
            &(self.base_url.clone() + "/scratch"),
            Option::<&()>::None,
        )
    }

    pub fn get_tile_config(&self, auth_token: &str) -> Result<ConfigResponse, ApiError> {
        self.get_json(
            Method::GET,
            TILE_CONFIG_PATH,
            auth_token,
            &(self.base_url.clone() + "/tile"),
            Option::<&()>::None,
        )
    }

    pub fn get_tile_history(&self, auth_token: &str) -> Result<HistoryResponse, ApiError> {
        self.get_json(
            Method::GET,
            TILE_HISTORY_PATH,
            auth_token,
            &(self.base_url.clone() + "/tile"),
            Option::<&()>::None,
        )
    }

    pub fn get_tile_me(&self, auth_token: &str) -> Result<TileMeResponse, ApiError> {
        let response: TileMeResponse = self.get_json(
            Method::GET,
            TILE_ME_PATH,
            auth_token,
            &(self.base_url.clone() + "/tile"),
            Option::<&()>::None,
        )?;
        if !response.authenticated {
            return Err(ApiError::Unauthorized(UnauthorizedError::new(
                "获取游戏信息失败：登录状态已失效，请重新登录",
            )));
        }
        Ok(response)
    }

    pub fn start_game(
        &self,
        auth_token: &str,
        difficulty: &str,
    ) -> Result<StartResponse, ApiError> {
        let mut response: StartResponse = self.get_json(
            Method::POST,
            TILE_START_PATH,
            auth_token,
            &(self.base_url.clone() + "/tile"),
            Some(&StartRequest {
                difficulty: difficulty.to_string(),
            }),
        )?;
        if response.difficulty.trim().is_empty() {
            response.difficulty = difficulty.to_string();
        }
        Ok(response)
    }

    pub fn step(&self, auth_token: &str, request: StepRequest) -> Result<StepResponse, ApiError> {
        self.get_json(
            Method::POST,
            TILE_STEP_PATH,
            auth_token,
            &(self.base_url.clone() + "/tile"),
            Some(&request),
        )
    }

    pub fn abandon(&self, auth_token: &str, session_id: i32) -> Result<AbandonResponse, ApiError> {
        self.get_json(
            Method::POST,
            TILE_ABANDON_PATH,
            auth_token,
            &(self.base_url.clone() + "/tile"),
            Some(&AbandonRequest { session_id }),
        )
    }

    pub fn get_puzzle_2048_config(
        &self,
        auth_token: &str,
    ) -> Result<Puzzle2048ConfigResponse, ApiError> {
        self.get_json(
            Method::GET,
            PUZZLE_2048_CONFIG_PATH,
            auth_token,
            &(self.base_url.clone() + "/puzzle2048"),
            Option::<&()>::None,
        )
    }

    pub fn get_puzzle_2048_history(
        &self,
        auth_token: &str,
    ) -> Result<Puzzle2048HistoryResponse, ApiError> {
        self.get_json(
            Method::GET,
            PUZZLE_2048_HISTORY_PATH,
            auth_token,
            &(self.base_url.clone() + "/puzzle2048"),
            Option::<&()>::None,
        )
    }

    pub fn get_puzzle_2048_me(&self, auth_token: &str) -> Result<Puzzle2048MeResponse, ApiError> {
        let response: Puzzle2048MeResponse = self.get_json(
            Method::GET,
            PUZZLE_2048_ME_PATH,
            auth_token,
            &(self.base_url.clone() + "/puzzle2048"),
            Option::<&()>::None,
        )?;
        if !response.authenticated {
            return Err(ApiError::Unauthorized(UnauthorizedError::new(
                "获取谜题2048账号信息失败：登录状态已失效，请重新登录",
            )));
        }
        Ok(response)
    }

    pub fn start_puzzle_2048(
        &self,
        auth_token: &str,
        difficulty: &str,
    ) -> Result<Puzzle2048StartResponse, ApiError> {
        let mut response: Puzzle2048StartResponse = self.get_json(
            Method::POST,
            PUZZLE_2048_START_PATH,
            auth_token,
            &(self.base_url.clone() + "/puzzle2048"),
            Some(&Puzzle2048StartRequest {
                difficulty: difficulty.to_string(),
            }),
        )?;
        if response.difficulty.trim().is_empty() {
            response.difficulty = difficulty.to_string();
        }
        Ok(response)
    }

    pub fn move_puzzle_2048(
        &self,
        auth_token: &str,
        session_id: i32,
        direction: &str,
    ) -> Result<Puzzle2048MoveResponse, ApiError> {
        self.get_json(
            Method::POST,
            PUZZLE_2048_MOVE_PATH,
            auth_token,
            &(self.base_url.clone() + "/puzzle2048"),
            Some(&Puzzle2048MoveRequest {
                session_id,
                direction: direction.to_string(),
            }),
        )
    }

    pub fn abandon_puzzle_2048(
        &self,
        auth_token: &str,
        session_id: i32,
    ) -> Result<Puzzle2048MoveResponse, ApiError> {
        self.get_json(
            Method::POST,
            PUZZLE_2048_ABANDON_PATH,
            auth_token,
            &(self.base_url.clone() + "/puzzle2048"),
            Some(&Puzzle2048AbandonRequest { session_id }),
        )
    }

    pub fn get_memory_config(&self, auth_token: &str) -> Result<MemoryConfigResponse, ApiError> {
        self.get_json(
            Method::GET,
            MEMORY_CONFIG_PATH,
            auth_token,
            &(self.base_url.clone() + "/memory"),
            Option::<&()>::None,
        )
    }

    pub fn get_memory_history(&self, auth_token: &str) -> Result<MemoryHistoryResponse, ApiError> {
        self.get_json(
            Method::GET,
            MEMORY_HISTORY_PATH,
            auth_token,
            &(self.base_url.clone() + "/memory"),
            Option::<&()>::None,
        )
    }

    pub fn get_memory_me(&self, auth_token: &str) -> Result<MemoryMeResponse, ApiError> {
        let response: MemoryMeResponse = self.get_json(
            Method::GET,
            MEMORY_ME_PATH,
            auth_token,
            &(self.base_url.clone() + "/memory"),
            Option::<&()>::None,
        )?;
        if !response.authenticated {
            return Err(ApiError::Unauthorized(UnauthorizedError::new(
                "获取记忆翻牌账号信息失败：登录状态已失效，请重新登录",
            )));
        }
        Ok(response)
    }

    pub fn start_memory(
        &self,
        auth_token: &str,
        difficulty: &str,
    ) -> Result<MemoryStartResponse, ApiError> {
        let mut response: MemoryStartResponse = self.get_json(
            Method::POST,
            MEMORY_START_PATH,
            auth_token,
            &(self.base_url.clone() + "/memory"),
            Some(&MemoryStartRequest {
                difficulty: difficulty.to_string(),
            }),
        )?;
        if response.difficulty.trim().is_empty() {
            response.difficulty = difficulty.to_string();
        }
        Ok(response)
    }

    pub fn flip_memory(
        &self,
        auth_token: &str,
        session_id: i32,
        index: i32,
    ) -> Result<MemoryFlipResponse, ApiError> {
        self.get_json(
            Method::POST,
            MEMORY_FLIP_PATH,
            auth_token,
            &(self.base_url.clone() + "/memory"),
            Some(&MemoryFlipRequest { session_id, index }),
        )
    }

    pub fn get_minesweeper_config(
        &self,
        auth_token: &str,
    ) -> Result<MinesweeperConfigResponse, ApiError> {
        self.get_json(
            Method::GET,
            MINESWEEPER_CONFIG_PATH,
            auth_token,
            &(self.base_url.clone() + "/minesweeper"),
            Option::<&()>::None,
        )
    }

    pub fn get_minesweeper_history(
        &self,
        auth_token: &str,
    ) -> Result<MinesweeperHistoryResponse, ApiError> {
        self.get_json(
            Method::GET,
            MINESWEEPER_HISTORY_PATH,
            auth_token,
            &(self.base_url.clone() + "/minesweeper"),
            Option::<&()>::None,
        )
    }

    pub fn get_minesweeper_me(&self, auth_token: &str) -> Result<MinesweeperMeResponse, ApiError> {
        let response: MinesweeperMeResponse = self.get_json(
            Method::GET,
            MINESWEEPER_ME_PATH,
            auth_token,
            &(self.base_url.clone() + "/minesweeper"),
            Option::<&()>::None,
        )?;
        if !response.ok {
            return Err(ApiError::Message(
                "获取扫雷账号信息失败：服务端返回 ok=false".to_string(),
            ));
        }
        Ok(response)
    }

    pub fn start_minesweeper(
        &self,
        auth_token: &str,
        difficulty: &str,
    ) -> Result<MinesweeperStartResponse, ApiError> {
        let mut response: MinesweeperStartResponse = self.get_json(
            Method::POST,
            MINESWEEPER_START_PATH,
            auth_token,
            &(self.base_url.clone() + "/minesweeper"),
            Some(&MinesweeperStartRequest {
                difficulty: difficulty.to_string(),
            }),
        )?;
        if response.session.difficulty.trim().is_empty() {
            response.session.difficulty = difficulty.to_string();
        }
        Ok(response)
    }

    pub fn click_minesweeper(
        &self,
        auth_token: &str,
        play_id: i32,
        action: &str,
        x: i32,
        y: i32,
    ) -> Result<MinesweeperClickResponse, ApiError> {
        self.get_json(
            Method::POST,
            MINESWEEPER_CLICK_PATH,
            auth_token,
            &(self.base_url.clone() + "/minesweeper"),
            Some(&MinesweeperClickRequest {
                play_id,
                action: action.to_string(),
                x,
                y,
            }),
        )
    }

    pub fn get_sokoban_config(&self, auth_token: &str) -> Result<SokobanConfigResponse, ApiError> {
        self.get_json(
            Method::GET,
            SOKOBAN_CONFIG_PATH,
            auth_token,
            &(self.base_url.clone() + "/sokoban"),
            Option::<&()>::None,
        )
    }

    pub fn get_sokoban_me(&self, auth_token: &str) -> Result<SokobanMeResponse, ApiError> {
        let response: SokobanMeResponse = self.get_json(
            Method::GET,
            SOKOBAN_ME_PATH,
            auth_token,
            &(self.base_url.clone() + "/sokoban"),
            Option::<&()>::None,
        )?;
        if !response.ok {
            return Err(ApiError::Message(
                "get sokoban account info returned ok=false".to_string(),
            ));
        }
        Ok(response)
    }

    pub fn start_sokoban(
        &self,
        auth_token: &str,
        difficulty: &str,
    ) -> Result<SokobanStartResponse, ApiError> {
        let mut response: SokobanStartResponse = self.get_json(
            Method::POST,
            SOKOBAN_START_PATH,
            auth_token,
            &(self.base_url.clone() + "/sokoban"),
            Some(&SokobanStartRequest {
                difficulty: difficulty.to_string(),
            }),
        )?;
        if response.session.difficulty.trim().is_empty() {
            response.session.difficulty = difficulty.to_string();
        }
        Ok(response)
    }

    pub fn move_sokoban(
        &self,
        auth_token: &str,
        session_id: i32,
        direction: &str,
    ) -> Result<SokobanMoveResponse, ApiError> {
        self.get_json(
            Method::POST,
            SOKOBAN_MOVE_PATH,
            auth_token,
            &(self.base_url.clone() + "/sokoban"),
            Some(&SokobanMoveRequest {
                session_id,
                direction: direction.to_string(),
            }),
        )
    }

    pub fn get_sokoban_history(
        &self,
        auth_token: &str,
    ) -> Result<SokobanHistoryResponse, ApiError> {
        self.get_json(
            Method::GET,
            SOKOBAN_HISTORY_PATH,
            auth_token,
            &(self.base_url.clone() + "/sokoban"),
            Option::<&()>::None,
        )
    }

    pub fn get_lightsout_config(
        &self,
        auth_token: &str,
    ) -> Result<LightsoutConfigResponse, ApiError> {
        self.get_json(
            Method::GET,
            LIGHTSOUT_CONFIG_PATH,
            auth_token,
            &(self.base_url.clone() + "/lightsout"),
            Option::<&()>::None,
        )
    }

    pub fn get_lightsout_me(&self, auth_token: &str) -> Result<LightsoutMeResponse, ApiError> {
        let response: LightsoutMeResponse = self.get_json(
            Method::GET,
            LIGHTSOUT_ME_PATH,
            auth_token,
            &(self.base_url.clone() + "/lightsout"),
            Option::<&()>::None,
        )?;
        if !response.ok {
            return Err(ApiError::Message(
                "get lightsout account info returned ok=false".to_string(),
            ));
        }
        Ok(response)
    }

    pub fn start_lightsout(
        &self,
        auth_token: &str,
        difficulty: &str,
    ) -> Result<LightsoutStartResponse, ApiError> {
        let mut response: LightsoutStartResponse = self.get_json(
            Method::POST,
            LIGHTSOUT_START_PATH,
            auth_token,
            &(self.base_url.clone() + "/lightsout"),
            Some(&LightsoutStartRequest {
                difficulty: difficulty.to_string(),
            }),
        )?;
        if response.session.difficulty.trim().is_empty() {
            response.session.difficulty = difficulty.to_string();
        }
        Ok(response)
    }

    pub fn click_lightsout(
        &self,
        auth_token: &str,
        session_id: i32,
        r: i32,
        c: i32,
    ) -> Result<LightsoutClickResponse, ApiError> {
        self.get_json(
            Method::POST,
            LIGHTSOUT_CLICK_PATH,
            auth_token,
            &(self.base_url.clone() + "/lightsout"),
            Some(&LightsoutClickRequest { session_id, r, c }),
        )
    }

    pub fn get_lightsout_history(
        &self,
        auth_token: &str,
    ) -> Result<LightsoutHistoryResponse, ApiError> {
        self.get_json(
            Method::GET,
            LIGHTSOUT_HISTORY_PATH,
            auth_token,
            &(self.base_url.clone() + "/lightsout"),
            Option::<&()>::None,
        )
    }

    pub fn get_maze_config(&self, auth_token: &str) -> Result<MazeConfigResponse, ApiError> {
        self.get_json(
            Method::GET,
            MAZE_CONFIG_PATH,
            auth_token,
            &(self.base_url.clone() + "/maze"),
            Option::<&()>::None,
        )
    }

    pub fn get_maze_me(&self, auth_token: &str) -> Result<MazeMeResponse, ApiError> {
        let response: MazeMeResponse = self.get_json(
            Method::GET,
            MAZE_ME_PATH,
            auth_token,
            &(self.base_url.clone() + "/maze"),
            Option::<&()>::None,
        )?;
        if !response.ok {
            return Err(ApiError::Message(
                "get maze account info returned ok=false".to_string(),
            ));
        }
        Ok(response)
    }

    pub fn start_maze(
        &self,
        auth_token: &str,
        difficulty: &str,
    ) -> Result<MazeStartResponse, ApiError> {
        let mut response: MazeStartResponse = self.get_json(
            Method::POST,
            MAZE_START_PATH,
            auth_token,
            &(self.base_url.clone() + "/maze"),
            Some(&MazeStartRequest {
                difficulty: difficulty.to_string(),
            }),
        )?;
        if response.session.difficulty.trim().is_empty() {
            response.session.difficulty = difficulty.to_string();
        }
        Ok(response)
    }

    pub fn move_maze(
        &self,
        auth_token: &str,
        session_id: i32,
        direction: &str,
    ) -> Result<MazeMoveResponse, ApiError> {
        self.get_json(
            Method::POST,
            MAZE_MOVE_PATH,
            auth_token,
            &(self.base_url.clone() + "/maze"),
            Some(&MazeMoveRequest {
                session_id,
                direction: direction.to_string(),
            }),
        )
    }

    pub fn get_maze_history(&self, auth_token: &str) -> Result<MazeHistoryResponse, ApiError> {
        self.get_json(
            Method::GET,
            MAZE_HISTORY_PATH,
            auth_token,
            &(self.base_url.clone() + "/maze"),
            Option::<&()>::None,
        )
    }

    pub fn get_nonogram_config(
        &self,
        auth_token: &str,
    ) -> Result<NonogramConfigResponse, ApiError> {
        self.get_json(
            Method::GET,
            NONOGRAM_CONFIG_PATH,
            auth_token,
            &(self.base_url.clone() + "/nonogram"),
            Option::<&()>::None,
        )
    }

    pub fn get_nonogram_me(&self, auth_token: &str) -> Result<NonogramMeResponse, ApiError> {
        let response: NonogramMeResponse = self.get_json(
            Method::GET,
            NONOGRAM_ME_PATH,
            auth_token,
            &(self.base_url.clone() + "/nonogram"),
            Option::<&()>::None,
        )?;
        if !response.ok {
            return Err(ApiError::Message(
                "get nonogram account info returned ok=false".to_string(),
            ));
        }
        Ok(response)
    }

    pub fn start_nonogram(
        &self,
        auth_token: &str,
        difficulty: &str,
    ) -> Result<NonogramStartResponse, ApiError> {
        let mut response: NonogramStartResponse = self.get_json(
            Method::POST,
            NONOGRAM_START_PATH,
            auth_token,
            &(self.base_url.clone() + "/nonogram"),
            Some(&NonogramStartRequest {
                difficulty: difficulty.to_string(),
            }),
        )?;
        if response.session.difficulty.trim().is_empty() {
            response.session.difficulty = difficulty.to_string();
        }
        Ok(response)
    }

    pub fn click_nonogram(
        &self,
        auth_token: &str,
        session_id: i32,
        action: &str,
        r: i32,
        c: i32,
    ) -> Result<NonogramClickResponse, ApiError> {
        self.get_json(
            Method::POST,
            NONOGRAM_CLICK_PATH,
            auth_token,
            &(self.base_url.clone() + "/nonogram"),
            Some(&NonogramClickRequest {
                session_id,
                action: action.to_string(),
                r,
                c,
            }),
        )
    }

    pub fn finish_nonogram(
        &self,
        auth_token: &str,
        session_id: i32,
        moves: Vec<NonogramMove>,
    ) -> Result<NonogramFinishResponse, ApiError> {
        self.get_json(
            Method::POST,
            NONOGRAM_FINISH_PATH,
            auth_token,
            &(self.base_url.clone() + "/nonogram"),
            Some(&NonogramFinishRequest { session_id, moves }),
        )
    }

    pub fn get_nonogram_history(
        &self,
        auth_token: &str,
    ) -> Result<NonogramHistoryResponse, ApiError> {
        self.get_json(
            Method::GET,
            NONOGRAM_HISTORY_PATH,
            auth_token,
            &(self.base_url.clone() + "/nonogram"),
            Option::<&()>::None,
        )
    }

    pub fn get_flowfree_config(
        &self,
        auth_token: &str,
    ) -> Result<FlowfreeConfigResponse, ApiError> {
        self.get_json(
            Method::GET,
            FLOWFREE_CONFIG_PATH,
            auth_token,
            &(self.base_url.clone() + "/flowfree"),
            Option::<&()>::None,
        )
    }

    pub fn get_flowfree_me(&self, auth_token: &str) -> Result<FlowfreeMeResponse, ApiError> {
        let response: FlowfreeMeResponse = self.get_json(
            Method::GET,
            FLOWFREE_ME_PATH,
            auth_token,
            &(self.base_url.clone() + "/flowfree"),
            Option::<&()>::None,
        )?;
        if !response.ok {
            return Err(ApiError::Message(
                "get flowfree account info returned ok=false".to_string(),
            ));
        }
        Ok(response)
    }

    pub fn start_flowfree(
        &self,
        auth_token: &str,
        difficulty: &str,
    ) -> Result<FlowfreeStartResponse, ApiError> {
        let mut response: FlowfreeStartResponse = self.get_json(
            Method::POST,
            FLOWFREE_START_PATH,
            auth_token,
            &(self.base_url.clone() + "/flowfree"),
            Some(&FlowfreeStartRequest {
                difficulty: difficulty.to_string(),
            }),
        )?;
        if response.session.difficulty.trim().is_empty() {
            response.session.difficulty = difficulty.to_string();
        }
        Ok(response)
    }

    pub fn finish_flowfree(
        &self,
        auth_token: &str,
        session_id: i32,
        moves: Vec<FlowfreeMove>,
    ) -> Result<FlowfreeFinishResponse, ApiError> {
        self.get_json(
            Method::POST,
            FLOWFREE_FINISH_PATH,
            auth_token,
            &(self.base_url.clone() + "/flowfree"),
            Some(&FlowfreeFinishRequest { session_id, moves }),
        )
    }

    pub fn abandon_flowfree(
        &self,
        auth_token: &str,
        session_id: i32,
    ) -> Result<FlowfreeAbandonResponse, ApiError> {
        self.get_json(
            Method::POST,
            FLOWFREE_ABANDON_PATH,
            auth_token,
            &(self.base_url.clone() + "/flowfree"),
            Some(&FlowfreeAbandonRequest { session_id }),
        )
    }

    pub fn get_flowfree_history(
        &self,
        auth_token: &str,
    ) -> Result<FlowfreeHistoryResponse, ApiError> {
        self.get_json(
            Method::GET,
            FLOWFREE_HISTORY_PATH,
            auth_token,
            &(self.base_url.clone() + "/flowfree"),
            Option::<&()>::None,
        )
    }
    pub fn get_puzzle_15_config(
        &self,
        auth_token: &str,
    ) -> Result<Puzzle15ConfigResponse, ApiError> {
        self.get_json(
            Method::GET,
            PUZZLE_15_CONFIG_PATH,
            auth_token,
            &(self.base_url.clone() + "/puzzle15"),
            Option::<&()>::None,
        )
    }

    pub fn get_puzzle_15_history(
        &self,
        auth_token: &str,
    ) -> Result<Puzzle15HistoryResponse, ApiError> {
        self.get_json(
            Method::GET,
            PUZZLE_15_HISTORY_PATH,
            auth_token,
            &(self.base_url.clone() + "/puzzle15"),
            Option::<&()>::None,
        )
    }

    pub fn get_puzzle_15_me(&self, auth_token: &str) -> Result<Puzzle15MeResponse, ApiError> {
        let response: Puzzle15MeResponse = self.get_json(
            Method::GET,
            PUZZLE_15_ME_PATH,
            auth_token,
            &(self.base_url.clone() + "/puzzle15"),
            Option::<&()>::None,
        )?;
        if !response.authenticated {
            return Err(ApiError::Unauthorized(UnauthorizedError::new(
                "获取华容道账号信息失败：登录状态已失效，请重新登录",
            )));
        }
        Ok(response)
    }

    pub fn start_puzzle_15(
        &self,
        auth_token: &str,
        difficulty: &str,
    ) -> Result<Puzzle15StartResponse, ApiError> {
        let mut response: Puzzle15StartResponse = self.get_json(
            Method::POST,
            PUZZLE_15_START_PATH,
            auth_token,
            &(self.base_url.clone() + "/puzzle15"),
            Some(&Puzzle15StartRequest {
                difficulty: difficulty.to_string(),
            }),
        )?;
        if response.difficulty.trim().is_empty() {
            response.difficulty = difficulty.to_string();
        }
        Ok(response)
    }

    pub fn move_puzzle_15(
        &self,
        auth_token: &str,
        session_id: i32,
        direction: &str,
    ) -> Result<Puzzle15MoveResponse, ApiError> {
        self.get_json(
            Method::POST,
            PUZZLE_15_MOVE_PATH,
            auth_token,
            &(self.base_url.clone() + "/puzzle15"),
            Some(&Puzzle15MoveRequest {
                session_id,
                direction: direction.to_string(),
            }),
        )
    }

    pub fn get_sudoku_config(&self, auth_token: &str) -> Result<SudokuConfigResponse, ApiError> {
        self.get_json(
            Method::GET,
            SUDOKU_CONFIG_PATH,
            auth_token,
            &(self.base_url.clone() + "/sudoku"),
            Option::<&()>::None,
        )
    }

    pub fn get_sudoku_history(&self, auth_token: &str) -> Result<SudokuHistoryResponse, ApiError> {
        self.get_json(
            Method::GET,
            SUDOKU_HISTORY_PATH,
            auth_token,
            &(self.base_url.clone() + "/sudoku"),
            Option::<&()>::None,
        )
    }

    pub fn get_sudoku_me(&self, auth_token: &str) -> Result<SudokuMeResponse, ApiError> {
        let response: SudokuMeResponse = self.get_json(
            Method::GET,
            SUDOKU_ME_PATH,
            auth_token,
            &(self.base_url.clone() + "/sudoku"),
            Option::<&()>::None,
        )?;
        if !response.authenticated {
            return Err(ApiError::Unauthorized(UnauthorizedError::new(
                "获取数独账号信息失败：登录状态已失效，请重新登录",
            )));
        }
        Ok(response)
    }

    pub fn start_sudoku(
        &self,
        auth_token: &str,
        difficulty: &str,
    ) -> Result<SudokuStartResponse, ApiError> {
        let mut response: SudokuStartResponse = self.get_json(
            Method::POST,
            SUDOKU_START_PATH,
            auth_token,
            &(self.base_url.clone() + "/sudoku"),
            Some(&SudokuStartRequest {
                difficulty: difficulty.to_string(),
            }),
        )?;
        if response.difficulty.trim().is_empty() {
            response.difficulty = difficulty.to_string();
        }
        Ok(response)
    }

    pub fn fill_sudoku(
        &self,
        auth_token: &str,
        session_id: i32,
        row: i32,
        col: i32,
        value: Option<i32>,
    ) -> Result<SudokuFillResponse, ApiError> {
        self.get_json(
            Method::POST,
            SUDOKU_FILL_PATH,
            auth_token,
            &(self.base_url.clone() + "/sudoku"),
            Some(&SudokuFillRequest {
                session_id,
                row,
                col,
                value,
            }),
        )
    }

    fn get_json<T: DeserializeOwned, B: serde::Serialize>(
        &self,
        method: Method,
        path: &str,
        auth_token: &str,
        referer: &str,
        payload: Option<&B>,
    ) -> Result<T, ApiError> {
        let url = format!("{}{}", self.base_url, path);
        let mut request = self.http_client.request(method, &url);
        request = request
            .header(CONTENT_TYPE, "application/json")
            .header(ACCEPT, "application/json, text/plain, */*")
            .header(ACCEPT_LANGUAGE, "zh")
            .header(ORIGIN, &self.base_url)
            .header(REFERER, referer)
            .header(USER_AGENT, DEFAULT_USER_AGENT);
        if !auth_token.trim().is_empty() {
            request = request.header(AUTHORIZATION, auth_token.trim());
        }
        let cookie_header = {
            let cookies = self.session_cookies.lock().unwrap();
            cookie_header_value(&cookies)
        };
        if !cookie_header.is_empty() {
            request = request.header(COOKIE, cookie_header);
        }
        if let Some(payload) = payload {
            request = request.json(payload);
        }

        let response = request
            .send()
            .map_err(|error| ApiError::Message(error.to_string()))?;
        let set_cookie_headers = response
            .headers()
            .get_all(SET_COOKIE)
            .iter()
            .filter_map(|value| value.to_str().ok().map(str::to_string))
            .collect::<Vec<_>>();
        if !set_cookie_headers.is_empty() {
            self.store_set_cookie_headers(&set_cookie_headers);
        }
        let status = response.status();
        let body = response
            .text()
            .map_err(|error| ApiError::Message(error.to_string()))?;
        if status != StatusCode::OK {
            if status == StatusCode::UNAUTHORIZED {
                return Err(ApiError::Unauthorized(UnauthorizedError::new(
                    localized_status_message(status, &body),
                )));
            }
            return Err(ApiError::HttpStatus {
                status: status.as_u16(),
                message: localized_status_message(status, &body),
            });
        }
        serde_json::from_str(&body).map_err(|error| {
            ApiError::Message(format!(
                "{} 返回的数据格式无法识别，请稍后再试。（接口：{}，解析错误：{}，返回内容：{}）",
                api_label_for_path(path),
                path,
                error,
                response_body_preview(&body)
            ))
        })
    }

    fn store_set_cookie_headers(&self, set_cookie_headers: &[String]) {
        let mut stored = self.session_cookies.lock().unwrap();
        let existing = stored.clone();
        *stored = merge_session_cookies(&existing, set_cookie_headers);
    }
}

fn response_body_preview(body: &str) -> String {
    let normalized = body.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.is_empty() {
        return "空".to_string();
    }
    const MAX_CHARS: usize = 200;
    let mut preview = normalized.chars().take(MAX_CHARS).collect::<String>();
    if normalized.chars().count() > MAX_CHARS {
        preview.push_str("...");
    }
    preview
}

pub fn is_unauthorized(error: &ApiError) -> bool {
    matches!(error, ApiError::Unauthorized(_))
}

pub fn is_http_status(error: &ApiError, status: u16) -> bool {
    matches!(error, ApiError::HttpStatus { status: code, .. } if *code == status)
}
