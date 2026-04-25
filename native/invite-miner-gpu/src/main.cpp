#include <exception>
#include <iostream>
#include <string>

#include "app/config.hpp"
#include "app/runner.hpp"

int main(int argc, char** argv) {
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
            runner.run_benchmark();
            return 0;
        }
        if (mode == "auto") {
            runner.run_auto_tuned();
            return 0;
        }

        runner.run();
        return 0;
    } catch (const std::exception& error) {
        std::cerr << error.what() << '\n';
        return 1;
    }
}
