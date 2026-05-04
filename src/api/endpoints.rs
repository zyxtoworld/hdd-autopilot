use reqwest::StatusCode;

use super::{
    AUTH_ME_PATH, ApiErrorBody, CHECKIN_CLAIM_PATH, CHECKIN_ME_PATH, CHECKIN_TODAY_PATH,
    LOGIN_PATH, MEMORY_CONFIG_PATH, MEMORY_FLIP_PATH, MEMORY_HISTORY_PATH, MEMORY_ME_PATH,
    MEMORY_START_PATH, MINESWEEPER_CLICK_PATH, MINESWEEPER_CONFIG_PATH, MINESWEEPER_HISTORY_PATH,
    MINESWEEPER_ME_PATH, MINESWEEPER_START_PATH, PUZZLE_15_CONFIG_PATH, PUZZLE_15_HISTORY_PATH,
    PUZZLE_15_ME_PATH, PUZZLE_15_MOVE_PATH, PUZZLE_15_START_PATH, PUZZLE_2048_ABANDON_PATH,
    PUZZLE_2048_CONFIG_PATH, PUZZLE_2048_HISTORY_PATH, PUZZLE_2048_ME_PATH, PUZZLE_2048_MOVE_PATH,
    PUZZLE_2048_START_PATH, SCRATCH_HISTORY_PATH, SCRATCH_PLAY_PATH, SCRATCH_REVEAL_PATH,
    SUDOKU_CONFIG_PATH, SUDOKU_FILL_PATH, SUDOKU_HISTORY_PATH, SUDOKU_ME_PATH, SUDOKU_START_PATH,
    TILE_ABANDON_PATH, TILE_CONFIG_PATH, TILE_HISTORY_PATH, TILE_ME_PATH, TILE_START_PATH,
    TILE_STEP_PATH,
};

pub(super) fn api_label_for_path(path: &str) -> &'static str {
    if path.starts_with("/sokoban-api/") {
        return "推箱子接口";
    }
    if path.starts_with("/lightsout-api/") {
        return "点灯接口";
    }
    if path.starts_with("/maze-api/") {
        return "迷宫接口";
    }
    if path.starts_with("/nonogram-api/") {
        return "数织接口";
    }
    if path.starts_with("/flowfree-api/") {
        return "连线接口";
    }
    match path {
        CHECKIN_ME_PATH | CHECKIN_TODAY_PATH | CHECKIN_CLAIM_PATH => "签到接口",
        SCRATCH_PLAY_PATH | SCRATCH_REVEAL_PATH | SCRATCH_HISTORY_PATH => "刮刮乐接口",
        TILE_CONFIG_PATH | TILE_HISTORY_PATH | TILE_ME_PATH | TILE_START_PATH | TILE_STEP_PATH
        | TILE_ABANDON_PATH => "羊了个羊接口",
        PUZZLE_2048_CONFIG_PATH
        | PUZZLE_2048_HISTORY_PATH
        | PUZZLE_2048_ME_PATH
        | PUZZLE_2048_START_PATH
        | PUZZLE_2048_MOVE_PATH
        | PUZZLE_2048_ABANDON_PATH => "谜题2048接口",
        MEMORY_CONFIG_PATH | MEMORY_HISTORY_PATH | MEMORY_ME_PATH | MEMORY_START_PATH
        | MEMORY_FLIP_PATH => "记忆翻牌接口",
        MINESWEEPER_CONFIG_PATH
        | MINESWEEPER_HISTORY_PATH
        | MINESWEEPER_ME_PATH
        | MINESWEEPER_START_PATH
        | MINESWEEPER_CLICK_PATH => "扫雷接口",
        PUZZLE_15_CONFIG_PATH
        | PUZZLE_15_HISTORY_PATH
        | PUZZLE_15_ME_PATH
        | PUZZLE_15_START_PATH
        | PUZZLE_15_MOVE_PATH => "华容道接口",
        SUDOKU_CONFIG_PATH | SUDOKU_HISTORY_PATH | SUDOKU_ME_PATH | SUDOKU_START_PATH
        | SUDOKU_FILL_PATH => "数独接口",
        LOGIN_PATH | AUTH_ME_PATH => "登录接口",
        _ => "服务端接口",
    }
}

