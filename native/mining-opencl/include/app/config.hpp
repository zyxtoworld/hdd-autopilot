#pragma once

#include <chrono>
#include <cstddef>
#include <filesystem>
#include <string>

namespace app {

enum class MiningMode {
    invite_then_balance,
    balance_then_invite,
    invite_only,
    balance_only,
};

struct Config {
    std::string base_url;
    std::filesystem::path invite_output_file;
    std::filesystem::path balance_output_file;
    std::size_t batch_size;
    std::size_t device_index;
    std::chrono::milliseconds http_timeout;
    std::chrono::milliseconds heartbeat_interval;
    std::chrono::milliseconds progress_interval;
    std::chrono::milliseconds retry_delay;
    std::chrono::milliseconds success_delay;
    std::chrono::milliseconds daily_limit_delay;
    std::chrono::milliseconds inventory_depleted_delay;
    std::chrono::milliseconds round_status_poll_interval;
    MiningMode mode;
};

Config default_config(std::size_t batch_size, MiningMode mode);
const char* mode_name(MiningMode mode) noexcept;
const char* mode_description(MiningMode mode) noexcept;

} // namespace app
