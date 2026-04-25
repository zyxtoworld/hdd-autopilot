#include "app/path.hpp"

#include <windows.h>

#include <filesystem>
#include <optional>

namespace app {
namespace {

bool file_exists(const std::filesystem::path& path) {
    std::error_code ec;
    return std::filesystem::is_regular_file(path, ec);
}

std::optional<std::filesystem::path> find_nearest_root_dir(std::filesystem::path current) {
    std::error_code ec;
    current = std::filesystem::weakly_canonical(current, ec);
    if (ec) {
        current = std::filesystem::absolute(current, ec);
        ec.clear();
    }
    current = current.lexically_normal();

    while (!current.empty()) {
        if (file_exists(current / "go.mod")) {
            return current;
        }
        auto parent = current.parent_path();
        if (parent == current) {
            break;
        }
        current = parent;
    }
    return std::nullopt;
}

std::optional<std::filesystem::path> find_nearest_module_dir(std::filesystem::path current) {
    std::error_code ec;
    current = std::filesystem::weakly_canonical(current, ec);
    if (ec) {
        current = std::filesystem::absolute(current, ec);
        ec.clear();
    }
    current = current.lexically_normal();

    while (!current.empty()) {
        if (file_exists(current / "CMakeLists.txt")) {
            return current;
        }
        auto parent = current.parent_path();
        if (parent == current) {
            break;
        }
        current = parent;
    }
    return std::nullopt;
}

std::optional<std::filesystem::path> find_root_from_runtime_dir(std::filesystem::path start) {
    if (auto root_dir = find_nearest_root_dir(start)) {
        return root_dir;
    }
    if (auto module_dir = find_nearest_module_dir(start)) {
        auto parent = module_dir->parent_path();
        if (parent.filename() == "native") {
            return parent.parent_path();
        }
        return parent;
    }
    auto current = start.lexically_normal();
    if (current.filename() == "dist") {
        return current.parent_path();
    }
    return current;
}

std::optional<std::filesystem::path> find_root_from_working_dir() {
    std::error_code ec;
    auto current = std::filesystem::current_path(ec);
    if (ec) {
        return std::nullopt;
    }
    return find_root_from_runtime_dir(current);
}

std::optional<std::filesystem::path> find_root_from_executable() {
    wchar_t buffer[MAX_PATH];
    const auto size = GetModuleFileNameW(nullptr, buffer, MAX_PATH);
    if (size == 0 || size >= MAX_PATH) {
        return std::nullopt;
    }

    const auto executable_dir = std::filesystem::path(buffer).parent_path();
    return find_root_from_runtime_dir(executable_dir);
}

} // namespace

std::filesystem::path resolve_data_file_path(const std::string& name) {
    if (auto root = find_root_from_working_dir()) {
        return *root / name;
    }
    if (auto root = find_root_from_executable()) {
        return *root / name;
    }
    return std::filesystem::path("..") / name;
}

} // namespace app
