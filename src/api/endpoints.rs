use reqwest::StatusCode;

use super::{
    AUTH_ME_PATH, ApiErrorBody, CHECKIN_CLAIM_PATH, CHECKIN_ME_PATH, CHECKIN_TODAY_PATH,
    LOGIN_PATH, MEMORY_CONFIG_PATH, MEMORY_FLIP_PATH, MEMORY_HISTORY_PATH, MEMORY_START_PATH,
    PUZZLE_15_CONFIG_PATH, PUZZLE_15_HISTORY_PATH, PUZZLE_15_MOVE_PATH, PUZZLE_15_START_PATH,
    PUZZLE_2048_ABANDON_PATH, PUZZLE_2048_CONFIG_PATH, PUZZLE_2048_HISTORY_PATH,
    PUZZLE_2048_MOVE_PATH, PUZZLE_2048_START_PATH, SCRATCH_HISTORY_PATH, SCRATCH_PLAY_PATH,
    SCRATCH_REVEAL_PATH, SUDOKU_CONFIG_PATH, SUDOKU_FILL_PATH, SUDOKU_HISTORY_PATH,
    SUDOKU_START_PATH, TILE_ABANDON_PATH, TILE_CONFIG_PATH, TILE_HISTORY_PATH, TILE_ME_PATH,
    TILE_START_PATH, TILE_STEP_PATH,
};

pub(super) fn api_label_for_path(path: &str) -> &'static str {
    match path {
        CHECKIN_ME_PATH | CHECKIN_TODAY_PATH | CHECKIN_CLAIM_PATH => "签到接口",
        SCRATCH_PLAY_PATH | SCRATCH_REVEAL_PATH | SCRATCH_HISTORY_PATH => "刮刮乐接口",
        TILE_CONFIG_PATH | TILE_HISTORY_PATH | TILE_ME_PATH | TILE_START_PATH | TILE_STEP_PATH
        | TILE_ABANDON_PATH => "羊了个羊接口",
        PUZZLE_2048_CONFIG_PATH
        | PUZZLE_2048_HISTORY_PATH
        | PUZZLE_2048_START_PATH
        | PUZZLE_2048_MOVE_PATH
        | PUZZLE_2048_ABANDON_PATH => "谜题2048接口",
        MEMORY_CONFIG_PATH | MEMORY_HISTORY_PATH | MEMORY_START_PATH | MEMORY_FLIP_PATH => {
            "记忆翻牌接口"
        }
        PUZZLE_15_CONFIG_PATH
        | PUZZLE_15_HISTORY_PATH
        | PUZZLE_15_START_PATH
        | PUZZLE_15_MOVE_PATH => "华容道接口",
        SUDOKU_CONFIG_PATH | SUDOKU_HISTORY_PATH | SUDOKU_START_PATH | SUDOKU_FILL_PATH => {
            "数独接口"
        }
        LOGIN_PATH | AUTH_ME_PATH => "登录接口",
        _ => "服务端接口",
    }
}

pub(super) fn localized_status_message(status: StatusCode, body: &str) -> String {
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
                localized_visible_text(api_error.message.trim(), "服务端返回了错误")
            );
        }
        if api_error.code != 0 {
            return format!(
                "请求失败了（状态码 {}）：服务端错误码 {}",
                status.as_u16(),
                api_error.code
            );
        }
    }
    if status == StatusCode::UNAUTHORIZED {
        return "登录状态已失效，请重新登录".to_string();
    }
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return format!("请求失败了（状态码 {}）", status.as_u16());
    }
    format!(
        "请求失败了（状态码 {}）：{}",
        status.as_u16(),
        localized_visible_text(trimmed, "服务端返回了错误")
    )
}

fn localized_visible_text(text: &str, fallback: &str) -> String {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return fallback.to_string();
    }
    let lower = trimmed.to_ascii_lowercase();
    match lower.as_str() {
        "invalid email or password" => "邮箱或密码错误".to_string(),
        _ if lower.contains("daily limit reached") => "今天这个难度的次数已经用完了".to_string(),
        _ if lower.contains("active session") || lower.contains("max active") => {
            "当前还有未结束对局，需要先续玩残局".to_string()
        }
        _ if lower.contains("invalid direction") => "移动方向无效".to_string(),
        _ if lower.contains("game over") || lower.contains("already ended") => {
            "当前对局已经结束".to_string()
        }
        _ if lower.contains("unauthorized") || lower.contains("invalid token") => {
            "登录状态已失效，请重新登录".to_string()
        }
        _ if lower.contains("slot full") => "槽位已满".to_string(),
        _ if lower.contains("tile not on board") => "目标方块已不在棋盘上".to_string(),
        _ if lower.contains("tile is covered") => "目标方块被遮挡，当前不能点击".to_string(),
        _ if lower.contains("session not found") => "当前对局已失效".to_string(),
        _ if lower.contains("invalid action") => "当前操作已经失效".to_string(),
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
        MEMORY_FLIP_PATH, PUZZLE_15_MOVE_PATH, PUZZLE_2048_MOVE_PATH, SCRATCH_PLAY_PATH,
        SUDOKU_FILL_PATH, TILE_STEP_PATH,
    };

    #[test]
    fn api_label_for_path_uses_endpoint_group_name() {
        assert_eq!(api_label_for_path(SCRATCH_PLAY_PATH), "刮刮乐接口");
        assert_eq!(api_label_for_path(TILE_STEP_PATH), "羊了个羊接口");
        assert_eq!(api_label_for_path(PUZZLE_2048_MOVE_PATH), "谜题2048接口");
        assert_eq!(api_label_for_path(MEMORY_FLIP_PATH), "记忆翻牌接口");
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
