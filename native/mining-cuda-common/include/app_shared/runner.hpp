#pragma once

#include <atomic>
#include <mutex>

#include "app/config.hpp"
#include "app/solver.hpp"
#include "app_shared/code_store.hpp"
#include "app_shared/pool_client.hpp"
#include "app_shared/reward_policy.hpp"

namespace app {

class RewardRunner {
public:
    RewardRunner(Config config, RewardPolicy policy);

    void run();
    void run_auto_tuned();
    void run_benchmark();
    void request_stop() noexcept;
    bool stop_requested() const noexcept;

private:
    Config config_;
    RewardPolicy policy_;
    PoolClient pool_client_;
    CodeStore code_store_;
    Solver solver_;
    std::atomic_bool stop_requested_{false};
    std::mutex active_stop_mutex_;
    std::atomic_bool* active_stop_ = nullptr;

    void run_loop(const SolverConfig& solver_config);
    void run_cycle(const SolverConfig& solver_config);
    void reset_stop_request() noexcept;
    void attach_active_stop(std::atomic_bool* stop) noexcept;
    void detach_active_stop(std::atomic_bool* stop) noexcept;
};

} // namespace app
