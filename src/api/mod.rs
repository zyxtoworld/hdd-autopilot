mod client;
mod endpoints;

use std::fmt;
#[cfg(target_os = "macos")]
use std::process::Command;

use reqwest::blocking::Client;

pub const DEFAULT_BASE_URL: &str = "https://sub.hdd.sb";
pub const AUTH_ME_PATH: &str = "/api/v1/auth/me";
pub const LOGIN_PATH: &str = "/api/v1/auth/login";
pub const CHECKIN_ME_PATH: &str = "/checkin-api/me";
pub const CHECKIN_TODAY_PATH: &str = "/checkin-api/today";
pub const CHECKIN_CLAIM_PATH: &str = "/checkin-api/claim";
pub const ARROW_OUT_CONFIG_PATH: &str = "/arrow-out-api/config";
pub const ARROW_OUT_HISTORY_PATH: &str = "/arrow-out-api/history";
pub const ARROW_OUT_ME_PATH: &str = "/arrow-out-api/me";
pub const ARROW_OUT_START_PATH: &str = "/arrow-out-api/start";
pub const ARROW_OUT_FINISH_PATH: &str = "/arrow-out-api/finish";
pub const ARROW_OUT_ABANDON_PATH: &str = "/arrow-out-api/abandon";
pub const SCRATCH_PLAY_PATH: &str = "/scratch-api/play";
pub const SCRATCH_REVEAL_PATH: &str = "/scratch-api/reveal";
pub const SCRATCH_HISTORY_PATH: &str = "/scratch-api/history";
pub const TILE_CONFIG_PATH: &str = "/tile-api/config";
pub const TILE_HISTORY_PATH: &str = "/tile-api/history";
pub const TILE_ME_PATH: &str = "/tile-api/me";
pub const TILE_START_PATH: &str = "/tile-api/start";
pub const TILE_STEP_PATH: &str = "/tile-api/step";
pub const TILE_ABANDON_PATH: &str = "/tile-api/abandon";
pub const PUZZLE_2048_CONFIG_PATH: &str = "/puzzle2048-api/config";
pub const PUZZLE_2048_HISTORY_PATH: &str = "/puzzle2048-api/history";
pub const PUZZLE_2048_ME_PATH: &str = "/puzzle2048-api/me";
pub const PUZZLE_2048_START_PATH: &str = "/puzzle2048-api/start";
pub const PUZZLE_2048_MOVE_PATH: &str = "/puzzle2048-api/move";
pub const PUZZLE_2048_ABANDON_PATH: &str = "/puzzle2048-api/abandon";
pub const MEMORY_CONFIG_PATH: &str = "/memory-api/config";
pub const MEMORY_HISTORY_PATH: &str = "/memory-api/history";
pub const MEMORY_ME_PATH: &str = "/memory-api/me";
pub const MEMORY_START_PATH: &str = "/memory-api/start";
pub const MEMORY_FLIP_PATH: &str = "/memory-api/flip";
pub const MINESWEEPER_CONFIG_PATH: &str = "/minesweeper-api/config";
pub const MINESWEEPER_HISTORY_PATH: &str = "/minesweeper-api/history";
pub const MINESWEEPER_ME_PATH: &str = "/minesweeper-api/me";
pub const MINESWEEPER_START_PATH: &str = "/minesweeper-api/start";
pub const MINESWEEPER_CLICK_PATH: &str = "/minesweeper-api/click";
pub const SOKOBAN_CONFIG_PATH: &str = "/sokoban-api/config";
pub const SOKOBAN_HISTORY_PATH: &str = "/sokoban-api/history";
pub const SOKOBAN_ME_PATH: &str = "/sokoban-api/me";
pub const SOKOBAN_START_PATH: &str = "/sokoban-api/start";
pub const SOKOBAN_MOVE_PATH: &str = "/sokoban-api/move";
pub const LIGHTSOUT_CONFIG_PATH: &str = "/lightsout-api/config";
pub const LIGHTSOUT_HISTORY_PATH: &str = "/lightsout-api/history";
pub const LIGHTSOUT_ME_PATH: &str = "/lightsout-api/me";
pub const LIGHTSOUT_START_PATH: &str = "/lightsout-api/start";
pub const LIGHTSOUT_CLICK_PATH: &str = "/lightsout-api/click";
pub const MAZE_CONFIG_PATH: &str = "/maze-api/config";
pub const MAZE_HISTORY_PATH: &str = "/maze-api/history";
pub const MAZE_ME_PATH: &str = "/maze-api/me";
pub const MAZE_START_PATH: &str = "/maze-api/start";
pub const MAZE_MOVE_PATH: &str = "/maze-api/move";
pub const NONOGRAM_CONFIG_PATH: &str = "/nonogram-api/config";
pub const NONOGRAM_HISTORY_PATH: &str = "/nonogram-api/history";
pub const NONOGRAM_ME_PATH: &str = "/nonogram-api/me";
pub const NONOGRAM_START_PATH: &str = "/nonogram-api/start";
pub const NONOGRAM_CLICK_PATH: &str = "/nonogram-api/click";
pub const NONOGRAM_FINISH_PATH: &str = "/nonogram-api/finish";
pub const FLOWFREE_CONFIG_PATH: &str = "/flowfree-api/config";
pub const FLOWFREE_HISTORY_PATH: &str = "/flowfree-api/history";
pub const FLOWFREE_ME_PATH: &str = "/flowfree-api/me";
pub const FLOWFREE_START_PATH: &str = "/flowfree-api/start";
pub const FLOWFREE_FINISH_PATH: &str = "/flowfree-api/finish";
pub const FLOWFREE_ABANDON_PATH: &str = "/flowfree-api/abandon";
pub const PUZZLE_15_CONFIG_PATH: &str = "/puzzle15-api/config";
pub const PUZZLE_15_HISTORY_PATH: &str = "/puzzle15-api/history";
pub const PUZZLE_15_ME_PATH: &str = "/puzzle15-api/me";
pub const PUZZLE_15_START_PATH: &str = "/puzzle15-api/start";
pub const PUZZLE_15_MOVE_PATH: &str = "/puzzle15-api/move";
pub const SUDOKU_CONFIG_PATH: &str = "/sudoku-api/config";
pub const SUDOKU_HISTORY_PATH: &str = "/sudoku-api/history";
pub const SUDOKU_ME_PATH: &str = "/sudoku-api/me";
pub const SUDOKU_START_PATH: &str = "/sudoku-api/start";
pub const SUDOKU_FILL_PATH: &str = "/sudoku-api/fill";
const DEFAULT_USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/147.0.0.0 Safari/537.36 Edg/147.0.0.0";

