#include "app/runner.hpp"

#include <utility>

#include "app_shared/reward_policy.hpp"

namespace app {

Runner::Runner(Config config)
    : inner_(std::move(config), invite_reward_policy()) {
}

void Runner::run() {
    inner_.run();
}

void Runner::run_auto_tuned() {
    inner_.run_auto_tuned();
}

void Runner::run_benchmark() {
    inner_.run_benchmark();
}

void Runner::request_stop() noexcept {
    inner_.request_stop();
}

bool Runner::stop_requested() const noexcept {
    return inner_.stop_requested();
}

} // namespace app
