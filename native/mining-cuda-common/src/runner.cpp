#include "app_shared/runner.hpp"

#include <algorithm>
#include <atomic>
#include <cctype>
#include <chrono>
#include <cstdint>
#include <iostream>
#include <thread>
#include <utility>
#include <vector>

#include "app/job.hpp"

namespace app {
namespace {

bool is_daily_limit(const PoolError& error) {
    return error.code() == PoolErrorCode::daily_limit;
}

bool is_inventory_depleted(const PoolError& error) {
    return error.code() == PoolErrorCode::inventory_depleted;
}

bool is_no_open_round(const PoolError& error) {
    return error.code() == PoolErrorCode::no_open_round;
}

bool is_pool_disabled(const PoolError& error) {
    return error.code() == PoolErrorCode::pool_disabled;
}

bool is_round_closed(const PoolError& error) {
    return error.code() == PoolErrorCode::round_closed;
}

void ensure_round_active(const StatusResponse& snapshot, int expected_round_id, RewardKind kind) {
    if (!snapshot.enabled || !snapshot.current_round.has_value() || !snapshot.current_round->is_open() || snapshot.current_round->id != expected_round_id) {
        throw PoolError(PoolErrorCode::round_closed, "round_closed");
    }
    if (snapshot.inventory_remaining_for(kind) <= 0) {
        throw PoolError(PoolErrorCode::inventory_depleted, "inventory_depleted");
    }
    if (snapshot.daily_limit_reached()) {
        throw PoolError(PoolErrorCode::daily_limit, "daily_limit");
    }
}

JobConfig job_config_from_challenge(const ChallengeResponse& challenge) {
    return JobConfig{
        .seed = challenge.seed,
        .round_id = challenge.round_id,
        .visitor_id = challenge.visitor_id,
        .challenge_id = challenge.challenge_id,
        .session_salt = challenge.session_salt,
        .time_cost = challenge.time_cost,
        .memory_cost_mb = challenge.memory_cost_mb,
        .parallelism = challenge.parallelism,
        .difficulty_bits = challenge.difficulty_bits,
    };
}

Job benchmark_job() {
    return Job(JobConfig{
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

std::string display_path(const std::filesystem::path& path) {
    const auto utf8 = path.u8string();
    return std::string(utf8.begin(), utf8.end());
}

std::string trim_copy(std::string value) {
    const auto is_space = [](unsigned char ch) {
        return std::isspace(ch) != 0;
    };
    value.erase(value.begin(), std::find_if(value.begin(), value.end(), [&](unsigned char ch) { return !is_space(ch); }));
    value.erase(std::find_if(value.rbegin(), value.rend(), [&](unsigned char ch) { return !is_space(ch); }).base(), value.end());
    return value;
}

std::string result_label(const std::string& result) {
    const auto trimmed = trim_copy(result);
    if (trimmed == kResultDailyWinLimitReached) {
        return "今日命中次数已达上限";
    }
    if (trimmed == kResultRoundClosed) {
        return "轮次已关闭";
    }
    if (trimmed == kResultLate) {
        return "提交过晚";
    }
    if (trimmed == "ok" || trimmed == "accepted" || trimmed == "success") {
        return "成功";
    }
    return trimmed.empty() ? "未说明" : trimmed;
}

void validate_difficulty_rules() {
    const std::vector<std::uint8_t> full_byte_ok{0x00, 0xFF};
    if (!meets_difficulty(full_byte_ok, 8)) {
        throw std::runtime_error("难度自检失败：整字节匹配校验未通过");
    }
    if (meets_difficulty(full_byte_ok, 9)) {
        throw std::runtime_error("难度自检失败：整字节溢出校验未通过");
    }

    const std::vector<std::uint8_t> partial_ok{0x00, 0x7F};
    if (!meets_difficulty(partial_ok, 9)) {
        throw std::runtime_error("难度自检失败：部分位匹配校验未通过");
    }

    const std::vector<std::uint8_t> partial_fail{0x00, 0x80};
    if (meets_difficulty(partial_fail, 9)) {
        throw std::runtime_error("难度自检失败：部分位拒绝校验未通过");
    }
}

void validate_solver_basics(Solver& solver) {
    validate_difficulty_rules();
    solver.validate_against_reference(benchmark_job(), 1);
}

} // namespace

RewardRunner::RewardRunner(Config config, RewardPolicy policy)
    : config_(std::move(config)),
      policy_(policy),
      pool_client_(config_, policy_),
      code_store_(config_.output_file, policy_.reward_name),
      solver_(config_.device_index) {
}

void RewardRunner::request_stop() noexcept {
    stop_requested_.store(true);
    std::lock_guard<std::mutex> lock(active_stop_mutex_);
    if (active_stop_ != nullptr) {
        active_stop_->store(true);
    }
}

bool RewardRunner::stop_requested() const noexcept {
    return stop_requested_.load();
}

void RewardRunner::reset_stop_request() noexcept {
    stop_requested_.store(false);
}

void RewardRunner::attach_active_stop(std::atomic_bool* stop) noexcept {
    std::lock_guard<std::mutex> lock(active_stop_mutex_);
    active_stop_ = stop;
    if (active_stop_ != nullptr && stop_requested_.load()) {
        active_stop_->store(true);
    }
}

void RewardRunner::detach_active_stop(std::atomic_bool* stop) noexcept {
    std::lock_guard<std::mutex> lock(active_stop_mutex_);
    if (active_stop_ == stop) {
        active_stop_ = nullptr;
    }
}

void RewardRunner::run() {
    reset_stop_request();
    std::cout << "开始运行 " << policy_.product_name << "：这次会持续尝试" << policy_.reward_name << "。\n";
    if (config_.batch_size == 0) {
        std::cout << "这次的批大小会自动选择。\n";
    } else {
        std::cout << "这次的批大小是 " << config_.batch_size << "。\n";
    }
    std::cout << "启动前会先校验显卡求解器和难度规则。\n";
    validate_solver_basics(solver_);
    std::cout << "命中的" << policy_.reward_name << "会保存到：" << display_path(code_store_.path()) << "\n";
    run_loop(SolverConfig{
        .batch_size = config_.batch_size,
        .by_segment = false,
        .precompute_refs = false,
    });
}

void RewardRunner::run_auto_tuned() {
    reset_stop_request();
    std::cout << "开始运行 " << policy_.product_name << " 自动调优模式：先测一下这张显卡更适合哪套配置。\n";
    std::cout << "启动前会先校验显卡求解器和难度规则。\n";
    validate_solver_basics(solver_);
    const auto best = solver_.find_best_benchmark_config();
    std::cout << "已经选好一套推荐配置：批大小 " << best.config.batch_size
              << "，按分段 " << (best.config.by_segment ? "是" : "否")
              << "，预计算参考值 " << (best.config.precompute_refs ? "是" : "否")
              << "，预计速度约 " << best.attempts_per_second << " 次/秒。\n";
    std::cout << "命中的" << policy_.reward_name << "会保存到：" << display_path(code_store_.path()) << "\n";
    run_loop(best.config);
}

void RewardRunner::run_benchmark() {
    reset_stop_request();
    std::cout << "开始本地压测：这一模式不会连接矿池，也不会提交结果。\n";
    std::cout << "启动前会先校验显卡求解器和难度规则。\n";
    validate_solver_basics(solver_);
    const auto best = solver_.find_best_benchmark_config();
    std::cout << "推荐配置是：批大小 " << best.config.batch_size
              << "，按分段 " << (best.config.by_segment ? "是" : "否")
              << "，预计算参考值 " << (best.config.precompute_refs ? "是" : "否")
              << "，预计速度约 " << best.attempts_per_second << " 次/秒。\n";
}

void RewardRunner::run_loop(const SolverConfig& solver_config) {
    for (;;) {
        if (stop_requested()) {
            std::cout << "已收到停止请求，正在返回。\n";
            return;
        }
        try {
            run_cycle(solver_config);
            if (stop_requested()) {
                std::cout << "已收到停止请求，正在返回。\n";
                return;
            }
            std::cout << "这一轮已经命中，接下来等下一轮开放。\n";
            std::this_thread::sleep_for(config_.success_delay);
        } catch (const PoolError& error) {
            if (stop_requested()) {
                std::cout << "已收到停止请求，正在返回。\n";
                return;
            }
            if (is_daily_limit(error)) {
                std::cout << "今天的命中次数已经用完了，稍后再试。\n";
                std::this_thread::sleep_for(config_.daily_limit_delay);
                continue;
            }
            if (is_inventory_depleted(error)) {
                std::cout << "这一轮的" << policy_.reward_name << "已经发完了，稍后再试。\n";
                std::this_thread::sleep_for(config_.inventory_depleted_delay);
                continue;
            }
            if (is_no_open_round(error) || is_pool_disabled(error) || is_round_closed(error)) {
                std::cout << "当前还没有可抢的轮次，稍后会自动重试。\n";
                std::this_thread::sleep_for(config_.retry_delay);
                continue;
            }
            throw;
        } catch (const std::exception& error) {
            if (stop_requested()) {
                std::cout << "已收到停止请求，正在返回。\n";
                return;
            }
            std::cout << "这一轮没有顺利完成：" << error.what() << "。稍后会自动重试。\n";
            std::this_thread::sleep_for(config_.retry_delay);
        }
    }
}

void RewardRunner::run_cycle(const SolverConfig& solver_config) {
    std::cout << "先获取矿池状态。\n";
    const auto status = pool_client_.get_status();
    std::cout << "当前是第 #" << status.current_round->id
              << " 轮，难度 " << status.current_round->difficulty_bits
              << "，还剩 " << status.inventory_remaining_for(policy_.kind) << " 个" << policy_.reward_name << "。\n";

    std::cout << "开始获取挑战。\n";
    const auto challenge = pool_client_.get_challenge();
    std::cout << "拿到挑战：挑战 #" << challenge.challenge_id
              << "，第 #" << challenge.round_id
              << " 轮，难度 " << challenge.difficulty_bits << "。\n";

    Job job(job_config_from_challenge(challenge));
    auto current_config = solver_config;
    if (current_config.batch_size == 0) {
        current_config = solver_.default_config_for(job);
        std::cout << "这次自动选出的批大小是 " << current_config.batch_size << "。\n";
    }

    std::atomic_bool stop{stop_requested()};
    attach_active_stop(&stop);
    std::atomic<std::int64_t> attempts{0};
    std::uint64_t next_nonce = 1;

    const auto heartbeat_started_at = std::chrono::steady_clock::now();
    auto next_round_status_poll = heartbeat_started_at + config_.round_status_poll_interval;
    auto next_heartbeat = heartbeat_started_at + config_.heartbeat_interval;
    auto next_progress = heartbeat_started_at + config_.progress_interval;

    for (;;) {
        if (stop_requested()) {
            stop.store(true);
        }
        const auto now = std::chrono::steady_clock::now();
        if (now >= next_round_status_poll) {
            try {
                ensure_round_active(pool_client_.get_status_snapshot(), challenge.round_id, policy_.kind);
            } catch (...) {
                stop.store(true);
                detach_active_stop(&stop);
                throw;
            }
            next_round_status_poll += config_.round_status_poll_interval;
        }

        if (now >= next_heartbeat) {
            try {
                pool_client_.heartbeat(HeartbeatRequest{.challenge_id = challenge.challenge_id, .round_id = challenge.round_id});
            } catch (...) {
                stop.store(true);
                detach_active_stop(&stop);
                throw;
            }
            next_heartbeat += config_.heartbeat_interval;
        }

        if (now >= next_progress) {
            std::cout << "目前已经尝试了 " << attempts.load() << " 次。\n";
            next_progress += config_.progress_interval;
        }

        const auto result = solver_.mine_batch(job, current_config, next_nonce, stop, attempts);
        next_nonce += current_config.batch_size;
        if (!result.found) {
            if (stop.load()) {
                detach_active_stop(&stop);
                throw PoolError(PoolErrorCode::round_closed, "round_closed");
            }
            continue;
        }

        std::cout << "找到解了：随机数是 " << result.nonce << "，摘要是 " << result.digest << "，一共尝试了 " << attempts.load() << " 次。\n";
        std::cout << "开始提交结果。\n";
        const auto submit_response = pool_client_.submit(SubmitRequest{
            .challenge_id = challenge.challenge_id,
            .round_id = challenge.round_id,
            .nonce = std::to_string(result.nonce),
            .digest = result.digest,
            .preference = policy_.preference,
        });

        std::cout << "提交结果已经返回：结果是" << result_label(submit_response.result) << "。\n";
        if (submit_response.result == kResultLate || submit_response.result == kResultRoundClosed) {
            std::cout << "提交晚了一步，这一轮已经关了。\n";
            detach_active_stop(&stop);
            return;
        }
        if (submit_response.result == kResultDailyWinLimitReached) {
            detach_active_stop(&stop);
            throw PoolError(PoolErrorCode::daily_limit, "daily_limit");
        }

        const auto reward_code = submit_response.preferred_reward_code(policy_.kind);
        if (!reward_code.empty()) {
            std::cout << "命中了" << policy_.reward_name << "：" << reward_code << "\n";
            code_store_.save(reward_code);
            std::cout << policy_.reward_name << "已经保存到 " << display_path(code_store_.path()) << "：" << reward_code << "\n";
            if (policy_.reset_client_after_success) {
                pool_client_.reset();
                std::cout << "网络客户端已经重建，旧登录会话不会再用了。\n";
            }
        }
        detach_active_stop(&stop);
        return;
    }
}

} // namespace app
