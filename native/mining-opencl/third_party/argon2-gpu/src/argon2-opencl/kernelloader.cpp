#include "kernelloader.h"

#include <iostream>
#include <sstream>

#include "app/embedded_kernel.hpp"

namespace argon2 {
namespace opencl {

cl::Program KernelLoader::loadArgon2Program(
        const cl::Context &context,
        const std::string &sourceDirectory,
        Type type, Version version, bool debug)
{
    std::string sourcePath = sourceDirectory + "/argon2_kernel.cl";
    std::string sourceText = app::kArgon2KernelSource;
    std::stringstream buildOpts;

    if (debug) {
        buildOpts << "-g -s \"embedded:argon2_kernel.cl\" ";
    }
    buildOpts << "-DARGON2_TYPE=" << type << " ";
    buildOpts << "-DARGON2_VERSION=" << version << " ";

    cl::Program prog(context, sourceText);
    try {
        std::string opts = buildOpts.str();
        prog.build(opts.c_str());
    } catch (const cl::Error &err) {
        std::cerr << "ERROR: Failed to build program:" << std::endl;
        for (cl::Device &device : context.getInfo<CL_CONTEXT_DEVICES>()) {
            std::cerr << "  Build log from device '" << device.getInfo<CL_DEVICE_NAME>() << "':" << std::endl;
            std::cerr << prog.getBuildInfo<CL_PROGRAM_BUILD_LOG>(device);
        }
        throw;
    }
    return prog;
}

} // namespace opencl
} // namespace argon2

