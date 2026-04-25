#include "app/balance_code_store.hpp"

#include <chrono>
#include <ctime>
#include <fstream>
#include <iomanip>
#include <sstream>

namespace app {

BalanceCodeStore::BalanceCodeStore(std::filesystem::path path) : path_(std::move(path)) {
}

const std::filesystem::path& BalanceCodeStore::path() const noexcept {
    return path_;
}

void BalanceCodeStore::save(const std::string& code) {
    std::lock_guard lock(mutex_);
    std::error_code ec;
    if (auto parent = path_.parent_path(); !parent.empty()) {
        std::filesystem::create_directories(parent, ec);
    }

    std::ofstream out(path_, std::ios::app);
    if (!out) {
        throw std::runtime_error("无法打开余额兑换码保存文件");
    }

    const auto now = std::chrono::system_clock::now();
    const auto time = std::chrono::system_clock::to_time_t(now);
    std::tm local_tm{};
    localtime_s(&local_tm, &time);

    std::ostringstream stamp;
    stamp << std::put_time(&local_tm, "%Y-%m-%d %H:%M:%S");
    out << '[' << stamp.str() << "] 已保存余额兑换码：" << code << '\n';
}

} // namespace app
