#include "app/solver.hpp"

#include <algorithm>
#include <array>
#include <chrono>
#include <cstring>
#include <stdexcept>
#include <vector>

#include "argon2-cuda/device.h"
#include "argon2-cuda/globalcontext.h"
#include "argon2-cuda/processingunit.h"
#include "argon2-cuda/programcontext.h"
#include "argon2-gpu-common/argon2-common.h"
#include "argon2-gpu-common/argon2params.h"

#include <argon2.h>

namespace app {
namespace {

constexpr std::size_t kDigestSize = 32;
constexpr std::size_t kDefaultRunBatchCap = 32;
constexpr std::chrono::milliseconds kBenchmarkCaseDuration{5000};

struct BatchResult {
    bool found = false;
    std::uint64_t nonce = 0;
    std::array<std::uint8_t, kDigestSize> digest{};
};

std::vector<std::uint8_t> compute_reference_digest(const Job& job, std::uint64_t nonce) {
    const auto password = job.password_for_nonce(nonce);
    std::vector<std::uint8_t> digest(kDigestSize);
    const auto result = argon2id_hash_raw(job.time_cost(),
                                          job.memory_cost_kb(),
                                          job.parallelism(),
                                          password.data(),
                                          password.size(),
                                          job.seed_bytes().data(),
                                          job.seed_bytes().size(),
                                          digest.data(),
                                          digest.size());
    if (result != ARGON2_OK) {
        throw std::runtime_error(argon2_error_message(result));
    }
    return digest;
}

std::vector<std::uint8_t> compute_gpu_digest(const Job& job,
                                             const SolverConfig& config,
                                             std::size_t device_index,
                                             std::uint64_t nonce) {
    using namespace argon2;
    using namespace argon2::cuda;

    GlobalContext global;
    const auto& devices = global.getAllDevices();
    if (device_index >= devices.size()) {
        throw std::runtime_error("CUDA device index out of range");
    }

    const auto& device = devices[device_index];
    ProgramContext program_context(&global, {device}, argon2::ARGON2_ID, argon2::ARGON2_VERSION_13);
    Argon2Params params(kDigestSize,
                        job.seed_bytes().data(),
                        job.seed_bytes().size(),
                        nullptr,
                        0,
                        nullptr,
                        0,
                        job.time_cost(),
                        job.memory_cost_kb(),
                        job.parallelism());
    ProcessingUnit unit(&program_context,
                        &params,
                        &device,
                        1,
                        config.by_segment,
                        config.precompute_refs);

    const auto password = job.password_for_nonce(nonce);
    unit.setPassword(0, password.data(), password.size());
    unit.beginProcessing();
    unit.endProcessing();

    std::vector<std::uint8_t> digest(kDigestSize);
    unit.getHash(0, digest.data());
    return digest;
}

BatchResult mine_batch_gpu(const Job& job,
                           const SolverConfig& config,
                           std::size_t device_index,
                           std::uint64_t start_nonce,
                           std::atomic_bool& stop,
                           std::atomic<std::int64_t>& attempts) {
    using namespace argon2;
    using namespace argon2::cuda;

    GlobalContext global;
    const auto& devices = global.getAllDevices();
    if (device_index >= devices.size()) {
        throw std::runtime_error("CUDA device index out of range");
    }

    const auto& device = devices[device_index];
    ProgramContext program_context(&global, {device}, argon2::ARGON2_ID, argon2::ARGON2_VERSION_13);
    Argon2Params params(kDigestSize,
                        job.seed_bytes().data(),
                        job.seed_bytes().size(),
                        nullptr,
                        0,
                        nullptr,
                        0,
                        job.time_cost(),
                        job.memory_cost_kb(),
                        job.parallelism());
    ProcessingUnit unit(&program_context,
                        &params,
                        &device,
                        config.batch_size,
                        config.by_segment,
                        config.precompute_refs);

    std::vector<std::string> passwords(config.batch_size);
    for (std::size_t i = 0; i < config.batch_size; ++i) {
        if (stop.load()) {
            break;
        }
        passwords[i] = job.password_for_nonce(start_nonce + i);
        unit.setPassword(i, passwords[i].data(), passwords[i].size());
    }

    if (stop.load()) {
        return {};
    }

    unit.beginProcessing();
    unit.endProcessing();

    BatchResult result;
    for (std::size_t i = 0; i < config.batch_size; ++i) {
        if (stop.load()) {
            break;
        }

        std::array<std::uint8_t, kDigestSize> digest{};
        unit.getHash(i, digest.data());
        attempts.fetch_add(1);
        if (meets_difficulty(digest.data(), digest.size(), job.difficulty_bits())) {
            result.found = true;
            result.nonce = start_nonce + i;
            result.digest = digest;
            stop.store(true);
            break;
        }
    }
    return result;
}

} // namespace

Solver::Solver(std::size_t device_index) : device_index_(device_index) {
}

SolverConfig Solver::default_config_for(const Job& job) const {
    SolverConfig config;
    config.batch_size = std::min<std::size_t>(estimate_max_batch_size(job), kDefaultRunBatchCap);
    config.by_segment = false;
    config.precompute_refs = false;
    return config;
}

SolveResult Solver::mine_batch(const Job& job,
                               const SolverConfig& config,
                               std::uint64_t start_nonce,
                               std::atomic_bool& stop,
                               std::atomic<std::int64_t>& attempts) const {
    auto current_config = config;
    if (current_config.batch_size == 0) {
        current_config = default_config_for(job);
    }

    SolveResult result;
    const auto batch = mine_batch_gpu(job, current_config, device_index_, start_nonce, stop, attempts);
    if (batch.found) {
        result.found = true;
        result.nonce = batch.nonce;
        result.digest = hex_encode(batch.digest.data(), batch.digest.size());
    }
    return result;
}

BenchmarkResult Solver::run_benchmark_case(const Job& job,
                                           const SolverConfig& config,
                                           std::chrono::milliseconds duration) const {
    BenchmarkResult result;
    result.config = config;

    std::atomic_bool stop{false};
    std::atomic<std::int64_t> attempts{0};
    std::uint64_t next_nonce = 1;
    const auto started_at = std::chrono::steady_clock::now();

    while (std::chrono::steady_clock::now() - started_at < duration) {
        mine_batch_gpu(job, config, device_index_, next_nonce, stop, attempts);
        next_nonce += config.batch_size;
    }

    result.attempts = attempts.load();
    result.elapsed = std::chrono::duration_cast<std::chrono::milliseconds>(std::chrono::steady_clock::now() - started_at);
    if (result.elapsed.count() > 0) {
        result.attempts_per_second = static_cast<double>(result.attempts) * 1000.0 / static_cast<double>(result.elapsed.count());
    }
    return result;
}

void Solver::validate_against_reference(const Job& job, std::uint64_t nonce) const {
    const auto reference_digest = compute_reference_digest(job, nonce);
    const auto gpu_digest = compute_gpu_digest(job,
                                               SolverConfig{.batch_size = 1, .by_segment = false, .precompute_refs = false},
                                               device_index_,
                                               nonce);
    if (reference_digest != gpu_digest) {
        throw std::runtime_error("GPU digest mismatch for nonce=" + std::to_string(nonce)
            + ": expected=" + hex_encode(reference_digest.data(), reference_digest.size())
            + " actual=" + hex_encode(gpu_digest.data(), gpu_digest.size()));
    }
}

BenchmarkResult Solver::find_best_benchmark_config() const {
    Job benchmark_job(JobConfig{
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

    const auto max_batch_size = estimate_max_batch_size(benchmark_job);
    const auto candidates = build_benchmark_candidates(max_batch_size);

    BenchmarkResult best;
    for (const auto& candidate : candidates) {
        const auto current = run_benchmark_case(benchmark_job, candidate, kBenchmarkCaseDuration);
        if (current.attempts_per_second > best.attempts_per_second) {
            best = current;
        }
    }
    return best;
}

std::size_t Solver::estimate_max_batch_size(const Job& job) const {
    using namespace argon2::cuda;

    GlobalContext global;
    const auto& devices = global.getAllDevices();
    if (device_index_ >= devices.size()) {
        throw std::runtime_error("CUDA device index out of range");
    }

    cudaDeviceProp properties{};
    cudaGetDeviceProperties(&properties, static_cast<int>(device_index_));

    const auto bytes_per_job = static_cast<std::size_t>(job.memory_cost_kb()) * 1024ULL * static_cast<std::size_t>(job.parallelism());
    if (bytes_per_job == 0) {
        return 1;
    }

    const auto usable = static_cast<std::size_t>(static_cast<double>(properties.totalGlobalMem) * 0.5);
    const auto max_batch = usable / bytes_per_job;
    return std::max<std::size_t>(1, std::min<std::size_t>(max_batch, 256));
}

std::vector<SolverConfig> Solver::build_benchmark_candidates(std::size_t max_batch_size) {
    std::vector<SolverConfig> candidates;
    for (std::size_t batch_size : {std::size_t{1}, std::size_t{2}, std::size_t{4}, std::size_t{8}, std::size_t{16}, std::size_t{32}, std::size_t{64}, std::size_t{128}, std::size_t{256}}) {
        if (batch_size > max_batch_size) {
            continue;
        }
        candidates.push_back(SolverConfig{.batch_size = batch_size, .by_segment = false, .precompute_refs = false});
        candidates.push_back(SolverConfig{.batch_size = batch_size, .by_segment = true, .precompute_refs = false});
        candidates.push_back(SolverConfig{.batch_size = batch_size, .by_segment = true, .precompute_refs = true});
    }
    if (candidates.empty()) {
        candidates.push_back(SolverConfig{.batch_size = 1, .by_segment = false, .precompute_refs = false});
    }
    return candidates;
}

} // namespace app
