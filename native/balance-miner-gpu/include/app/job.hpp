#pragma once

#include <cstddef>
#include <cstdint>
#include <string>
#include <vector>

namespace app {

struct JobConfig {
    std::string seed;
    int round_id = 0;
    std::string visitor_id;
    int challenge_id = 0;
    std::string session_salt;
    int time_cost = 1;
    int memory_cost_mb = 1;
    int parallelism = 1;
    int difficulty_bits = 0;
};

class Job {
public:
    explicit Job(JobConfig config);

    const std::string& seed() const noexcept;
    const std::vector<std::uint8_t>& seed_bytes() const noexcept;
    const std::string& pass_prefix() const noexcept;
    std::string password_for_nonce(std::uint64_t nonce) const;
    std::uint32_t time_cost() const noexcept;
    std::uint32_t memory_cost_kb() const noexcept;
    std::uint32_t parallelism() const noexcept;
    int difficulty_bits() const noexcept;

private:
    std::vector<std::uint8_t> seed_bytes_;
    std::string seed_;
    std::string pass_prefix_;
    std::uint32_t time_cost_ = 1;
    std::uint32_t memory_cost_kb_ = 1024;
    std::uint32_t parallelism_ = 1;
    int difficulty_bits_ = 0;
};

bool meets_difficulty(const std::vector<std::uint8_t>& digest, int difficulty_bits);
bool meets_difficulty(const std::uint8_t* digest, std::size_t digest_size, int difficulty_bits);
std::string hex_encode(const std::uint8_t* data, std::size_t size);

} // namespace app
