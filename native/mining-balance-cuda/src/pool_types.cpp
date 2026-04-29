#include "app/pool_types.hpp"

namespace app {

bool CurrentRound::is_open() const {
    return status.empty() || status == kRoundStatusOpen;
}

bool StatusResponse::daily_limit_reached() const {
    if (result == kResultDailyWinLimitReached) {
        return true;
    }
    return daily_drop_remaining.has_value() && *daily_drop_remaining <= 0;
}

int StatusResponse::balance_inventory_remaining() const {
    if (pool_stats.has_value()) {
        return pool_stats->balance_unused;
    }
    return inventory_remaining;
}

void from_json(const nlohmann::json& j, PoolStats& value) {
    value.balance_unused = j.value("balance_unused", 0);
    value.invite_unused = j.value("invite_unused", 0);
}

void from_json(const nlohmann::json& j, CurrentRound& value) {
    value.id = j.value("id", 0);
    value.difficulty_bits = j.value("difficulty_bits", 0);
    value.expires_at = j.value("expires_at", "");
    value.memory_cost_mb = j.value("memory_cost_mb", 0);
    value.parallelism = j.value("parallelism", 0);
    value.round_key = j.value("round_key", "");
    value.seed = j.value("seed", "");
    value.status = j.value("status", "");
    value.time_cost = j.value("time_cost", 0);
}

void from_json(const nlohmann::json& j, StatusResponse& value) {
    value.admin_lock = j.value("admin_lock", "");
    if (j.contains("current_round") && !j.at("current_round").is_null()) {
        value.current_round = j.at("current_round").get<CurrentRound>();
    } else {
        value.current_round.reset();
    }
    if (j.contains("daily_drop_remaining") && !j.at("daily_drop_remaining").is_null()) {
        value.daily_drop_remaining = j.at("daily_drop_remaining").get<int>();
    } else {
        value.daily_drop_remaining.reset();
    }
    value.desktop_only = j.value("desktop_only", false);
    value.enabled = j.value("enabled", false);
    value.inventory_remaining = j.value("inventory_remaining", 0);
    if (j.contains("pool_stats") && !j.at("pool_stats").is_null()) {
        value.pool_stats = j.at("pool_stats").get<PoolStats>();
    } else {
        value.pool_stats.reset();
    }
    value.result = j.value("result", "");
    value.server_time = j.value("server_time", "");
}

void from_json(const nlohmann::json& j, ChallengeResponse& value) {
    value.admin_lock = j.value("admin_lock", "");
    value.challenge_id = j.value("challenge_id", 0);
    value.difficulty_bits = j.value("difficulty_bits", 0);
    value.expires_at = j.value("expires_at", "");
    value.memory_cost_mb = j.value("memory_cost_mb", 0);
    value.message = j.value("message", "");
    value.ok = j.value("ok", false);
    value.parallelism = j.value("parallelism", 0);
    if (j.contains("pool_stats") && !j.at("pool_stats").is_null()) {
        value.pool_stats = j.at("pool_stats").get<PoolStats>();
    } else {
        value.pool_stats.reset();
    }
    value.result = j.value("result", "");
    value.round_id = j.value("round_id", 0);
    value.seed = j.value("seed", "");
    value.session_salt = j.value("session_salt", "");
    value.time_cost = j.value("time_cost", 0);
    value.visitor_id = j.value("visitor_id", "");
}

void from_json(const nlohmann::json& j, HeartbeatResponse& value) {
    value.result = j.value("result", "");
}

void from_json(const nlohmann::json& j, SubmitResponse& value) {
    value.balance_amount = j.value("balance_amount", 0.0);
    value.code_type = j.value("code_type", "");
    value.drop_type = j.value("drop_type", "");
    value.forced_by = j.value("forced_by", "");
    value.balance_code = j.value("invite_code", "");
    value.ok = j.value("ok", false);
    value.result = j.value("result", "");
    value.reward_code_id = j.value("reward_code_id", 0);
}

void to_json(nlohmann::json& j, const HeartbeatRequest& value) {
    j = nlohmann::json{{"challenge_id", value.challenge_id}, {"round_id", value.round_id}};
}

void to_json(nlohmann::json& j, const SubmitRequest& value) {
    j = nlohmann::json{{"challenge_id", value.challenge_id},
                       {"round_id", value.round_id},
                       {"nonce", value.nonce},
                       {"digest", value.digest},
                       {"preference", value.preference}};
}

} // namespace app
