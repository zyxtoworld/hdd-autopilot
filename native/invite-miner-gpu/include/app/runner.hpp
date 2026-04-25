#pragma once

#include "app/config.hpp"
#include "app/invite_store.hpp"
#include "app/pool_client.hpp"
#include "app/solver.hpp"

namespace app {

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
    Solver solver_;

    void run_loop(const SolverConfig& solver_config);
    void run_cycle(const SolverConfig& solver_config);
};

} // namespace app
