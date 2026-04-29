#pragma once

#include <atomic>
#include <mutex>
#include <string>
#include <vector>

#include "app/balance_code_store.hpp"
#include "app/config.hpp"
#include "app/invite_store.hpp"
#include "app/pool_client.hpp"
#include "app/solver.hpp"

namespace app {

enum class RewardKind {
    invite,
    balance,
};

class Runner {
public:
    explicit Runner(Config config);

    void run();
    void run_auto_tuned();
    void run_benchmark();
    void request_stop() noexcept;
    bool stop_requested() const noexcept;

private:
    Config config_;
    PoolClient pool_client_;
    InviteStore invite_store_;
    BalanceCodeStore balance_code_store_;
    Solver solver_;
    std::atomic_bool stop_requested_{false};
    std::mutex active_stop_mutex_;
    std::atomic_bool* active_stop_ = nullptr;

    void run_loop(const SolverConfig& solver_config);
    void run_cycle(const SolverConfig& solver_config);
    std::vector<RewardKind> preferred_reward_order() const;
    RewardKind select_reward_kind(const StatusResponse& status) const;
    int remaining_for(const StatusResponse& status, RewardKind target) const;
    const char* name_for(RewardKind target) const;
    const char* preference_for(RewardKind target) const;
    const std::filesystem::path& output_path_for(RewardKind target) const;
    void save_code(RewardKind target, const std::string& code);
    void reset_stop_request() noexcept;
    void attach_active_stop(std::atomic_bool* stop) noexcept;
    void detach_active_stop(std::atomic_bool* stop) noexcept;
};

} // namespace app