pub(super) fn localized_status_message(status: StatusCode, body: &str) -> String {
    let fallback = fallback_status_detail(status);
    if let Ok(api_error) = serde_json::from_str::<ApiErrorBody>(body) {
        if status == StatusCode::UNAUTHORIZED
            && (api_error.reason == "INVALID_CREDENTIALS"
                || api_error
                    .message
                    .eq_ignore_ascii_case("invalid email or password"))
        {
            return "邮箱或密码错误".to_string();
        }
        if status == StatusCode::UNAUTHORIZED {
            return "登录状态已失效，请重新登录".to_string();
        }
        if !api_error.message.trim().is_empty() {
            return format!(
                "请求失败了（状态码 {}）：{}",
                status.as_u16(),
                localized_visible_text(api_error.message.trim(), fallback)
            );
        }
        if !api_error.reason.trim().is_empty() {
            return format!(
                "请求失败了（状态码 {}）：{}",
                status.as_u16(),
                localized_visible_text(api_error.reason.trim(), fallback)
            );
        }
        if api_error.code != 0 {
            return format!(
                "请求失败了（状态码 {}）：{}（服务端错误码 {}）",
                status.as_u16(),
                fallback,
                api_error.code
            );
        }
    }
    if status == StatusCode::UNAUTHORIZED {
        return "登录状态已失效，请重新登录".to_string();
    }
    if let Some(message) = visible_error_text_from_json(body, fallback) {
        return format!("请求失败了（状态码 {}）：{}", status.as_u16(), message);
    }
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return format!("请求失败了（状态码 {}）：{}", status.as_u16(), fallback);
    }
    format!(
        "请求失败了（状态码 {}）：{}",
        status.as_u16(),
        localized_visible_text(trimmed, fallback)
    )
}

fn fallback_status_detail(status: StatusCode) -> &'static str {
    match status {
        StatusCode::BAD_REQUEST => "请求参数不符合服务端要求",
        StatusCode::CONFLICT => {
            "请求状态冲突：可能已有未结束对局、重复提交了一步，或服务端状态还没同步"
        }
        StatusCode::TOO_MANY_REQUESTS => "请求太频繁，服务端要求稍后再试",
        _ if status.is_server_error() => "服务端暂时异常",
        _ => "服务端返回了错误",
    }
}

fn visible_error_text_from_json(body: &str, fallback: &str) -> Option<String> {
    let value = serde_json::from_str::<serde_json::Value>(body).ok()?;
    visible_error_text_from_value(&value, fallback)
}

fn visible_error_text_from_value(value: &serde_json::Value, fallback: &str) -> Option<String> {
    match value {
        serde_json::Value::String(text) => {
            let text = localized_visible_text(text, fallback);
            if text.trim().is_empty() {
                None
            } else {
                Some(text)
            }
        }
        serde_json::Value::Object(fields) => {
            for key in ["message", "error", "detail", "description", "reason"] {
                if let Some(message) = fields
                    .get(key)
                    .and_then(|value| visible_error_text_from_value(value, fallback))
                {
                    return Some(message);
                }
            }
            None
        }
        _ => None,
    }
}

fn localized_visible_text(text: &str, fallback: &str) -> String {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return fallback.to_string();
    }
    let lower = trimmed.to_ascii_lowercase();
    let normalized = lower.replace(['_', '-'], " ");
    match lower.as_str() {
        "invalid email or password" => "邮箱或密码错误".to_string(),
        "conflict" => fallback.to_string(),
        _ if normalized.contains("daily limit")
            || normalized.contains("no remaining plays")
            || normalized.contains("remaining plays exhausted") =>
        {
            "今天这个难度的次数已经用完了".to_string()
        }
        _ if normalized.contains("active session") || normalized.contains("max active") => {
            "当前还有未结束对局，需要先续玩残局".to_string()
        }
        _ if normalized.contains("invalid direction") => "移动方向无效".to_string(),
        _ if normalized.contains("game over") || normalized.contains("already ended") => {
            "当前对局已经结束".to_string()
        }
        _ if normalized.contains("unauthorized") || normalized.contains("invalid token") => {
            "登录状态已失效，请重新登录".to_string()
        }
        _ if normalized.contains("slot full") => "槽位已满".to_string(),
        _ if normalized.contains("tile not on board") => "目标方块已不在棋盘上".to_string(),
        _ if normalized.contains("tile is covered") => "目标方块被遮挡，当前不能点击".to_string(),
        _ if normalized.contains("session not found")
            || normalized.contains("no active session") =>
        {
            "当前对局已失效".to_string()
        }
        _ if normalized.contains("invalid action") => "当前操作已经失效".to_string(),
        _ if contains_ascii_alpha(trimmed) => fallback.to_string(),
        _ => trimmed.to_string(),
    }
}

