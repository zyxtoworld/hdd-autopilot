use serde::{Deserialize, Serialize, de::Deserializer};
use serde_json::Value;

#[derive(Debug, Clone, Serialize)]
pub(crate) struct HeartbeatRequest {
    pub(crate) challenge_id: i32,
    pub(crate) round_id: i32,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct SubmitRequest {
    pub(crate) challenge_id: i32,
    pub(crate) round_id: i32,
    pub(crate) nonce: String,
    pub(crate) digest: String,
    pub(crate) preference: String,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct ApiErrorBody {
    #[serde(default, deserialize_with = "deserialize_string_or_default")]
    pub(crate) message: String,
    #[serde(default, deserialize_with = "deserialize_string_or_default")]
    pub(crate) reason: String,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct PoolStats {
    #[serde(default, deserialize_with = "deserialize_i32_or_default")]
    pub(crate) balance_unused: i32,
    #[serde(default, deserialize_with = "deserialize_i32_or_default")]
    pub(crate) invite_unused: i32,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub(crate) struct CurrentRound {
    #[serde(default, deserialize_with = "deserialize_i32_or_default")]
    pub(crate) id: i32,
    #[serde(default, deserialize_with = "deserialize_i32_or_default")]
    pub(crate) difficulty_bits: i32,
    #[serde(default, deserialize_with = "deserialize_string_or_default")]
    pub(crate) expires_at: String,
    #[serde(default, deserialize_with = "deserialize_i32_or_default")]
    pub(crate) memory_cost_mb: i32,
    #[serde(default, deserialize_with = "deserialize_i32_or_default")]
    pub(crate) parallelism: i32,
    #[serde(default, deserialize_with = "deserialize_string_or_default")]
    pub(crate) round_key: String,
    #[serde(default, deserialize_with = "deserialize_string_or_default")]
    pub(crate) seed: String,
    #[serde(default, deserialize_with = "deserialize_string_or_default")]
    pub(crate) status: String,
    #[serde(default, deserialize_with = "deserialize_i32_or_default")]
    pub(crate) time_cost: i32,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub(crate) struct StatusResponse {
    #[serde(default, deserialize_with = "deserialize_string_or_default")]
    pub(crate) admin_lock: String,
    pub(crate) current_round: Option<CurrentRound>,
    #[serde(default, deserialize_with = "deserialize_option_i32")]
    pub(crate) daily_drop_remaining: Option<i32>,
    #[serde(default, deserialize_with = "deserialize_bool_or_default")]
    pub(crate) desktop_only: bool,
    #[serde(default, deserialize_with = "deserialize_bool_or_default")]
    pub(crate) enabled: bool,
    #[serde(default, deserialize_with = "deserialize_i32_or_default")]
    pub(crate) inventory_remaining: i32,
    pub(crate) pool_stats: Option<PoolStats>,
    #[serde(default, deserialize_with = "deserialize_string_or_default")]
    pub(crate) result: String,
    #[serde(default, deserialize_with = "deserialize_string_or_default")]
    pub(crate) server_time: String,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct ChallengeResponse {
    #[serde(default, deserialize_with = "deserialize_bool_or_default")]
    pub(crate) ok: bool,
    #[serde(default, deserialize_with = "deserialize_i32_or_default")]
    pub(crate) challenge_id: i32,
    #[serde(default, deserialize_with = "deserialize_i32_or_default")]
    pub(crate) round_id: i32,
    #[serde(default, deserialize_with = "deserialize_i32_or_default")]
    pub(crate) difficulty_bits: i32,
    #[serde(default, deserialize_with = "deserialize_i32_or_default")]
    pub(crate) memory_cost_mb: i32,
    #[serde(default, deserialize_with = "deserialize_i32_or_default")]
    pub(crate) parallelism: i32,
    #[serde(default, deserialize_with = "deserialize_string_or_default")]
    pub(crate) seed: String,
    #[serde(default, deserialize_with = "deserialize_string_or_default")]
    pub(crate) session_salt: String,
    #[serde(default, deserialize_with = "deserialize_i32_or_default")]
    pub(crate) time_cost: i32,
    #[serde(default, deserialize_with = "deserialize_string_or_default")]
    pub(crate) visitor_id: String,
    #[serde(default, deserialize_with = "deserialize_string_or_default")]
    pub(crate) message: String,
    #[serde(default, deserialize_with = "deserialize_string_or_default")]
    pub(crate) result: String,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct HeartbeatResponse {
    #[serde(default, deserialize_with = "deserialize_string_or_default")]
    pub(crate) result: String,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct SubmitResponse {
    #[serde(default, deserialize_with = "deserialize_f64_or_default")]
    pub(crate) balance_amount: f64,
    #[serde(default, deserialize_with = "deserialize_string_or_default")]
    pub(crate) code_type: String,
    #[serde(
        default,
        alias = "invite_code",
        alias = "balance_code",
        alias = "parallel_code",
        alias = "concurrent_code",
        deserialize_with = "deserialize_string_or_default"
    )]
    pub(crate) reward_code: String,
    #[serde(default, deserialize_with = "deserialize_i32_or_default")]
    pub(crate) concurrency: i32,
    #[serde(default, deserialize_with = "deserialize_string_or_default")]
    pub(crate) result: String,
    #[serde(default, deserialize_with = "deserialize_i32_or_default")]
    pub(crate) reward_code_id: i32,
}

impl StatusResponse {
    pub(crate) fn daily_limit_reached(&self) -> bool {
        self.result
            .trim()
            .eq_ignore_ascii_case("daily win limit reached")
            || self
                .daily_drop_remaining
                .is_some_and(|remaining| remaining <= 0)
    }

    pub(crate) fn invite_inventory_remaining(&self) -> i32 {
        self.pool_stats
            .as_ref()
            .map(|stats| stats.invite_unused)
            .unwrap_or(self.inventory_remaining)
    }

    pub(crate) fn balance_inventory_remaining(&self) -> i32 {
        self.pool_stats
            .as_ref()
            .map(|stats| stats.balance_unused)
            .unwrap_or(self.inventory_remaining)
    }
}

impl CurrentRound {
    pub(crate) fn is_open(&self) -> bool {
        self.status.trim().is_empty() || self.status.trim().eq_ignore_ascii_case("open")
    }
}

fn deserialize_string_or_default<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    Ok(match Option::<Value>::deserialize(deserializer)? {
        None | Some(Value::Null) => String::new(),
        Some(Value::String(value)) => value,
        Some(Value::Bool(value)) => value.to_string(),
        Some(Value::Number(value)) => value.to_string(),
        Some(other) => other.to_string(),
    })
}

fn deserialize_bool_or_default<'de, D>(deserializer: D) -> Result<bool, D::Error>
where
    D: Deserializer<'de>,
{
    Ok(Option::<Value>::deserialize(deserializer)?
        .and_then(parse_bool_value)
        .unwrap_or(false))
}

fn deserialize_i32_or_default<'de, D>(deserializer: D) -> Result<i32, D::Error>
where
    D: Deserializer<'de>,
{
    Ok(Option::<Value>::deserialize(deserializer)?
        .and_then(parse_i32_value)
        .unwrap_or_default())
}

fn deserialize_option_i32<'de, D>(deserializer: D) -> Result<Option<i32>, D::Error>
where
    D: Deserializer<'de>,
{
    Ok(Option::<Value>::deserialize(deserializer)?.and_then(parse_i32_value))
}

fn deserialize_f64_or_default<'de, D>(deserializer: D) -> Result<f64, D::Error>
where
    D: Deserializer<'de>,
{
    Ok(Option::<Value>::deserialize(deserializer)?
        .and_then(parse_f64_value)
        .unwrap_or_default())
}

fn parse_bool_value(value: Value) -> Option<bool> {
    match value {
        Value::Bool(value) => Some(value),
        Value::Number(value) => value.as_i64().map(|value| value != 0),
        Value::String(value) => match value.trim().to_ascii_lowercase().as_str() {
            "true" | "1" | "yes" | "y" | "on" => Some(true),
            "false" | "0" | "no" | "n" | "off" => Some(false),
            _ => None,
        },
        _ => None,
    }
}

fn parse_i32_value(value: Value) -> Option<i32> {
    match value {
        Value::Number(value) => value
            .as_i64()
            .and_then(|value| i32::try_from(value).ok())
            .or_else(|| value.as_u64().and_then(|value| i32::try_from(value).ok()))
            .or_else(|| value.as_f64().map(|value| value as i32)),
        Value::String(value) => value.trim().parse::<i32>().ok(),
        Value::Bool(value) => Some(i32::from(value)),
        _ => None,
    }
}

fn parse_f64_value(value: Value) -> Option<f64> {
    match value {
        Value::Number(value) => value.as_f64(),
        Value::String(value) => value.trim().parse::<f64>().ok(),
        Value::Bool(value) => Some(if value { 1.0 } else { 0.0 }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{StatusResponse, SubmitResponse};

    #[test]
    fn status_sample_deserializes() {
        let response: StatusResponse = serde_json::from_str(
            r#"{
                "admin_lock": "any",
                "current_round": {
                    "difficulty_bits": 16,
                    "expires_at": "Tue, 28 Apr 2026 11:07:20 GMT",
                    "id": 1331,
                    "memory_cost_mb": 128,
                    "parallelism": 1,
                    "round_key": "ee6960249e6b43bb",
                    "seed": "7dea2e90ff46172860a62a476588c363",
                    "status": "open",
                    "time_cost": 3
                },
                "daily_drop_remaining": null,
                "desktop_only": true,
                "enabled": true,
                "inventory_remaining": 396,
                "pool_stats": {
                    "balance_unused": 329,
                    "invite_unused": 67
                },
                "server_time": "2026-04-28T10:07:26.974107+00:00"
            }"#,
        )
        .expect("deserialize status sample");

        assert_eq!(response.admin_lock, "any");
        assert!(response.desktop_only);
        assert!(response.enabled);
        assert_eq!(response.inventory_remaining, 396);
        assert_eq!(response.invite_inventory_remaining(), 67);
        assert_eq!(response.balance_inventory_remaining(), 329);
        assert_eq!(response.server_time, "2026-04-28T10:07:26.974107+00:00");
        assert_eq!(response.daily_drop_remaining, None);

        let round = response.current_round.expect("current round");
        assert_eq!(round.id, 1331);
        assert_eq!(round.difficulty_bits, 16);
        assert_eq!(round.memory_cost_mb, 128);
        assert_eq!(round.parallelism, 1);
        assert_eq!(round.round_key, "ee6960249e6b43bb");
        assert_eq!(round.seed, "7dea2e90ff46172860a62a476588c363");
        assert_eq!(round.time_cost, 3);
        assert!(round.is_open());
    }

    #[test]
    fn stringly_status_fields_deserialize() {
        let response: StatusResponse = serde_json::from_str(
            r#"{
                "admin_lock": 7,
                "current_round": {
                    "difficulty_bits": "16",
                    "id": "1331",
                    "memory_cost_mb": "128",
                    "parallelism": "1",
                    "round_key": 88,
                    "seed": true,
                    "status": "open",
                    "time_cost": "3"
                },
                "daily_drop_remaining": "2",
                "desktop_only": "true",
                "enabled": "1",
                "inventory_remaining": "396",
                "pool_stats": {
                    "balance_unused": "329",
                    "invite_unused": "67"
                },
                "server_time": 123
            }"#,
        )
        .expect("deserialize stringly status sample");

        assert_eq!(response.admin_lock, "7");
        assert!(response.desktop_only);
        assert!(response.enabled);
        assert_eq!(response.daily_drop_remaining, Some(2));
        assert_eq!(response.inventory_remaining, 396);
        assert_eq!(response.invite_inventory_remaining(), 67);
        assert_eq!(response.balance_inventory_remaining(), 329);
        assert_eq!(response.server_time, "123");

        let round = response.current_round.expect("current round");
        assert_eq!(round.id, 1331);
        assert_eq!(round.difficulty_bits, 16);
        assert_eq!(round.memory_cost_mb, 128);
        assert_eq!(round.parallelism, 1);
        assert_eq!(round.round_key, "88");
        assert_eq!(round.seed, "true");
        assert_eq!(round.time_cost, 3);
    }

    #[test]
    fn submit_response_accepts_multiple_reward_code_keys() {
        let invite_response: SubmitResponse = serde_json::from_str(
            r#"{
                "balance_amount": 0,
                "code_type": "invite",
                "invite_code": "INVITE-123",
                "result": "success",
                "reward_code_id": 1
            }"#,
        )
        .expect("deserialize invite response");
        assert_eq!(invite_response.reward_code, "INVITE-123");

        let balance_response: SubmitResponse = serde_json::from_str(
            r#"{
                "balance_amount": "6.66",
                "code_type": "balance",
                "balance_code": "BALANCE-456",
                "result": "accepted",
                "reward_code_id": "2"
            }"#,
        )
        .expect("deserialize balance response");
        assert_eq!(balance_response.reward_code, "BALANCE-456");
        assert_eq!(balance_response.balance_amount, 6.66);
        assert_eq!(balance_response.reward_code_id, 2);

        let concurrent_response: SubmitResponse = serde_json::from_str(
            r#"{
                "balance_amount": 0,
                "code_type": "concurrent",
                "concurrent_code": "CONCURRENT-789",
                "concurrency": "4",
                "result": "accepted",
                "reward_code_id": "3"
            }"#,
        )
        .expect("deserialize concurrent response");
        assert_eq!(concurrent_response.reward_code, "CONCURRENT-789");
        assert_eq!(concurrent_response.concurrency, 4);
        assert_eq!(concurrent_response.reward_code_id, 3);
    }
}
