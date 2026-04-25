#pragma once

#include <filesystem>
#include <string>

namespace app {

std::filesystem::path resolve_data_file_path(const std::string& name);

} // namespace app
