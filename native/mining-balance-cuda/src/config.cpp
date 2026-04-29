#include "app/config.hpp"
#include "app/path.hpp"

namespace app {

Config default_config(std::size_t batch_size) {
    return Config{
        .base_url = "https://sub.hdd.sb",
        .output_file = resolve_data_file_path("mining/balance-codes.txt"),
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
    };
}

} // namespace app
