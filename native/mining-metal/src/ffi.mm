#import <Foundation/Foundation.h>
#import <Metal/Metal.h>

#include "app/ffi.hpp"

#include <algorithm>
#include <atomic>
#include <cstring>
#include <exception>
#include <stdexcept>
#include <string>
#include <vector>

#include "app/job.hpp"
#include "app/solver.hpp"

namespace {
thread_local std::string g_last_error;

void set_last_error(const std::exception& error) {
    g_last_error = error.what();
}

void clear_last_error() {
    g_last_error.clear();
}

app::Job benchmark_job() {
    return app::Job(app::JobConfig{
        .seed = "benchmark-seed-fixed",
        .round_id = 1,
        .visitor_id = "benchmark-visitor-fixed",
        .challenge_id = 1,
        .session_salt = "benchmark-session-salt-fixed",
        .time_cost = 1,
        .memory_cost_mb = 64,
        .parallelism = 1,
        .difficulty_bits = 255,
    });
}

app::Job make_job(const mining_metal_job& raw) {
    const auto seed = std::string(
        reinterpret_cast<const char*>(raw.seed_ptr),
        reinterpret_cast<const char*>(raw.seed_ptr) + raw.seed_len);
    const auto pass_prefix = std::string(
        reinterpret_cast<const char*>(raw.pass_prefix_ptr),
        reinterpret_cast<const char*>(raw.pass_prefix_ptr) + raw.pass_prefix_len);
    return app::Job(app::JobConfig{
        .seed = seed,
        .pass_prefix_override = pass_prefix,
        .time_cost = static_cast<int>(raw.time_cost),
        .memory_cost_mb = static_cast<int>(raw.memory_cost_kib / 1024),
        .parallelism = static_cast<int>(raw.parallelism),
        .difficulty_bits = raw.difficulty_bits,
    });
}

app::SolverConfig make_solver_config(const mining_metal_solver_config& raw) {
    return app::SolverConfig{
        .batch_size = raw.batch_size,
        .by_segment = raw.by_segment,
        .precompute_refs = raw.precompute_refs,
    };
}

void fill_mine_result(const app::SolveResult& mined,
                      const std::atomic<std::int64_t>& attempts,
                      mining_metal_mine_result* result) {
    result->found = mined.found;
    result->nonce = mined.nonce;
    result->attempts = attempts.load();
    std::fill(std::begin(result->digest_hex), std::end(result->digest_hex), '\0');
    if (!mined.digest.empty()) {
        std::strncpy(result->digest_hex, mined.digest.c_str(), sizeof(result->digest_hex) - 1);
    }
}

void fill_device_info(std::size_t device_index, mining_metal_device_info* result) {
    const auto devices = MTLCopyAllDevices();
    if (devices == nil || device_index >= devices.count) {
        throw std::runtime_error("Metal device index out of range");
    }
    const auto device = devices[device_index];
    result->device_index = device_index;
    std::fill(std::begin(result->device_id), std::end(result->device_id), '\0');
    std::fill(std::begin(result->name), std::end(result->name), '\0');
    const auto device_id = std::string("metal:") + std::to_string(static_cast<unsigned long long>(device.registryID));
    const auto name = std::string(device.name.UTF8String ?: "");
    std::strncpy(result->device_id, device_id.c_str(), sizeof(result->device_id) - 1);
    std::strncpy(result->name, name.c_str(), sizeof(result->name) - 1);
}
} // namespace

struct mining_metal_session {
    app::Job job;
    app::Solver solver;
    app::SolverSession session;
    std::atomic_bool stop{false};
    std::atomic<std::int64_t> attempts{0};

    mining_metal_session(std::size_t device_index,
                      app::Job&& native_job,
                      const app::SolverConfig& native_config,
                      std::uint64_t start_nonce)
        : job(std::move(native_job)),
          solver(device_index),
          session(solver.create_session(job, native_config, start_nonce)) {
    }
};

bool mining_metal_is_available() {
    try {
        clear_last_error();
        const auto devices = MTLCopyAllDevices();
        return devices != nil && devices.count > 0;
    } catch (const std::exception& error) {
        set_last_error(error);
        return false;
    }
}

bool mining_metal_validate_device(std::size_t device_index) {
    try {
        clear_last_error();
        const auto devices = MTLCopyAllDevices();
        if (devices == nil || devices.count == 0) {
            throw std::runtime_error("当前没有检测到可用的 Metal 设备。");
        }
        if (device_index >= devices.count) {
            throw std::runtime_error("Metal device index out of range");
        }
        app::Solver solver(device_index);
        solver.validate_against_reference(benchmark_job(), 1);
        return true;
    } catch (const std::exception& error) {
        set_last_error(error);
        return false;
    }
}

