#pragma once

#include <cstddef>
#include <cstdint>

#ifdef _WIN32
#define MINING_OPENCL_EXPORT extern "C" __declspec(dllexport)
#else
#define MINING_OPENCL_EXPORT extern "C"
#endif

struct mining_opencl_solver_config {
    std::size_t batch_size;
    bool by_segment;
    bool precompute_refs;
};

struct mining_opencl_job {
    const std::uint8_t* seed_ptr;
    std::size_t seed_len;
    const std::uint8_t* pass_prefix_ptr;
    std::size_t pass_prefix_len;
    std::uint32_t time_cost;
    std::uint32_t memory_cost_kib;
    std::uint32_t parallelism;
    int difficulty_bits;
};

struct mining_opencl_session;

struct mining_opencl_benchmark_result {
    std::size_t batch_size;
    bool by_segment;
    bool precompute_refs;
    std::int64_t attempts;
    std::int64_t elapsed_ms;
    double attempts_per_second;
};

struct mining_opencl_mine_result {
    bool found;
    std::uint64_t nonce;
    std::int64_t attempts;
    char digest_hex[65];
};

struct mining_opencl_device_info {
    std::size_t device_index;
    char device_id[32];
    char name[128];
};

MINING_OPENCL_EXPORT bool mining_opencl_is_available();
MINING_OPENCL_EXPORT bool mining_opencl_validate();
MINING_OPENCL_EXPORT std::size_t mining_opencl_device_count();
MINING_OPENCL_EXPORT bool mining_opencl_get_device_info(std::size_t device_index, mining_opencl_device_info* result);
MINING_OPENCL_EXPORT const char* mining_opencl_last_error_message();
MINING_OPENCL_EXPORT bool mining_opencl_default_solver_config(
    std::size_t device_index,
    const mining_opencl_job* job,
    mining_opencl_solver_config* result);
MINING_OPENCL_EXPORT bool mining_opencl_find_best_benchmark_config(
    std::size_t device_index,
    mining_opencl_benchmark_result* result);
MINING_OPENCL_EXPORT bool mining_opencl_mine_batch(
    std::size_t device_index,
    const mining_opencl_job* job,
    const mining_opencl_solver_config* config,
    std::uint64_t start_nonce,
    mining_opencl_mine_result* result);
MINING_OPENCL_EXPORT mining_opencl_session* mining_opencl_session_create(
    std::size_t device_index,
    const mining_opencl_job* job,
    const mining_opencl_solver_config* config,
    std::uint64_t start_nonce);
MINING_OPENCL_EXPORT bool mining_opencl_session_mine_next_batch(
    mining_opencl_session* session,
    mining_opencl_mine_result* result);
MINING_OPENCL_EXPORT void mining_opencl_session_destroy(mining_opencl_session* session);
