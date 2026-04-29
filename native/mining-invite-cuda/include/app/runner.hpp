#pragma once

#include "app/config.hpp"
#include "app_shared/runner.hpp"

namespace app {

class Runner {
public:
    explicit Runner(Config config);

    void run();
    void run_auto_tuned();
    void run_benchmark();
    void request_stop() noexcept;
    bool stop_requested() const noexcept;

private:
    RewardRunner inner_;
};

} // namespace app
