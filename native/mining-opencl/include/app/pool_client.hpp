#pragma once

#include <chrono>
#include <mutex>
#include <stdexcept>
#include <string>
#include <unordered_map>

#include <windows.h>
#include <winhttp.h>

#include "app/config.hpp"
#include "app/pool_types.hpp"

namespace app {

enum class PoolErrorCode {
    challenge_rejected,
    daily_limit,
    inventory_depleted,
    no_open_round,
    pool_disabled,
    round_closed,
};

class PoolError : public std::runtime_error {
public:
    PoolError(PoolErrorCode code, std::string message);

    PoolErrorCode code() const noexcept;

private:
    PoolErrorCode code_;
};

class PoolClient {
public:
    explicit PoolClient(const Config& config);
    ~PoolClient();

    PoolClient(const PoolClient&) = delete;
    PoolClient& operator=(const PoolClient&) = delete;

    void reset();

    StatusResponse get_status();
    StatusResponse get_status_snapshot();
    ChallengeResponse get_challenge();
    HeartbeatResponse heartbeat(const HeartbeatRequest& request);
    SubmitResponse submit(const SubmitRequest& request);

private:
    bool secure_ = true;
    INTERNET_PORT port_ = INTERNET_DEFAULT_HTTPS_PORT;
    std::wstring host_;
    std::wstring base_path_;
    HINTERNET session_ = nullptr;
    HINTERNET connection_ = nullptr;
    std::chrono::milliseconds timeout_{};

    std::mutex mutex_;
    std::unordered_map<std::wstring, std::wstring> cookies_;

    nlohmann::json get_json(const std::wstring& path);
    nlohmann::json post_json(const std::wstring& path, const nlohmann::json* body);
    nlohmann::json request_json(const wchar_t* method, const std::wstring& path, const nlohmann::json* body);

    void parse_base_url(const std::string& url);
    std::wstring build_request_path(const std::wstring& path) const;
    void apply_cookies_locked(HINTERNET request) const;
    void store_response_cookies_locked(HINTERNET request);
};

} // namespace app
