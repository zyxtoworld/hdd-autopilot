#pragma once

#include <chrono>
#include <cstddef>
#include <filesystem>
#include <string>

namespace app {

struct Config {
    std::string base_url;
    std::filesystem::path output_file;
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
};

Config default_config(std::size_t batch_size);

} // namespace app
