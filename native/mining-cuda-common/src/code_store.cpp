#include "app_shared/code_store.hpp"

#include <chrono>
#include <ctime>
#include <fstream>
#include <iomanip>
#include <sstream>
#include <system_error>
#include <utility>

namespace app {

CodeStore::CodeStore(std::filesystem::path path, std::string label)
    : path_(std::move(path)), label_(std::move(label)) {
}

const std::filesystem::path& CodeStore::path() const noexcept {
    return path_;
}

void CodeStore::save(const std::string& code) {
    std::lock_guard lock(mutex_);
    std::error_code ec;
    if (auto parent = path_.parent_path(); !parent.empty()) {
        std::filesystem::create_directories(parent, ec);
    }

    std::ofstream out(path_, std::ios::app);
    if (!out) {
        throw std::runtime_error("无法打开" + label_ + "保存文件");
    }

    const auto now = std::chrono::system_clock::now();
    const auto time = std::chrono::system_clock::to_time_t(now);
    std::tm local_tm{};
    localtime_s(&local_tm, &time);

    std::ostringstream stamp;
    stamp << std::put_time(&local_tm, "%Y-%m-%d %H:%M:%S");
    out << '[' << stamp.str() << "] 已保存" << label_ << "：" << code << '\n';
}

} // namespace app
