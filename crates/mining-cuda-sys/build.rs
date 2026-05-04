use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone)]
struct Target {
    os: String,
    arch: String,
}

fn main() {
    println!("cargo:rustc-check-cfg=cfg(mining_cuda_native_enabled)");
    println!("cargo:rustc-check-cfg=cfg(mining_cuda_supported_target)");
    println!("cargo:rerun-if-env-changed=HOST");
    println!("cargo:rerun-if-env-changed=CUDA_PATH");
    println!("cargo:rerun-if-env-changed=CUDA_HOME");
    println!("cargo:rerun-if-env-changed=CUDA_ROOT");
    println!("cargo:rerun-if-env-changed=CUDAToolkit_ROOT");
    println!("cargo:rerun-if-env-changed=CUDACXX");
    println!("cargo:rerun-if-env-changed=CMAKE_CUDA_ARCHITECTURES");
    println!("cargo:rerun-if-env-changed=CMAKE_MAKE_PROGRAM");
    println!("cargo:rerun-if-env-changed=CUDAARCHS");
    println!("cargo:rerun-if-env-changed=HDD_AUTOPILOT_REQUIRE_CUDA");
    println!("cargo:rerun-if-changed=../../native/mining-cuda/CMakeLists.txt");
    println!("cargo:rerun-if-changed=../../native/mining-cuda/include");
    println!("cargo:rerun-if-changed=../../native/mining-cuda/src");

    let target = Target {
        os: env::var("CARGO_CFG_TARGET_OS").unwrap_or_default(),
        arch: env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default(),
    };
    let Some(support) = cuda_support(&target) else {
        return;
    };
    println!("cargo:rustc-cfg=mining_cuda_supported_target");

    let require_cuda = support.required_by_default || env_flag("HDD_AUTOPILOT_REQUIRE_CUDA");
    if !host_matches_target(&target) {
        warn_native_disabled(&format!(
            "host cannot build CUDA for target {}-{}",
            target.arch, target.os
        ));
        return;
    }

    let Some(nvcc_path) = find_nvcc(&target) else {
        if require_cuda {
            warn_native_disabled("nvcc was not found; skipping CUDA native backend");
        }
        return;
    };
    let Some(cuda_lib_dir) = find_cuda_lib_dir(&target, &nvcc_path) else {
        if require_cuda {
            warn_native_disabled(
                "CUDA library directory was not found; skipping CUDA native backend",
            );
        }
        return;
    };

    prepare_platform_build_environment(&target);

    let Some(dst) = build_native(&target, nvcc_path) else {
        warn_native_disabled(
            "CUDA native backend build failed; falling back to unavailable backend",
        );
        return;
    };

    let lib_dir = dst.join("build").join("lib");
    println!("cargo:rustc-cfg=mining_cuda_native_enabled");
    println!("cargo:rustc-link-search=native={}", lib_dir.display());
    println!("cargo:rustc-link-search=native={}", cuda_lib_dir.display());
    println!("cargo:rustc-link-lib=static=mining_cuda_core");
    println!("cargo:rustc-link-lib=static=argon2_ref");
    println!("cargo:rustc-link-lib=static=argon2_cuda");
    println!("cargo:rustc-link-lib=static=argon2_gpu_common");
    println!("cargo:rustc-link-lib=static=cudadevrt");
    println!("cargo:rustc-link-lib=static=cudart_static");
    link_platform_libraries(&target);
}

#[derive(Debug, Clone, Copy)]
struct CudaSupport {
    required_by_default: bool,
}

fn cuda_support(target: &Target) -> Option<CudaSupport> {
    match (target.os.as_str(), target.arch.as_str()) {
        ("windows", "x86_64") => Some(CudaSupport {
            required_by_default: true,
        }),
        ("linux", "x86_64") => Some(CudaSupport {
            required_by_default: true,
        }),
        ("linux", "aarch64") => Some(CudaSupport {
            required_by_default: false,
        }),
        ("macos", "x86_64") => Some(CudaSupport {
            required_by_default: false,
        }),
        _ => None,
    }
}

