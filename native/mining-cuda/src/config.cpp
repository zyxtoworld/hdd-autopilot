#include "app/config.hpp"
#include "app/path.hpp"

namespace app {

Config default_config(std::size_t batch_size, MiningMode mode) {
    return Config{
        .base_url = "https://sub.hdd.sb",
        .invite_output_file = resolve_data_file_path("mining/invite-codes.txt"),
        .balance_output_file = resolve_data_file_path("mining/balance-codes.txt"),
        .batch_size = batch_size,
        .device_index = 0,
        .http_timeout = std::chrono::seconds(30),
        .heartbeat_interval = std::chrono::seconds(4),
        .progress_interval = std::chrono::seconds(10),
        .retry_delay = std::chrono::seconds(3),
        .success_delay = std::chrono::seconds(3),
        .daily_limit_delay = std::chrono::seconds(60),
        .inventory_depleted_delay = std::chrono::seconds(60),
        .round_status_poll_interval = std::chrono::milliseconds(500),
        .mode = mode,
    };
}

const char* mode_name(MiningMode mode) noexcept {
    switch (mode) {
    case MiningMode::invite_then_balance:
        return "invite_then_balance";
    case MiningMode::balance_then_invite:
        return "balance_then_invite";
    case MiningMode::invite_only:
        return "invite_only";
    case MiningMode::balance_only:
        return "balance_only";
    }
    return "invite_then_balance";
}

const char* mode_description(MiningMode mode) noexcept {
    switch (mode) {
    case MiningMode::invite_then_balance:
        return "先尝试邀请码，不够了再切到余额兑换码";
    case MiningMode::balance_then_invite:
        return "先尝试余额兑换码，不够了再切到邀请码";
    case MiningMode::invite_only:
        return "只尝试邀请码";
    case MiningMode::balance_only:
        return "只尝试余额兑换码";
    }
    return "先尝试邀请码，不够了再切到余额兑换码";
}

} // namespace app
