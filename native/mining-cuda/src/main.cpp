#include <atomic>
#include <exception>
#include <iostream>
#include <string>
#include <thread>
#include <utility>

#include <windows.h>

#include "app/config.hpp"
#include "app/runner.hpp"

namespace {

void prepare_console() {
    SetConsoleCP(65001);
    SetConsoleOutputCP(65001);
}

app::MiningMode parse_mining_mode(const std::string& value) {
    if (value == "invite_then_balance") {
        return app::MiningMode::invite_then_balance;
    }
    if (value == "balance_then_invite") {
        return app::MiningMode::balance_then_invite;
    }
    if (value == "invite_only") {
        return app::MiningMode::invite_only;
    }
    if (value == "balance_only") {
        return app::MiningMode::balance_only;
    }
    throw std::runtime_error("不支持的 GPU 挖矿模式参数");
}

template <typename RunFn>
int run_with_escape_stop(app::Runner& runner, RunFn&& run_fn) {
    std::atomic_bool finished{false};
    std::exception_ptr worker_error;
    std::thread worker([&] {
        try {
            std::forward<RunFn>(run_fn)();
        } catch (...) {
            worker_error = std::current_exception();
        }
        finished.store(true);
    });

    bool stop_requested = false;
    while (!finished.load()) {
        if (!stop_requested && (GetAsyncKeyState(VK_ESCAPE) & 0x8000) != 0) {
            stop_requested = true;
            std::cout << "已收到 ESC，正在请求停止，可能要等当前批次或调优步骤结束后返回。\n";
            runner.request_stop();
        }
        Sleep(50);
    }

    worker.join();
    if (worker_error) {
        std::rethrow_exception(worker_error);
    }
    return 0;
}

} // namespace

int main(int argc, char** argv) {
    prepare_console();
    try {
        std::string command = "run";
        std::size_t batch_size = 0;
        app::MiningMode mining_mode = app::MiningMode::invite_then_balance;

        int index = 1;
        if (index < argc) {
            command = argv[index];
            if (command != "run" && command != "auto" && command != "bench") {
                try {
                    batch_size = static_cast<std::size_t>(std::stoull(command));
                    command = "run";
                    ++index;
                } catch (...) {
                    command = "run";
                }
            } else {
                ++index;
            }
        }

        while (index < argc) {
            const std::string arg = argv[index];
            if (arg == "--mode") {
                ++index;
                if (index >= argc) {
                    throw std::runtime_error("--mode 缺少参数");
                }
                mining_mode = parse_mining_mode(argv[index]);
                ++index;
                continue;
            }
            throw std::runtime_error("不支持的参数");
        }

        auto config = app::default_config(batch_size, mining_mode);
        app::Runner runner(config);

        if (command == "bench") {
            return run_with_escape_stop(runner, [&] { runner.run_benchmark(); });
        }
        if (command == "auto") {
            return run_with_escape_stop(runner, [&] { runner.run_auto_tuned(); });
        }

        return run_with_escape_stop(runner, [&] { runner.run(); });
    } catch (const std::exception& error) {
        std::cerr << error.what() << '\n';
        return 1;
    }
}
