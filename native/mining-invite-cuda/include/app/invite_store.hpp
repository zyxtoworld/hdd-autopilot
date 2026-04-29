#pragma once

#include <filesystem>
#include <mutex>
#include <string>

namespace app {

class InviteStore {
public:
    explicit InviteStore(std::filesystem::path path);

    const std::filesystem::path& path() const noexcept;
    void save(const std::string& code);

private:
    std::filesystem::path path_;
    mutable std::mutex mutex_;
};

} // namespace app
