fn main() {
    println!("cargo:rustc-check-cfg=cfg(mining_opencl_native_enabled)");
    println!("cargo:rerun-if-env-changed=HOST");
    println!("cargo:rerun-if-changed=../../native/mining-opencl/CMakeLists.txt");
    println!("cargo:rerun-if-changed=../../native/mining-opencl/include");
    println!("cargo:rerun-if-changed=../../native/mining-opencl/src");
    println!(
        "cargo:rerun-if-changed=../../native/mining-opencl/third_party/argon2-gpu/include/argon2-opencl"
    );
    println!(
        "cargo:rerun-if-changed=../../native/mining-opencl/third_party/argon2-gpu/src/argon2-opencl"
    );

    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os != "macos" && target_os != "linux" {
        return;
    }

    let target_arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
    let cmake_arch = if target_os == "macos" {
        let Some(cmake_arch) = macos_cmake_arch(&target_arch) else {
            warn_native_disabled(
                "target architecture is not supported for macOS OpenCL native backend",
            );
            return;
        };
        Some(cmake_arch)
    } else {
        None
    };

    if target_os == "macos" && !host_is_macos() {
        warn_native_disabled("host is not macOS; skipping OpenCL native backend");
        return;
    }

    if target_os == "linux" && !host_is_matching_linux(&target_arch) {
        warn_native_disabled("Linux host architecture does not match OpenCL target architecture");
        return;
    }

    let Some(dst) = build_native(cmake_arch) else {
        warn_native_disabled(
            "OpenCL native backend build failed; falling back to unavailable backend",
        );
        return;
    };

    let lib_dir = dst.join("build").join("lib");
    println!("cargo:rustc-cfg=mining_opencl_native_enabled");
    println!("cargo:rustc-link-search=native={}", lib_dir.display());
    println!("cargo:rustc-link-lib=static=mining_opencl_core");
    println!("cargo:rustc-link-lib=static=argon2_ref");
    println!("cargo:rustc-link-lib=static=argon2_opencl");
    println!("cargo:rustc-link-lib=static=argon2_gpu_common");
    if target_os == "macos" {
        println!("cargo:rustc-link-lib=framework=OpenCL");
        println!("cargo:rustc-link-lib=dylib=c++");
    } else {
        println!("cargo:rustc-link-lib=dylib=OpenCL");
        println!("cargo:rustc-link-lib=dylib=stdc++");
    }
}

fn build_native(cmake_arch: Option<&str>) -> Option<std::path::PathBuf> {
    run_with_suppressed_panic_output(|| {
        let mut config = cmake::Config::new("../../native/mining-opencl");
        config.profile("Release");
        if let Some(cmake_arch) = cmake_arch {
            config.define("CMAKE_OSX_ARCHITECTURES", cmake_arch);
        }
        config.build_target("mining_opencl_core");
        config.build()
    })
}

fn run_with_suppressed_panic_output<T, F>(f: F) -> Option<T>
where
    F: FnOnce() -> T,
{
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)).ok();
    std::panic::set_hook(hook);
    result
}

fn warn_native_disabled(reason: &str) {
    println!("cargo:warning=OpenCL native backend disabled: {reason}");
}

fn host_is_macos() -> bool {
    std::env::var("HOST")
        .map(|host| host.contains("apple-darwin"))
        .unwrap_or(false)
}

fn host_is_matching_linux(target_arch: &str) -> bool {
    let Ok(host) = std::env::var("HOST") else {
        return false;
    };
    if !host.contains("linux") {
        return false;
    }
    match target_arch {
        "x86_64" => host.starts_with("x86_64-"),
        "aarch64" => host.starts_with("aarch64-"),
        _ => false,
    }
}

fn macos_cmake_arch(target_arch: &str) -> Option<&'static str> {
    match target_arch {
        "x86_64" => Some("x86_64"),
        "aarch64" => Some("arm64"),
        _ => None,
    }
}
