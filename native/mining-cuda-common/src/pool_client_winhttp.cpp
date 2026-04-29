#include "app_shared/pool_client.hpp"

#include <algorithm>
#include <cctype>
#include <sstream>
#include <vector>

#include <nlohmann/json.hpp>

namespace app {
namespace {

std::wstring utf8_to_wide(const std::string& input) {
    if (input.empty()) {
        return {};
    }
    const auto size = MultiByteToWideChar(CP_UTF8, 0, input.data(), static_cast<int>(input.size()), nullptr, 0);
    if (size <= 0) {
        throw std::runtime_error("UTF-8 转 UTF-16 失败");
    }
    std::wstring output(size, L'\0');
    MultiByteToWideChar(CP_UTF8, 0, input.data(), static_cast<int>(input.size()), output.data(), size);
    return output;
}

std::string read_response_body(HINTERNET request) {
    std::string body;
    for (;;) {
        DWORD available = 0;
        if (!WinHttpQueryDataAvailable(request, &available)) {
            throw std::runtime_error("读取响应数据长度失败");
        }
        if (available == 0) {
            break;
        }
        const auto start = body.size();
        body.resize(start + available);
        DWORD read = 0;
        if (!WinHttpReadData(request, body.data() + start, available, &read)) {
            throw std::runtime_error("读取响应数据失败");
        }
        body.resize(start + read);
    }
    return body;
}

std::wstring trim_copy(std::wstring value) {
    const auto is_space = [](wchar_t ch) {
        return ch == L' ' || ch == L'\t' || ch == L'\r' || ch == L'\n';
    };
    while (!value.empty() && is_space(value.front())) {
        value.erase(value.begin());
    }
    while (!value.empty() && is_space(value.back())) {
        value.pop_back();
    }
    return value;
}

std::wstring extract_cookie_pair(const std::wstring& header) {
    const auto end = header.find(L';');
    return trim_copy(header.substr(0, end));
}

std::wstring join_cookie_pairs(const std::unordered_map<std::wstring, std::wstring>& cookies) {
    std::wstring joined;
    for (const auto& [name, value] : cookies) {
        if (!joined.empty()) {
            joined += L"; ";
        }
        joined += name;
        joined += L'=';
        joined += value;
    }
    return joined;
}

std::string trim_copy(std::string value) {
    const auto is_space = [](unsigned char ch) {
        return std::isspace(ch) != 0;
    };
    value.erase(value.begin(), std::find_if(value.begin(), value.end(), [&](unsigned char ch) { return !is_space(ch); }));
    value.erase(std::find_if(value.rbegin(), value.rend(), [&](unsigned char ch) { return !is_space(ch); }).base(), value.end());
    return value;
}

std::string lower_copy(std::string value) {
    std::transform(value.begin(), value.end(), value.begin(), [](unsigned char ch) {
        return static_cast<char>(std::tolower(ch));
    });
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

std::string localized_visible_text(const std::string& text, const std::string& fallback) {
    const auto trimmed = trim_copy(text);
    if (trimmed.empty()) {
        return fallback;
    }
    const auto lower = lower_copy(trimmed);
    if (lower == "invalid email or password") {
        return "邮箱或密码错误";
    }
    if (lower.find("daily win limit reached") != std::string::npos || lower.find("daily limit reached") != std::string::npos) {
        return "今日命中次数已达上限";
    }
    if (lower.find("no open round") != std::string::npos) {
        return "当前没有开放轮次";
    }
    if (lower.find("round closed") != std::string::npos) {
        return "当前轮次已关闭";
    }
    if (lower.find("pool disabled") != std::string::npos) {
        return "矿池当前未开放";
    }
    if (lower.find("challenge rejected") != std::string::npos) {
        return "挑战被矿池拒绝";
    }
    if (lower.find("inventory depleted") != std::string::npos) {
        return "当前目标库存已耗尽";
    }
    if (lower.find("unauthorized") != std::string::npos || lower.find("invalid token") != std::string::npos) {
        return "登录状态已失效，请重新登录";
    }
    return fallback_visible_text(trimmed, fallback);
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

std::string localized_status_message(DWORD status_code, const std::string& response_text) {
    const auto json = nlohmann::json::parse(response_text, nullptr, false);
    if (json.is_object()) {
        const auto reason = json.value("reason", "");
        const auto message = json.value("message", "");
        if (status_code == 401 && (reason == "INVALID_CREDENTIALS" || lower_copy(message) == "invalid email or password")) {
            return "邮箱或密码错误";
        }
        if (status_code == 401) {
            return "登录状态已失效，请重新登录";
        }
        if (!trim_copy(message).empty()) {
            std::ostringstream stream;
            stream << "请求失败（状态码 " << status_code << "）：" << localized_visible_text(message, "服务端返回错误");
            return stream.str();
        }
    }
    if (status_code == 401) {
        return "登录状态已失效，请重新登录";
    }
    if (trim_copy(response_text).empty()) {
        std::ostringstream stream;
        stream << "请求失败（状态码 " << status_code << "）";
        return stream.str();
    }
    std::ostringstream stream;
    stream << "请求失败（状态码 " << status_code << "）：" << localized_visible_text(response_text, "服务端返回错误");
    return stream.str();
}

PoolError challenge_error(const ChallengeResponse& response) {
    if (response.result == kResultDailyWinLimitReached || response.message == kResultDailyWinLimitReached) {
        return PoolError(PoolErrorCode::daily_limit, "daily_limit");
    }
    if (!response.message.empty()) {
        return PoolError(PoolErrorCode::challenge_rejected, "挑战被矿池拒绝：" + localized_visible_text(response.message, "服务端返回错误"));
    }
    if (!response.result.empty()) {
        return PoolError(PoolErrorCode::challenge_rejected, "挑战被矿池拒绝：" + result_label(response.result));
    }
    return PoolError(PoolErrorCode::challenge_rejected, "挑战被矿池拒绝");
}

std::wstring build_default_headers(bool has_body) {
    std::wstring headers = L"Accept: application/json, text/plain, */*\r\n"
                           L"Accept-Language: zh\r\n"
                           L"Origin: https://sub.hdd.sb\r\n"
                           L"Referer: https://sub.hdd.sb/dashboard\r\n"
                           L"User-Agent: Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/147.0.0.0 Safari/537.36 Edg/147.0.0.0\r\n";
    if (has_body) {
        headers += L"Content-Type: application/json\r\n";
    }
    return headers;
}

} // namespace

PoolError::PoolError(PoolErrorCode code, std::string message) : std::runtime_error(std::move(message)), code_(code) {
}

PoolErrorCode PoolError::code() const noexcept {
    return code_;
}

PoolClient::PoolClient(const Config& config, RewardPolicy policy) : policy_(policy), timeout_(config.http_timeout) {
    parse_base_url(config.base_url);
    reset();
}

PoolClient::~PoolClient() {
    std::lock_guard lock(mutex_);
    if (connection_ != nullptr) {
        WinHttpCloseHandle(connection_);
    }
    if (session_ != nullptr) {
        WinHttpCloseHandle(session_);
    }
}

void PoolClient::reset() {
    std::lock_guard lock(mutex_);
    cookies_.clear();
    if (connection_ != nullptr) {
        WinHttpCloseHandle(connection_);
        connection_ = nullptr;
    }
    if (session_ != nullptr) {
        WinHttpCloseHandle(session_);
        session_ = nullptr;
    }

    session_ = WinHttpOpen(L"Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36",
                           WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY,
                           WINHTTP_NO_PROXY_NAME,
                           WINHTTP_NO_PROXY_BYPASS,
                           0);
    if (session_ == nullptr) {
        throw std::runtime_error("初始化 WinHTTP 会话失败");
    }

    const auto timeout_ms = static_cast<int>(timeout_.count());
    WinHttpSetTimeouts(session_, timeout_ms, timeout_ms, timeout_ms, timeout_ms);

    DWORD decompression = WINHTTP_DECOMPRESSION_FLAG_GZIP | WINHTTP_DECOMPRESSION_FLAG_DEFLATE;
    WinHttpSetOption(session_, WINHTTP_OPTION_DECOMPRESSION, &decompression, sizeof(decompression));

    connection_ = WinHttpConnect(session_, host_.c_str(), port_, 0);
    if (connection_ == nullptr) {
        throw std::runtime_error("连接服务器失败");
    }
}

StatusResponse PoolClient::get_status() {
    auto response = get_json(L"/mining-api/status").get<StatusResponse>();
    if (!response.enabled) {
        throw PoolError(PoolErrorCode::pool_disabled, "pool_disabled");
    }
    if (!response.current_round.has_value() || !response.current_round->is_open()) {
        throw PoolError(PoolErrorCode::no_open_round, "no_open_round");
    }
    if (response.inventory_remaining_for(policy_.kind) <= 0) {
        throw PoolError(PoolErrorCode::inventory_depleted, "inventory_depleted");
    }
    return response;
}

StatusResponse PoolClient::get_status_snapshot() {
    return get_json(L"/mining-api/status").get<StatusResponse>();
}

ChallengeResponse PoolClient::get_challenge() {
    auto response = post_json(L"/mining-api/challenge", nullptr).get<ChallengeResponse>();
    if (!response.ok) {
        throw challenge_error(response);
    }
    return response;
}

HeartbeatResponse PoolClient::heartbeat(const HeartbeatRequest& request) {
    nlohmann::json payload = request;
    auto response = post_json(L"/mining-api/heartbeat", &payload).get<HeartbeatResponse>();
    if (response.result == kResultRoundClosed) {
        throw PoolError(PoolErrorCode::round_closed, "round_closed");
    }
    return response;
}

SubmitResponse PoolClient::submit(const SubmitRequest& request) {
    nlohmann::json payload = request;
    return post_json(L"/mining-api/submit", &payload).get<SubmitResponse>();
}

nlohmann::json PoolClient::get_json(const std::wstring& path) {
    return request_json(L"GET", path, nullptr);
}

nlohmann::json PoolClient::post_json(const std::wstring& path, const nlohmann::json* body) {
    return request_json(L"POST", path, body);
}

nlohmann::json PoolClient::request_json(const wchar_t* method, const std::wstring& path, const nlohmann::json* body) {
    std::lock_guard lock(mutex_);

    const auto request_path = build_request_path(path);
    const DWORD flags = secure_ ? WINHTTP_FLAG_SECURE : 0;
    HINTERNET request = WinHttpOpenRequest(connection_, method, request_path.c_str(), nullptr, WINHTTP_NO_REFERER, WINHTTP_DEFAULT_ACCEPT_TYPES, flags);
    if (request == nullptr) {
        throw std::runtime_error("创建请求失败");
    }

    auto close_request = [&]() {
        WinHttpCloseHandle(request);
    };

    try {
        const auto headers = build_default_headers(body != nullptr);
        apply_cookies_locked(request);

        std::string body_text;
        LPCVOID body_ptr = WINHTTP_NO_REQUEST_DATA;
        DWORD body_size = 0;
        if (body != nullptr) {
            body_text = body->dump();
            body_ptr = body_text.data();
            body_size = static_cast<DWORD>(body_text.size());
        }

        if (!WinHttpSendRequest(request,
                                headers.c_str(),
                                static_cast<DWORD>(headers.size()),
                                const_cast<LPVOID>(body_ptr),
                                body_size,
                                body_size,
                                0)) {
            throw std::runtime_error("发送请求失败");
        }
        if (!WinHttpReceiveResponse(request, nullptr)) {
            throw std::runtime_error("接收响应失败");
        }

        DWORD status_code = 0;
        DWORD status_size = sizeof(status_code);
        if (!WinHttpQueryHeaders(request,
                                 WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
                                 WINHTTP_HEADER_NAME_BY_INDEX,
                                 &status_code,
                                 &status_size,
                                 WINHTTP_NO_HEADER_INDEX)) {
            throw std::runtime_error("读取响应状态码失败");
        }

        store_response_cookies_locked(request);
        auto response_text = read_response_body(request);
        close_request();
        request = nullptr;

        if (status_code < 200 || status_code >= 300) {
            throw std::runtime_error(localized_status_message(status_code, response_text));
        }
        if (response_text.empty()) {
            return nlohmann::json::object();
        }
        return nlohmann::json::parse(response_text);
    } catch (...) {
        if (request != nullptr) {
            close_request();
        }
        throw;
    }
}

void PoolClient::parse_base_url(const std::string& url) {
    const auto wide = utf8_to_wide(url);
    URL_COMPONENTS parts{};
    parts.dwStructSize = sizeof(parts);
    parts.dwSchemeLength = static_cast<DWORD>(-1);
    parts.dwHostNameLength = static_cast<DWORD>(-1);
    parts.dwUrlPathLength = static_cast<DWORD>(-1);
    parts.dwExtraInfoLength = static_cast<DWORD>(-1);
    if (!WinHttpCrackUrl(wide.c_str(), static_cast<DWORD>(wide.size()), 0, &parts)) {
        throw std::runtime_error("基础地址无效");
    }

    secure_ = parts.nScheme == INTERNET_SCHEME_HTTPS;
    port_ = parts.nPort;
    host_.assign(parts.lpszHostName, parts.dwHostNameLength);
    base_path_.assign(parts.lpszUrlPath, parts.dwUrlPathLength);
    if (parts.dwExtraInfoLength > 0) {
        base_path_.append(parts.lpszExtraInfo, parts.dwExtraInfoLength);
    }
    if (base_path_.empty()) {
        base_path_ = L"/";
    }
    if (base_path_.back() == L'/') {
        base_path_.pop_back();
    }
}

std::wstring PoolClient::build_request_path(const std::wstring& path) const {
    if (base_path_.empty() || base_path_ == L"/") {
        return path;
    }
    return base_path_ + path;
}

void PoolClient::apply_cookies_locked(HINTERNET request) const {
    if (cookies_.empty()) {
        return;
    }
    const auto cookie_header = join_cookie_pairs(cookies_);
    const auto header = std::wstring(L"Cookie: ") + cookie_header;
    WinHttpAddRequestHeaders(request, header.c_str(), static_cast<DWORD>(-1), WINHTTP_ADDREQ_FLAG_ADD);
}

void PoolClient::store_response_cookies_locked(HINTERNET request) {
    DWORD index = 0;
    for (;;) {
        DWORD size = 0;
        const auto ok = WinHttpQueryHeaders(request,
                                            WINHTTP_QUERY_SET_COOKIE,
                                            WINHTTP_HEADER_NAME_BY_INDEX,
                                            WINHTTP_NO_OUTPUT_BUFFER,
                                            &size,
                                            &index);
        if (!ok && GetLastError() == ERROR_INSUFFICIENT_BUFFER) {
            std::wstring buffer(size / sizeof(wchar_t), L'\0');
            if (!WinHttpQueryHeaders(request,
                                     WINHTTP_QUERY_SET_COOKIE,
                                     WINHTTP_HEADER_NAME_BY_INDEX,
                                     buffer.data(),
                                     &size,
                                     &index)) {
                break;
            }
            const auto header = trim_copy(buffer.c_str());
            const auto pair = extract_cookie_pair(header);
            const auto eq = pair.find(L'=');
            if (eq != std::wstring::npos) {
                cookies_[pair.substr(0, eq)] = pair.substr(eq + 1);
            }
            ++index;
            continue;
        }
        break;
    }
}

} // namespace app
