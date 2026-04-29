#pragma once

#include <filesystem>
#include <mutex>
#include <string>

namespace app {

class CodeStore {
public:
    CodeStore(std::filesystem::path path, std::string label);

    const std::filesystem::path& path() const noexcept;
    void save(const std::string& code);

private:
    std::filesystem::path path_;
    std::string label_;
    mutable std::mutex mutex_;
};

} // namespace app
