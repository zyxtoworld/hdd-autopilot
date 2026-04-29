cmake_minimum_required(VERSION 3.24)

project(mining-balance-cuda LANGUAGES C CXX CUDA)

find_package(CUDAToolkit REQUIRED)

set(CMAKE_C_STANDARD 11)
set(CMAKE_C_STANDARD_REQUIRED ON)
set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_CUDA_STANDARD 20)
set(CMAKE_CUDA_STANDARD_REQUIRED ON)
set(CMAKE_POSITION_INDEPENDENT_CODE ON)
set(CMAKE_CUDA_RUNTIME_LIBRARY Static)

if(NOT CMAKE_CUDA_ARCHITECTURES)
    set(CMAKE_CUDA_ARCHITECTURES 89 CACHE STRING "CUDA architectures to build for")
endif()

set(DIST_DIR "${CMAKE_SOURCE_DIR}/../../artifacts")
set(CMAKE_RUNTIME_OUTPUT_DIRECTORY "${DIST_DIR}")
set(CMAKE_LIBRARY_OUTPUT_DIRECTORY "${DIST_DIR}")
set(CMAKE_ARCHIVE_OUTPUT_DIRECTORY "${CMAKE_BINARY_DIR}/lib")
foreach(CONFIG Debug Release RelWithDebInfo MinSizeRel)
    string(TOUPPER "${CONFIG}" CONFIG_UPPER)
    set(CMAKE_RUNTIME_OUTPUT_DIRECTORY_${CONFIG_UPPER} "${DIST_DIR}")
    set(CMAKE_LIBRARY_OUTPUT_DIRECTORY_${CONFIG_UPPER} "${DIST_DIR}")
    set(CMAKE_ARCHIVE_OUTPUT_DIRECTORY_${CONFIG_UPPER} "${CMAKE_BINARY_DIR}/lib")
endforeach()

if(MSVC)
    add_compile_options(
        "$<$<COMPILE_LANGUAGE:C>:/utf-8>"
        "$<$<COMPILE_LANGUAGE:CXX>:/utf-8>"
        "$<$<COMPILE_LANGUAGE:CUDA>:-Xcompiler=/utf-8>"
    )
endif()

add_compile_definitions(NOMINMAX WIN32_LEAN_AND_MEAN HAVE_CUDA=1)

add_library(argon2_ref STATIC
    third_party/argon2/src/argon2.c
    third_party/argon2/src/core.c
    third_party/argon2/src/encoding.c
    third_party/argon2/src/ref.c
    third_party/argon2/src/thread.c
    third_party/argon2/src/blake2/blake2b.c
)
target_include_directories(argon2_ref
    PUBLIC third_party/argon2/include
    PRIVATE third_party/argon2/src third_party/argon2/src/blake2
)

add_library(argon2_gpu_common STATIC
    third_party/argon2-gpu/src/argon2-gpu-common/argon2params.cpp
    third_party/argon2-gpu/src/argon2-gpu-common/blake2b.cpp
)
target_include_directories(argon2_gpu_common
    PUBLIC third_party/argon2-gpu/include
    PRIVATE
        third_party/argon2-gpu/include/argon2-gpu-common
        third_party/argon2-gpu/src/argon2-gpu-common
)

add_library(argon2_cuda STATIC
    third_party/argon2-gpu/src/argon2-cuda/cudaexception.cpp
    third_party/argon2-gpu/src/argon2-cuda/device.cpp
    third_party/argon2-gpu/src/argon2-cuda/globalcontext.cpp
    third_party/argon2-gpu/src/argon2-cuda/programcontext.cpp
    third_party/argon2-gpu/src/argon2-cuda/processingunit.cpp
    third_party/argon2-gpu/src/argon2-cuda/kernelrunner.cu
)
target_include_directories(argon2_cuda
    PUBLIC third_party/argon2-gpu/include
    PRIVATE
        third_party/argon2-gpu/include/argon2-cuda
        third_party/argon2-gpu/include/argon2-gpu-common
        third_party/argon2-gpu/src/argon2-cuda
        ${CUDAToolkit_INCLUDE_DIRS}
)
target_link_libraries(argon2_cuda PUBLIC argon2_gpu_common CUDA::cudart_static)
set_target_properties(argon2_cuda PROPERTIES CUDA_SEPARABLE_COMPILATION ON CUDA_ARCHITECTURES "${CMAKE_CUDA_ARCHITECTURES}")

add_executable(mining-balance-cuda
    src/main.cpp
    src/config.cpp
    src/path.cpp
    src/job.cpp
    src/runner.cpp
    src/solver.cu
    ../mining-cuda-common/src/code_store.cpp
    ../mining-cuda-common/src/pool_types.cpp
    ../mining-cuda-common/src/pool_client_winhttp.cpp
    ../mining-cuda-common/src/runner.cpp
)
target_include_directories(mining-balance-cuda PRIVATE include ../mining-cuda-common/include third_party/json third_party/argon2/include third_party/argon2-gpu/include ${CUDAToolkit_INCLUDE_DIRS})
target_link_libraries(mining-balance-cuda PRIVATE argon2_ref argon2_cuda winhttp)
set_target_properties(mining-balance-cuda PROPERTIES
    OUTPUT_NAME "mining-balance-cuda-win-x64"
    CUDA_SEPARABLE_COMPILATION ON
    CUDA_ARCHITECTURES "${CMAKE_CUDA_ARCHITECTURES}"
)