fn build_native(target: &Target, nvcc_path: PathBuf) -> Option<PathBuf> {
    run_with_suppressed_panic_output(|| {
        let mut config = cmake::Config::new("../../native/mining-cuda");
        config
            .profile("Release")
            .define("MINING_CUDA_LIBRARY_ONLY", "ON")
            .define("CMAKE_CUDA_COMPILER", nvcc_path)
            .build_target("mining_cuda_core");
        configure_cmake_generator(target, &mut config);
        if target.os == "macos" {
            config.define("CMAKE_OSX_ARCHITECTURES", "x86_64");
        }
        config.build()
    })
}

fn configure_cmake_generator(target: &Target, config: &mut cmake::Config) {
    if target.os != "windows" {
        return;
    }
    let ninja = env::var_os("CMAKE_MAKE_PROGRAM")
        .map(PathBuf::from)
        .filter(|path| path.is_file())
        .or_else(find_visual_studio_ninja)
        .or_else(|| find_program_in_path("ninja.exe"))
        .or_else(|| find_program_in_path("ninja"));
    if let Some(ninja) = ninja {
        config.generator("Ninja");
        config.define("CMAKE_MAKE_PROGRAM", ninja);
    }
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
    println!("cargo:warning=CUDA native backend disabled: {reason}");
}

fn host_matches_target(target: &Target) -> bool {
    let Ok(host) = env::var("HOST") else {
        return false;
    };
    match (target.os.as_str(), target.arch.as_str()) {
        ("windows", "x86_64") => host.contains("windows") && host.starts_with("x86_64-"),
        ("linux", "x86_64") => host.contains("linux") && host.starts_with("x86_64-"),
        ("linux", "aarch64") => host.contains("linux") && host.starts_with("aarch64-"),
        ("macos", "x86_64") => host.contains("apple-darwin") && host.starts_with("x86_64-"),
        _ => false,
    }
}

fn prepare_platform_build_environment(target: &Target) {
    if target.os != "windows" {
        return;
    }
    if let Some(vs_env) = load_vs_dev_env() {
        for (key, value) in vs_env {
            set_env_var(key, value);
        }
    }
    if let Some(cmake_path) = find_visual_studio_cmake() {
        set_env_var("CMAKE", cmake_path);
    }
    if let Some(path) = find_visual_studio_ninja() {
        set_env_var("CMAKE_GENERATOR", "Ninja");
        set_env_var("CMAKE_MAKE_PROGRAM", path);
    }
}

fn set_env_var<K, V>(key: K, value: V)
where
    K: AsRef<std::ffi::OsStr>,
    V: AsRef<std::ffi::OsStr>,
{
    unsafe {
        env::set_var(key, value);
    }
}

fn link_platform_libraries(target: &Target) {
    match target.os.as_str() {
        "windows" => {
            println!("cargo:rustc-link-lib=winhttp");
        }
        "linux" => {
            println!("cargo:rustc-link-lib=dylib=stdc++");
            println!("cargo:rustc-link-lib=dylib=pthread");
            println!("cargo:rustc-link-lib=dylib=dl");
            println!("cargo:rustc-link-lib=dylib=rt");
        }
        "macos" => {
            println!("cargo:rustc-link-lib=dylib=c++");
        }
        _ => {}
    }
}

fn find_nvcc(target: &Target) -> Option<PathBuf> {
    if let Some(path) = env::var_os("CUDACXX").map(PathBuf::from)
        && path.is_file()
    {
        return Some(path);
    }
    for root in cuda_roots(target) {
        let candidate = root.join("bin").join(nvcc_name(target));
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    find_program_in_path(nvcc_name(target))
}

fn find_cuda_lib_dir(target: &Target, nvcc_path: &Path) -> Option<PathBuf> {
    let mut roots = cuda_roots(target);
    if let Some(root) = nvcc_path.parent().and_then(Path::parent) {
        roots.push(root.to_path_buf());
    }
    roots.into_iter().find_map(|root| {
        cuda_lib_candidates(target, &root)
            .into_iter()
            .find(|path| path.is_dir())
    })
}

fn nvcc_name(target: &Target) -> &'static str {
    if target.os == "windows" {
        "nvcc.exe"
    } else {
        "nvcc"
    }
}

