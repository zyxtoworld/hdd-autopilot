use serde::{Deserialize, Serialize};

pub const SCRATCH_GAME_TYPE_LUCKY_NUMBERS: &str = "lucky-numbers";
pub const SCRATCH_GAME_TYPE_THREE_KIND: &str = "three-kind";
pub const SCRATCH_GAME_TYPE_ICON_MATCH: &str = "icon-match";
pub const SCRATCH_GAME_TYPE_TREASURE_CHEST: &str = "treasure-chests";
pub const SCRATCH_GAME_TYPE_PROGRESS_RUN: &str = "progress-run";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ScratchPlayRequest {
    pub game_type: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ScratchRevealRequest {
    pub play_id: i32,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub reveal_token: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ScratchNumber {
    pub matched: bool,
    pub prize_label: String,
    pub value: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ScratchCell {
    pub label: String,
    pub winning: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ScratchIconCell {
    pub badge: String,
    pub icon: String,
    pub winning: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ScratchChest {
    pub tone: String,
    pub value: String,
    pub winning: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ScratchCheckpoint {
    pub label: String,
    pub state: String,
    pub winning: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ScratchTicketPayload {
    #[serde(default)]
    pub layout: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub subtitle: String,
    #[serde(default)]
    pub lucky_numbers: Vec<i32>,
    #[serde(default)]
    pub numbers: Vec<ScratchNumber>,
    #[serde(default)]
    pub cells: Vec<ScratchCell>,
    #[serde(default)]
    pub winning_indexes: Vec<i32>,
    #[serde(default)]
    pub icons: Vec<ScratchIconCell>,
    #[serde(default)]
    pub winning_icon: Option<String>,
    #[serde(default)]
    pub chests: Vec<ScratchChest>,
    #[serde(default)]
    pub checkpoints: Vec<ScratchCheckpoint>,
    #[serde(default)]
    pub finish_index: i32,
    #[serde(default)]
    pub reward_amount: Option<f64>,
    #[serde(default)]
    pub reward_label: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ScratchPlayResponse {
    #[serde(default)]
    pub balance: f64,
    #[serde(default)]
    pub cost_amount: f64,
    #[serde(default)]
    pub earliest_reveal_at_ms: i64,
    #[serde(default)]
    pub game_type: String,
    #[serde(default)]
    pub issued_at_ms: i64,
    #[serde(default)]
    pub min_scratch_ms: i32,
    #[serde(default)]
    pub play_id: i32,
    #[serde(default)]
    pub reveal_token: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub ticket_payload: ScratchTicketPayload,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ScratchRevealResponse {
    #[serde(default)]
    pub balance: f64,
    #[serde(default)]
    pub game_type: String,
    #[serde(default)]
    pub net_amount: f64,
    #[serde(default)]
    pub play_id: i32,
    #[serde(default)]
    pub reward_amount: f64,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub ticket_payload: ScratchTicketPayload,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ScratchHistoryItem {
    #[serde(default)]
    pub cost_amount: Option<f64>,
    #[serde(default)]
    pub id: i32,
    #[serde(default)]
    pub net_amount: Option<f64>,
    #[serde(default)]
    pub reward_amount: Option<f64>,
    #[serde(default)]
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ScratchHistoryResponse {
    #[serde(default)]
    pub items: Vec<ScratchHistoryItem>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct ScratchRoundResult {
    pub round: i32,
    pub duration_ms: i64,
    pub play_resp: Option<ScratchPlayResponse>,
    pub reveal_resp: Option<ScratchRevealResponse>,
    pub play_history_attempts: i32,
    pub reveal_history_attempts: i32,
    pub play_history_item: Option<ScratchHistoryItem>,
    pub reveal_history_item: Option<ScratchHistoryItem>,
    pub play_error_message: String,
    pub play_history_error_message: String,
    pub reveal_error_message: String,
    pub reveal_history_error_message: String,
}

pub fn scratch_reveal_ready_at(play_resp: &ScratchPlayResponse) -> i64 {
    if play_resp.earliest_reveal_at_ms > 0 {
        return play_resp.earliest_reveal_at_ms;
    }
    let fallback_ms = play_resp.issued_at_ms + i64::from(play_resp.min_scratch_ms.max(0));
    if fallback_ms > 0 {
        return fallback_ms;
    }
    i64::from(play_resp.min_scratch_ms.max(0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scratch_play_response_accepts_sparse_ticket_payload() {
        let response: ScratchPlayResponse = serde_json::from_str(
            r#"{"balance":12.0,"cost_amount":1.0,"earliest_reveal_at_ms":0,"game_type":"lucky-numbers","issued_at_ms":0,"min_scratch_ms":0,"play_id":10,"reveal_token":"token","status":"pending","ticket_payload":{"layout":"","title":"","subtitle":""}}"#,
        )
        .unwrap();

        assert_eq!(response.play_id, 10);
        assert_eq!(response.ticket_payload.layout, "");
        assert!(response.ticket_payload.numbers.is_empty());
        assert_eq!(response.ticket_payload.finish_index, 0);
        assert_eq!(response.ticket_payload.reward_label, "");
    }

    #[test]
    fn scratch_reveal_response_accepts_sparse_ticket_payload() {
        let response: ScratchRevealResponse = serde_json::from_str(
            r#"{"balance":13.0,"game_type":"lucky-numbers","net_amount":0.0,"play_id":10,"reward_amount":1.0,"status":"revealed","ticket_payload":{"layout":"","title":"","subtitle":""}}"#,
        )
        .unwrap();

        assert_eq!(response.play_id, 10);
        assert_eq!(response.ticket_payload.title, "");
        assert!(response.ticket_payload.icons.is_empty());
    }

    #[test]
    fn scratch_history_item_accepts_null_amounts() {
        let item: ScratchHistoryItem = serde_json::from_str(
            r#"{"id":10,"cost_amount":null,"reward_amount":null,"net_amount":null,"status":"done"}"#,
        )
        .unwrap();

        assert_eq!(item.id, 10);
        assert_eq!(item.cost_amount, None);
        assert_eq!(item.reward_amount, None);
        assert_eq!(item.net_amount, None);
    }
}
