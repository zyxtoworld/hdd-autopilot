use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct CheckinUser {
    #[serde(default)]
    pub balance: f64,
    pub email: String,
    pub id: i32,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct CheckinMeResponse {
    pub authenticated: bool,
    pub user: CheckinUser,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct CheckinTodayResponse {
    #[serde(default)]
    pub claim_date: String,
    #[serde(default)]
    pub claimed: bool,
    #[serde(default)]
    pub claimed_at: String,
    #[serde(default)]
    pub reward_amount: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct CheckinClaimResponse {
    #[serde(default)]
    pub already_claimed: bool,
    #[serde(default)]
    pub claim_date: String,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub ok: bool,
    #[serde(default)]
    pub reward_amount: f64,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct CheckinResult {
    pub email: String,
    pub status: String,
    pub success: bool,
    pub delta: f64,
    pub balance_after: f64,
    pub when_unix_ms: i64,
    pub error_message: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checkin_today_response_accepts_missing_claimed_at() {
        let response: CheckinTodayResponse = serde_json::from_str(
            r#"{"claim_date":"2026-04-26","claimed":false,"reward_amount":1.5}"#,
        )
        .unwrap();

        assert_eq!(response.claim_date, "2026-04-26");
        assert!(!response.claimed);
        assert_eq!(response.claimed_at, "");
        assert_eq!(response.reward_amount, 1.5);
    }
}
