use std::path::PathBuf;
use std::process::Command;

fn main() {
    println!("cargo:rustc-check-cfg=cfg(mining_cuda_native_enabled)");
    println!("cargo:rerun-if-env-changed=HOST");
    println!("cargo:rerun-if-env-changed=CUDA_PATH");
    println!("cargo:rerun-if-changed=../../native/mining-cuda/CMakeLists.txt");
    println!("cargo:rerun-if-changed=../../native/mining-cuda/include");
    println!("cargo:rerun-if-changed=../../native/mining-cuda/src");

    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
    if target_os != "windows" || target_arch != "x86_64" {
        return;
    }

    if !host_is_windows() {
        warn_native_disabled("当前不是 Windows 宿主，跳过 CUDA 原生后端构建。");
        return;
    }

    let Some(cuda_lib_dir) = find_cuda_lib_dir() else {
        warn_native_disabled("未检测到 CUDA 库目录，跳过 CUDA 原生后端构建。");
        return;
    };
    let Some(nvcc_path) = find_nvcc() else {
        warn_native_disabled("未检测到 nvcc，跳过 CUDA 原生后端构建。");
        return;
    };

    if let Some(vs_env) = load_vs_dev_env() {
        for (key, value) in vs_env {
            unsafe {
                std::env::set_var(key, value);
            }
        }
    }
    if let Some(cmake_path) = find_visual_studio_cmake() {
        unsafe {
            std::env::set_var("CMAKE", cmake_path);
        }
    }
    let ninja_path = find_visual_studio_ninja();
    if let Some(path) = ninja_path.as_ref() {
        unsafe {
            std::env::set_var("CMAKE_GENERATOR", "Ninja");
        }
        unsafe {
            std::env::set_var("CMAKE_MAKE_PROGRAM", path);
        }
    }

    let Some(dst) = build_native(ninja_path, nvcc_path) else {
        warn_native_disabled("CUDA 原生后端构建失败，已自动降级为不可用后端。");
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
    println!("cargo:rustc-link-lib=winhttp");
}

fn build_native(ninja_path: Option<PathBuf>, nvcc_path: PathBuf) -> Option<PathBuf> {
    run_with_suppressed_panic_output(|| {
        let mut config = cmake::Config::new("../../native/mining-cuda");
        config
            .profile("Release")
            .define("MINING_CUDA_LIBRARY_ONLY", "ON")
            .define("CMAKE_CUDA_COMPILER", nvcc_path)
            .build_target("mining_cuda_core");
        if let Some(path) = ninja_path {
            config.define("CMAKE_MAKE_PROGRAM", path);
        }
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
    println!("cargo:warning=CUDA native backend disabled: {reason}");
}

fn host_is_windows() -> bool {
    std::env::var("HOST")
        .map(|host| host.contains("windows"))
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
    let output = std::process::Command::new(vswhere)
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

fn find_nvcc() -> Option<PathBuf> {
    if let Some(cuda_path) = std::env::var_os("CUDA_PATH") {
        let path = PathBuf::from(&cuda_path).join("bin").join("nvcc.exe");
        if path.is_file() {
            return Some(path);
        }
    }
    let default_root = PathBuf::from(r"C:\Program Files\NVIDIA GPU Computing Toolkit\CUDA");
    if default_root.is_dir() {
        let mut entries = std::fs::read_dir(default_root)
            .ok()?
            .filter_map(Result::ok)
            .map(|entry| entry.path().join("bin").join("nvcc.exe"))
            .filter(|path| path.is_file())
            .collect::<Vec<_>>();
        entries.sort();
        entries.pop()
    } else {
        None
    }
}

fn find_cuda_lib_dir() -> Option<PathBuf> {
    if let Some(cuda_path) = std::env::var_os("CUDA_PATH") {
        let path = PathBuf::from(cuda_path).join("lib").join("x64");
        if path.is_dir() {
            return Some(path);
        }
    }
    let default_root = PathBuf::from(r"C:\Program Files\NVIDIA GPU Computing Toolkit\CUDA");
    if default_root.is_dir() {
        let mut entries = std::fs::read_dir(default_root)
            .ok()?
            .filter_map(Result::ok)
            .map(|entry| entry.path().join("lib").join("x64"))
            .filter(|path| path.is_dir())
            .collect::<Vec<_>>();
        entries.sort();
        entries.pop()
    } else {
        None
    }
}
