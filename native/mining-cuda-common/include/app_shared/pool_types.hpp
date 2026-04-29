#pragma once

#include <optional>
#include <string>

#include <nlohmann/json.hpp>

#include "app_shared/reward_policy.hpp"

namespace app {

inline constexpr const char* kResultDailyWinLimitReached = "daily win limit reached";
inline constexpr const char* kResultRoundClosed = "round_closed";
inline constexpr const char* kResultLate = "late";
inline constexpr const char* kRoundStatusOpen = "open";

struct PoolStats {
    int balance_unused = 0;
    int invite_unused = 0;
};

struct CurrentRound {
    int id = 0;
    int difficulty_bits = 0;
    std::string expires_at;
    int memory_cost_mb = 0;
    int parallelism = 0;
    std::string round_key;
    std::string seed;
    std::string status;
    int time_cost = 0;

    bool is_open() const;
};

struct StatusResponse {
    std::string admin_lock;
    std::optional<CurrentRound> current_round;
    std::optional<int> daily_drop_remaining;
    bool desktop_only = false;
    bool enabled = false;
    int inventory_remaining = 0;
    std::optional<PoolStats> pool_stats;
    std::string result;
    std::string server_time;

    bool daily_limit_reached() const;
    int balance_inventory_remaining() const;
    int invite_inventory_remaining() const;
    int inventory_remaining_for(RewardKind kind) const;
};

struct ChallengeResponse {
    std::string admin_lock;
    int challenge_id = 0;
    int difficulty_bits = 0;
    std::string expires_at;
    int memory_cost_mb = 0;
    std::string message;
    bool ok = false;
    int parallelism = 0;
    std::optional<PoolStats> pool_stats;
    std::string result;
    int round_id = 0;
    std::string seed;
    std::string session_salt;
    int time_cost = 0;
    std::string visitor_id;
};

struct HeartbeatRequest {
    int challenge_id = 0;
    int round_id = 0;
};

struct HeartbeatResponse {
    std::string result;
};

struct SubmitRequest {
    int challenge_id = 0;
    int round_id = 0;
    std::string nonce;
    std::string digest;
    std::string preference;
};

struct SubmitResponse {
    double balance_amount = 0.0;
    std::string code_type;
    std::string drop_type;
    std::string forced_by;
    std::string invite_code;
    std::string balance_code;
    bool ok = false;
    std::string result;
    int reward_code_id = 0;

    std::string preferred_reward_code(RewardKind kind) const;
};

void from_json(const nlohmann::json& j, PoolStats& value);
void from_json(const nlohmann::json& j, CurrentRound& value);
void from_json(const nlohmann::json& j, StatusResponse& value);
void from_json(const nlohmann::json& j, ChallengeResponse& value);
void from_json(const nlohmann::json& j, HeartbeatResponse& value);
void from_json(const nlohmann::json& j, SubmitResponse& value);
void to_json(nlohmann::json& j, const HeartbeatRequest& value);
void to_json(nlohmann::json& j, const SubmitRequest& value);

} // namespace app