bool mining_metal_validate() {
    return mining_metal_validate_device(0);
}

std::size_t mining_metal_device_count() {
    try {
        clear_last_error();
        const auto devices = MTLCopyAllDevices();
        return devices == nil ? 0 : devices.count;
    } catch (const std::exception& error) {
        set_last_error(error);
        return 0;
    }
}

bool mining_metal_get_device_info(std::size_t device_index, mining_metal_device_info* result) {
    if (result == nullptr) {
        g_last_error = "device info pointer is null";
        return false;
    }
    try {
        clear_last_error();
        fill_device_info(device_index, result);
        return true;
    } catch (const std::exception& error) {
        set_last_error(error);
        return false;
    }
}

const char* mining_metal_last_error_message() {
    return g_last_error.c_str();
}

bool mining_metal_default_solver_config(
    std::size_t device_index,
    const mining_metal_job* job,
    mining_metal_solver_config* result) {
    if (job == nullptr || result == nullptr) {
        g_last_error = "default_solver_config parameter is null";
        return false;
    }
    try {
        clear_last_error();
        app::Solver solver(device_index);
        const auto native_job = make_job(*job);
        const auto config = solver.default_config_for(native_job);
        result->batch_size = config.batch_size;
        result->by_segment = config.by_segment;
        result->precompute_refs = config.precompute_refs;
        return true;
    } catch (const std::exception& error) {
        set_last_error(error);
        return false;
    }
}

bool mining_metal_find_best_benchmark_config(std::size_t device_index, mining_metal_benchmark_result* result) {
    if (result == nullptr) {
        g_last_error = "benchmark result pointer is null";
        return false;
    }
    try {
        clear_last_error();
        app::Solver solver(device_index);
        const auto best = solver.find_best_benchmark_config();
        result->batch_size = best.config.batch_size;
        result->by_segment = best.config.by_segment;
        result->precompute_refs = best.config.precompute_refs;
        result->attempts = best.attempts;
        result->elapsed_ms = best.elapsed.count();
        result->attempts_per_second = best.attempts_per_second;
        return true;
    } catch (const std::exception& error) {
        set_last_error(error);
        return false;
    }
}

bool mining_metal_mine_batch(
    std::size_t device_index,
    const mining_metal_job* job,
    const mining_metal_solver_config* config,
    std::uint64_t start_nonce,
    mining_metal_mine_result* result) {
    if (job == nullptr || config == nullptr || result == nullptr) {
        g_last_error = "mine_batch parameter is null";
        return false;
    }
    try {
        clear_last_error();
        app::Solver solver(device_index);
        const auto native_job = make_job(*job);
        const auto native_config = make_solver_config(*config);
        std::atomic_bool stop{false};
        std::atomic<std::int64_t> attempts{0};
        const auto mined = solver.mine_batch(native_job, native_config, start_nonce, stop, attempts);
        fill_mine_result(mined, attempts, result);
        return true;
    } catch (const std::exception& error) {
        set_last_error(error);
        return false;
    }
}

mining_metal_session* mining_metal_session_create(
    std::size_t device_index,
    const mining_metal_job* job,
    const mining_metal_solver_config* config,
    std::uint64_t start_nonce) {
    if (job == nullptr || config == nullptr) {
        g_last_error = "session_create parameter is null";
        return nullptr;
    }
    try {
        clear_last_error();
        auto native_job = make_job(*job);
        const auto native_config = make_solver_config(*config);
        return new mining_metal_session(device_index, std::move(native_job), native_config, start_nonce);
    } catch (const std::exception& error) {
        set_last_error(error);
        return nullptr;
    }
}

bool mining_metal_session_mine_next_batch(
    mining_metal_session* session,
    mining_metal_mine_result* result) {
    if (session == nullptr || result == nullptr) {
        g_last_error = "session_mine_next_batch parameter is null";
        return false;
    }
    try {
        clear_last_error();
        const auto mined = session->solver.mine_next_batch(session->job, session->session, session->stop, session->attempts);
        fill_mine_result(mined, session->attempts, result);
        return true;
    } catch (const std::exception& error) {
        set_last_error(error);
        return false;
    }
}

void mining_metal_session_destroy(mining_metal_session* session) {
    delete session;
}
