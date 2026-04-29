#include "app/runner.hpp"

#include <algorithm>
#include <atomic>
#include <cctype>
#include <chrono>
#include <cstdint>
#include <iostream>
#include <mutex>
#include <thread>
#include <utility>
#include <vector>

namespace app {
namespace {

class RetryNowError final : public std::runtime_error {
public:
    RetryNowError() : std::runtime_error("retry_now") {
    }
};

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

bool contains_ascii_alpha(const std::string& text) {
    for (unsigned char ch : text) {
        if ((ch >= 'a' && ch <= 'z') || (ch >= 'A' && ch <= 'Z')) {
            return true;
        }
    }
    return false;
}

std::string fallback_visible_text(const std::string& value, const std::string& fallback) {
    const auto trimmed = trim_copy(value);
    if (trimmed.empty()) {
        return fallback;
    }
    if (contains_ascii_alpha(trimmed)) {
        return fallback;
    }
    return trimmed;
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
    return fallback_visible_text(trimmed, "未说明");
}

std::string preference_label(const std::string& value) {
    const auto trimmed = trim_copy(value);
    if (trimmed == "invite") {
        return "邀请码";
    }
    if (trimmed == "balance") {
        return "余额兑换码";
    }
    return fallback_visible_text(trimmed, "未说明");
}

std::string code_type_label(const std::string& value) {
    const auto trimmed = trim_copy(value);
    if (trimmed == "invite") {
        return "邀请码";
    }
    if (trimmed == "balance") {
        return "余额兑换码";
    }
    if (trimmed.empty() || trimmed == "none") {
        return "无";
    }
    return fallback_visible_text(trimmed, "未说明");
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

Runner::Runner(Config config)
    : config_(std::move(config)),
      pool_client_(config_),
      invite_store_(config_.invite_output_file),
      balance_code_store_(config_.balance_output_file),
      solver_(config_.device_index) {
}

void Runner::request_stop() noexcept {
    stop_requested_.store(true);
    std::lock_guard<std::mutex> lock(active_stop_mutex_);
    if (active_stop_ != nullptr) {
        active_stop_->store(true);
    }
}

bool Runner::stop_requested() const noexcept {
    return stop_requested_.load();
}

void Runner::reset_stop_request() noexcept {
    stop_requested_.store(false);
}

void Runner::attach_active_stop(std::atomic_bool* stop) noexcept {
    std::lock_guard<std::mutex> lock(active_stop_mutex_);
    active_stop_ = stop;
    if (active_stop_ != nullptr && stop_requested_.load()) {
        active_stop_->store(true);
    }
}

void Runner::detach_active_stop(std::atomic_bool* stop) noexcept {
    std::lock_guard<std::mutex> lock(active_stop_mutex_);
    if (active_stop_ == stop) {
        active_stop_ = nullptr;
    }
}

void Runner::run() {
    reset_stop_request();
    std::cout << "开始运行 mining-cuda：当前模式是" << mode_description(config_.mode) << "。\n";
    if (config_.batch_size == 0) {
        std::cout << "这次的批大小会自动选择。\n";
    } else {
        std::cout << "这次的批大小是 " << config_.batch_size << "。\n";
    }
    std::cout << "启动前会先校验显卡求解器和难度规则。\n";
    validate_solver_basics(solver_);
    std::cout << "命中的邀请码会保存到：" << display_path(invite_store_.path()) << "\n";
    std::cout << "命中的余额兑换码会保存到：" << display_path(balance_code_store_.path()) << "\n";
    run_loop(SolverConfig{
        .batch_size = config_.batch_size,
        .by_segment = false,
        .precompute_refs = false,
    });
}

void Runner::run_auto_tuned() {
    reset_stop_request();
    std::cout << "开始运行挖矿自动调优模式：先按显卡推荐配置挖矿。\n";
    std::cout << "启动前会先校验显卡求解器和难度规则。\n";
    validate_solver_basics(solver_);
    const auto best = solver_.find_best_benchmark_config();
    std::cout << "已应用推荐配置：批大小 " << best.config.batch_size
              << "，按分段 " << (best.config.by_segment ? "是" : "否")
              << "，预计算参考值 " << (best.config.precompute_refs ? "是" : "否")
              << "，预计速度约 " << best.attempts_per_second << " 次/秒。\n";
    std::cout << "当前模式：" << mode_description(config_.mode) << "。\n";
    std::cout << "命中的邀请码会保存到：" << display_path(invite_store_.path()) << "\n";
    std::cout << "命中的余额兑换码会保存到：" << display_path(balance_code_store_.path()) << "\n";
    run_loop(best.config);
}

void Runner::run_benchmark() {
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

void Runner::run_loop(const SolverConfig& solver_config) {
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
        } catch (const RetryNowError&) {
            if (stop_requested()) {
                std::cout << "已收到停止请求，正在返回。\n";
                return;
            }
            continue;
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
                std::cout << "这一轮的邀请码和余额兑换码都已经发完了，稍后再试。\n";
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

std::vector<RewardKind> Runner::preferred_reward_order() const {
    switch (config_.mode) {
    case MiningMode::invite_then_balance:
        return {RewardKind::invite, RewardKind::balance};
    case MiningMode::balance_then_invite:
        return {RewardKind::balance, RewardKind::invite};
    case MiningMode::invite_only:
        return {RewardKind::invite};
    case MiningMode::balance_only:
        return {RewardKind::balance};
    }
    return {RewardKind::invite, RewardKind::balance};
}

RewardKind Runner::select_reward_kind(const StatusResponse& status) const {
    for (const auto target : preferred_reward_order()) {
        if (remaining_for(status, target) > 0) {
            return target;
        }
    }
    throw PoolError(PoolErrorCode::inventory_depleted, "inventory_depleted");
}

int Runner::remaining_for(const StatusResponse& status, RewardKind target) const {
    if (target == RewardKind::invite) {
        return status.invite_inventory_remaining();
    }
    return status.balance_inventory_remaining();
}

const char* Runner::name_for(RewardKind target) const {
    if (target == RewardKind::invite) {
        return "邀请码";
    }
    return "余额兑换码";
}

const char* Runner::preference_for(RewardKind target) const {
    if (target == RewardKind::invite) {
        return "invite";
    }
    return "balance";
}

const std::filesystem::path& Runner::output_path_for(RewardKind target) const {
    if (target == RewardKind::invite) {
        return invite_store_.path();
    }
    return balance_code_store_.path();
}

void Runner::save_code(RewardKind target, const std::string& code) {
    if (target == RewardKind::invite) {
        invite_store_.save(code);
        return;
    }
    balance_code_store_.save(code);
}

void Runner::run_cycle(const SolverConfig& solver_config) {
    std::cout << "先获取矿池状态。\n";
    const auto status = pool_client_.get_status();
    const auto invite_left = status.invite_inventory_remaining();
    const auto balance_left = status.balance_inventory_remaining();
    std::cout << "当前是第 #" << status.current_round->id
              << " 轮，难度 " << status.current_round->difficulty_bits
              << "，还剩 " << invite_left << " 个邀请码"
              << "，还剩 " << balance_left << " 个余额兑换码。\n";

    const auto target = select_reward_kind(status);
    if (target == RewardKind::balance && invite_left <= 0) {
        std::cout << "邀请码已经没有库存了，这一轮改为尝试余额兑换码。\n";
    }
    if (target == RewardKind::invite && balance_left <= 0 && config_.mode == MiningMode::invite_then_balance) {
        std::cout << "余额兑换码已经没有库存了，这一轮继续尝试邀请码。\n";
    }
    std::cout << "这一轮会优先尝试：" << name_for(target) << "。\n";

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
    auto session = solver_.create_session(job, current_config, 1);

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
            const auto snapshot = pool_client_.get_status_snapshot();
            if (!snapshot.enabled || !snapshot.current_round.has_value() || !snapshot.current_round->is_open() || snapshot.current_round->id != challenge.round_id) {
                stop.store(true);
                detach_active_stop(&stop);
                throw PoolError(PoolErrorCode::round_closed, "round_closed");
            }
            if (remaining_for(snapshot, target) <= 0) {
                stop.store(true);
                detach_active_stop(&stop);
                throw RetryNowError();
            }
            if (snapshot.daily_limit_reached()) {
                stop.store(true);
                detach_active_stop(&stop);
                throw PoolError(PoolErrorCode::daily_limit, "daily_limit");
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

        const auto result = solver_.mine_next_batch(job, session, stop, attempts);
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
            .preference = preference_for(target),
        });

        std::cout << "提交结果已经返回：这次想要的是" << preference_label(preference_for(target))
                  << "，实际拿到的是" << code_type_label(submit_response.code_type)
                  << "，结果是" << result_label(submit_response.result)
                  << "，余额面额 " << submit_response.balance_amount
                  << "，奖励编号 " << submit_response.reward_code_id << "。\n";
        if (submit_response.result == kResultLate || submit_response.result == kResultRoundClosed) {
            std::cout << "提交晚了一步，这一轮已经关了。\n";
            detach_active_stop(&stop);
            return;
        }
        if (submit_response.result == kResultDailyWinLimitReached) {
            detach_active_stop(&stop);
            throw PoolError(PoolErrorCode::daily_limit, "daily_limit");
        }

        if (!submit_response.reward_code.empty()) {
            std::cout << "命中了" << name_for(target) << "：" << submit_response.reward_code << "\n";
            save_code(target, submit_response.reward_code);
            std::cout << name_for(target) << "已经保存到 " << display_path(output_path_for(target)) << "：" << submit_response.reward_code << "\n";
            pool_client_.reset();
            std::cout << "网络客户端已经重建，旧登录会话不会再用了。\n";
        }
        detach_active_stop(&stop);
        return;
    }
}

} // namespace app
