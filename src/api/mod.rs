mod client;
mod cookies;
mod endpoints;

use std::fmt;
use std::sync::{Arc, Mutex};

use crate::model::SessionCookie;
use reqwest::blocking::Client;

pub const DEFAULT_BASE_URL: &str = "https://sub.hdd.sb";
pub const AUTH_ME_PATH: &str = "/api/v1/auth/me?timezone=Asia%2FShanghai";
pub const LOGIN_PATH: &str = "/api/v1/auth/login";
pub const CHECKIN_ME_PATH: &str = "/checkin-api/me";
pub const CHECKIN_TODAY_PATH: &str = "/checkin-api/today";
pub const CHECKIN_CLAIM_PATH: &str = "/checkin-api/claim";
pub const SCRATCH_PLAY_PATH: &str = "/scratch-api/play";
pub const SCRATCH_REVEAL_PATH: &str = "/scratch-api/reveal";
pub const SCRATCH_HISTORY_PATH: &str = "/scratch-api/history";
pub const TILE_CONFIG_PATH: &str = "/tile-api/config";
pub const TILE_HISTORY_PATH: &str = "/tile-api/history";
pub const TILE_ME_PATH: &str = "/tile-api/me";
pub const TILE_START_PATH: &str = "/tile-api/start";
pub const TILE_STEP_PATH: &str = "/tile-api/step";
pub const TILE_ABANDON_PATH: &str = "/tile-api/abandon";
const DEFAULT_USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/147.0.0.0 Safari/537.36 Edg/147.0.0.0";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnauthorizedError {
    message: String,
}

impl UnauthorizedError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for UnauthorizedError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for UnauthorizedError {}

#[derive(Debug)]
pub enum ApiError {
    Unauthorized(UnauthorizedError),
    HttpStatus { status: u16, message: String },
    Message(String),
}

#[derive(Debug, serde::Deserialize)]
pub(crate) struct ApiErrorBody {
    #[serde(default)]
    code: i32,
    #[serde(default)]
    message: String,
    #[serde(default)]
    reason: String,
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unauthorized(error) => error.fmt(f),
            Self::HttpStatus { message, .. } | Self::Message(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for ApiError {}

pub use self::client::{is_http_status, is_unauthorized};

#[derive(Debug, Clone)]
pub struct ApiClient {
    base_url: String,
    http_client: Client,
    session_cookies: Arc<Mutex<Vec<SessionCookie>>>,
}