pub fn auth_me_path() -> String {
    auth_me_path_for_timezone(&system_timezone_name())
}

#[cfg(not(target_os = "macos"))]
fn system_timezone_name() -> String {
    iana_time_zone::get_timezone()
        .ok()
        .filter(|timezone| !timezone.trim().is_empty())
        .unwrap_or_else(|| "Etc/UTC".to_string())
}

#[cfg(target_os = "macos")]
fn system_timezone_name() -> String {
    std::env::var("TZ")
        .ok()
        .and_then(|timezone| normalize_timezone_name(&timezone))
        .or_else(macos_system_timezone_name)
        .unwrap_or_else(|| "Etc/UTC".to_string())
}

#[cfg(target_os = "macos")]
fn macos_system_timezone_name() -> Option<String> {
    let output = Command::new("systemsetup")
        .arg("-gettimezone")
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8(output.stdout).ok()?;
    let timezone = stdout
        .trim()
        .strip_prefix("Time Zone:")
        .unwrap_or(stdout.trim());
    normalize_timezone_name(timezone)
}

#[cfg(target_os = "macos")]
fn normalize_timezone_name(timezone: &str) -> Option<String> {
    let timezone = timezone.trim();
    (!timezone.is_empty()).then(|| timezone.to_string())
}

fn auth_me_path_for_timezone(timezone: &str) -> String {
    let timezone = timezone.trim();
    let timezone = if timezone.is_empty() {
        "Etc/UTC"
    } else {
        timezone
    };
    let encoded_timezone =
        url::form_urlencoded::byte_serialize(timezone.as_bytes()).collect::<String>();
    format!("{AUTH_ME_PATH}?timezone={encoded_timezone}")
}

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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auth_me_path_encodes_system_timezone_name() {
        assert_eq!(
            auth_me_path_for_timezone("America/New_York"),
            "/api/v1/auth/me?timezone=America%2FNew_York"
        );
    }

    #[test]
    fn auth_me_path_falls_back_when_timezone_is_empty() {
        assert_eq!(
            auth_me_path_for_timezone("   "),
            "/api/v1/auth/me?timezone=Etc%2FUTC"
        );
    }
}
