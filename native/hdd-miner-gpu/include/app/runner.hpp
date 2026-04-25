#pragma once

#include <string>

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

private:
    Config config_;
    PoolClient pool_client_;
    InviteStore invite_store_;
    BalanceCodeStore balance_code_store_;
    Solver solver_;

    void run_loop(const SolverConfig& solver_config);
    void run_cycle(const SolverConfig& solver_config);
    RewardKind select_reward_kind(const StatusResponse& status) const;
    int remaining_for(const StatusResponse& status, RewardKind target) const;
    const char* name_for(RewardKind target) const;
    const char* preference_for(RewardKind target) const;
    const std::filesystem::path& output_path_for(RewardKind target) const;
    void save_code(RewardKind target, const std::string& code);
};

} // namespace app