fn cuda_roots(target: &Target) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    for var in ["CUDA_PATH", "CUDA_HOME", "CUDA_ROOT", "CUDAToolkit_ROOT"] {
        if let Some(root) = env::var_os(var) {
            roots.push(PathBuf::from(root));
        }
    }
    match target.os.as_str() {
        "windows" => {
            let default_root = PathBuf::from(r"C:\Program Files\NVIDIA GPU Computing Toolkit\CUDA");
            if default_root.is_dir()
                && let Ok(entries) = std::fs::read_dir(default_root)
            {
                roots.extend(
                    entries
                        .filter_map(Result::ok)
                        .map(|entry| entry.path())
                        .filter(|path| path.is_dir()),
                );
            }
        }
        "linux" | "macos" => {
            roots.push(PathBuf::from("/usr/local/cuda"));
            if let Ok(entries) = std::fs::read_dir("/usr/local") {
                roots.extend(
                    entries
                        .filter_map(Result::ok)
                        .map(|entry| entry.path())
                        .filter(|path| {
                            path.is_dir()
                                && path
                                    .file_name()
                                    .and_then(|name| name.to_str())
                                    .is_some_and(|name| name.starts_with("cuda-"))
                        }),
                );
            }
        }
        _ => {}
    }
    roots
}

fn cuda_lib_candidates(target: &Target, root: &Path) -> Vec<PathBuf> {
    match (target.os.as_str(), target.arch.as_str()) {
        ("windows", _) => vec![root.join("lib").join("x64")],
        ("linux", "x86_64") => vec![
            root.join("targets").join("x86_64-linux").join("lib"),
            root.join("lib64"),
            root.join("lib"),
        ],
        ("linux", "aarch64") => vec![
            root.join("targets").join("sbsa-linux").join("lib"),
            root.join("targets").join("aarch64-linux").join("lib"),
            root.join("lib64"),
            root.join("lib"),
        ],
        ("macos", _) => vec![root.join("lib")],
        _ => Vec::new(),
    }
}

fn find_program_in_path(name: &str) -> Option<PathBuf> {
    let path_var = env::var_os("PATH")?;
    env::split_paths(&path_var)
        .map(|dir| dir.join(name))
        .find(|path| path.is_file())
}

fn env_flag(name: &str) -> bool {
    env::var(name)
        .map(|value| {
            matches!(
                value.as_str(),
                "1" | "true" | "TRUE" | "yes" | "YES" | "on" | "ON"
            )
        })
        .unwrap_or(false)
}

fn load_vs_dev_env() -> Option<Vec<(String, String)>> {
    let install = find_vs_installation()?;
    let vsdevcmd = install.join("Common7").join("Tools").join("VsDevCmd.bat");
    if !vsdevcmd.is_file() {
        return None;
    }
    let command = format!(
        "call \"{}\" -arch=x64 -host_arch=x64 >nul && set",
        vsdevcmd.display()
    );
    let output = Command::new("cmd")
        .arg("/d")
        .arg("/c")
        .arg(command)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let vars = text
        .lines()
        .filter_map(|line| line.split_once('='))
        .map(|(key, value)| (key.trim().to_string(), value.trim().to_string()))
        .collect::<Vec<_>>();
    if vars.is_empty() { None } else { Some(vars) }
}

fn find_visual_studio_cmake() -> Option<PathBuf> {
    let install = find_vs_installation()?;
    let path = install
        .join("Common7")
        .join("IDE")
        .join("CommonExtensions")
        .join("Microsoft")
        .join("CMake")
        .join("CMake")
        .join("bin")
        .join("cmake.exe");
    path.is_file().then_some(path)
}

fn find_visual_studio_ninja() -> Option<PathBuf> {
    let install = find_vs_installation()?;
    let path = install
        .join("Common7")
        .join("IDE")
        .join("CommonExtensions")
        .join("Microsoft")
        .join("CMake")
        .join("Ninja")
        .join("ninja.exe");
    path.is_file().then_some(path)
}

fn find_vs_installation() -> Option<PathBuf> {
    let vswhere =
        PathBuf::from(r"C:\Program Files (x86)\Microsoft Visual Studio\Installer\vswhere.exe");
    if !vswhere.is_file() {
        return None;
    }
    let output = Command::new(vswhere)
        .args([
            "-latest",
            "-products",
            "*",
            "-requires",
            "Microsoft.VisualStudio.Component.VC.Tools.x86.x64",
            "-property",
            "installationPath",
        ])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let install = String::from_utf8(output.stdout).ok()?;
    let trimmed = install.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(PathBuf::from(trimmed))
    }
}