fn contains_ascii_alpha(text: &str) -> bool {
    text.bytes().any(|byte| byte.is_ascii_alphabetic())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::{
        MEMORY_FLIP_PATH, MINESWEEPER_CLICK_PATH, PUZZLE_15_MOVE_PATH, PUZZLE_2048_MOVE_PATH,
        SCRATCH_PLAY_PATH, SUDOKU_FILL_PATH, TILE_STEP_PATH,
    };

    #[test]
    fn api_label_for_path_uses_endpoint_group_name() {
        assert_eq!(api_label_for_path(SCRATCH_PLAY_PATH), "刮刮乐接口");
        assert_eq!(api_label_for_path(TILE_STEP_PATH), "羊了个羊接口");
        assert_eq!(api_label_for_path(PUZZLE_2048_MOVE_PATH), "谜题2048接口");
        assert_eq!(api_label_for_path(MEMORY_FLIP_PATH), "记忆翻牌接口");
        assert_eq!(api_label_for_path(MINESWEEPER_CLICK_PATH), "扫雷接口");
        assert_eq!(api_label_for_path(PUZZLE_15_MOVE_PATH), "华容道接口");
        assert_eq!(api_label_for_path(SUDOKU_FILL_PATH), "数独接口");
    }

    #[test]
    fn localized_status_message_keeps_known_sheepmatch_conflicts() {
        let slot_full =
            localized_status_message(StatusCode::CONFLICT, r#"{"message":"slot full"}"#);
        let stale =
            localized_status_message(StatusCode::CONFLICT, r#"{"message":"tile is covered"}"#);

        assert!(slot_full.contains("槽位已满"));
        assert!(stale.contains("目标方块被遮挡"));
    }

    #[test]
    fn localized_status_message_keeps_known_puzzle_2048_errors() {
        let active = localized_status_message(
            StatusCode::CONFLICT,
            r#"{"message":"max active sessions reached"}"#,
        );
        let invalid_direction = localized_status_message(
            StatusCode::BAD_REQUEST,
            r#"{"message":"invalid direction"}"#,
        );

        assert!(active.contains("未结束对局"));
        assert!(invalid_direction.contains("移动方向无效"));
    }

    #[test]
    fn localized_status_message_explains_plain_conflict() {
        let message = localized_status_message(StatusCode::CONFLICT, r#"{"error":"conflict"}"#);

        assert!(message.contains("请求状态冲突"));
        assert!(message.contains("未结束对局"));
    }

    #[test]
    fn localized_status_message_reads_extra_error_fields() {
        let daily_limit = localized_status_message(
            StatusCode::TOO_MANY_REQUESTS,
            r#"{"detail":"DAILY_LIMIT_REACHED"}"#,
        );
        let rate_limited = localized_status_message(StatusCode::TOO_MANY_REQUESTS, "");

        assert!(daily_limit.contains("次数已经用完"));
        assert!(rate_limited.contains("请求太频繁"));
    }

    #[test]
    fn decode_error_mentions_endpoint_group_and_path() {
        let message = format!(
            "{} 返回的数据格式无法识别，请稍后再试。（接口：{}，解析错误：demo）",
            api_label_for_path(TILE_STEP_PATH),
            TILE_STEP_PATH
        );

        assert!(message.contains("羊了个羊接口"));
        assert!(message.contains(TILE_STEP_PATH));
    }
}
