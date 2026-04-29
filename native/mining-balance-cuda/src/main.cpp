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
        std::string mode = "run";
        std::size_t batch_size = 0;

        if (argc > 1) {
            mode = argv[1];
            if (mode != "run" && mode != "auto" && mode != "bench") {
                try {
                    batch_size = static_cast<std::size_t>(std::stoull(mode));
                    mode = "run";
                } catch (...) {
                    mode = "run";
                }
            }
        }

        auto config = app::default_config(batch_size);
        app::Runner runner(config);

        if (mode == "bench") {
            return run_with_escape_stop(runner, [&] { runner.run_benchmark(); });
        }
        if (mode == "auto") {
            return run_with_escape_stop(runner, [&] { runner.run_auto_tuned(); });
        }

        return run_with_escape_stop(runner, [&] { runner.run(); });
    } catch (const std::exception& error) {
        std::cerr << error.what() << '\n';
        return 1;
    }
}
