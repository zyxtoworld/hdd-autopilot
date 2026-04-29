#include "app/job.hpp"

#include <iomanip>
#include <sstream>

namespace app {

Job::Job(JobConfig config)
    : seed_bytes_(config.seed.begin(), config.seed.end()),
      seed_(std::move(config.seed)),
      pass_prefix_(config.pass_prefix_override.empty()
          ? (seed_ + ":" + std::to_string(config.round_id) + ":" + config.visitor_id + ":" + std::to_string(config.challenge_id) + ":" + config.session_salt + ":")
          : std::move(config.pass_prefix_override)),
      time_cost_(static_cast<std::uint32_t>(config.time_cost)),
      memory_cost_kb_(static_cast<std::uint32_t>(config.memory_cost_mb) * 1024u),
      parallelism_(static_cast<std::uint32_t>(config.parallelism > 0 ? config.parallelism : 1)),
      difficulty_bits_(config.difficulty_bits) {
}

const std::string& Job::seed() const noexcept {
    return seed_;
}

const std::vector<std::uint8_t>& Job::seed_bytes() const noexcept {
    return seed_bytes_;
}

const std::string& Job::pass_prefix() const noexcept {
    return pass_prefix_;
}

void Job::write_password_for_nonce(std::string& output, std::uint64_t nonce) const {
    output.clear();
    output.append(pass_prefix_);
    output.append(std::to_string(nonce));
}

std::string Job::password_for_nonce(std::uint64_t nonce) const {
    std::string output;
    output.reserve(pass_prefix_.size() + 20);
    write_password_for_nonce(output, nonce);
    return output;
}

std::uint32_t Job::time_cost() const noexcept {
    return time_cost_;
}

std::uint32_t Job::memory_cost_kb() const noexcept {
    return memory_cost_kb_;
}

std::uint32_t Job::parallelism() const noexcept {
    return parallelism_;
}

int Job::difficulty_bits() const noexcept {
    return difficulty_bits_;
}

bool meets_difficulty(const std::uint8_t* digest, std::size_t digest_size, int difficulty_bits) {
    if (difficulty_bits < 0) {
        return false;
    }

    const auto full_bytes = static_cast<std::size_t>(difficulty_bits / 8);
    for (std::size_t i = 0; i < full_bytes; ++i) {
        if (i >= digest_size || digest[i] != 0) {
            return false;
        }
    }

    const auto remaining_bits = difficulty_bits % 8;
    if (remaining_bits == 0) {
        return true;
    }
    if (full_bytes >= digest_size) {
        return false;
    }

    const auto mask = static_cast<std::uint8_t>(0xFFu << (8 - remaining_bits));
    return (digest[full_bytes] & mask) == 0;
}

bool meets_difficulty(const std::vector<std::uint8_t>& digest, int difficulty_bits) {
    return meets_difficulty(digest.data(), digest.size(), difficulty_bits);
}

std::string hex_encode(const std::uint8_t* data, std::size_t size) {
    std::ostringstream out;
    out << std::hex << std::setfill('0');
    for (std::size_t i = 0; i < size; ++i) {
        out << std::setw(2) << static_cast<int>(data[i]);
    }
    return out.str();
}

} // namespace app
