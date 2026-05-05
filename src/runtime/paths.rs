use std::fs;
use std::path::{Path, PathBuf};

const RUNTIME_DIR_NAME: &str = "var";
const DATA_DIR_NAME: &str = "data";
const LOG_DIR_NAME: &str = "log";
const DIST_DIR_NAME: &str = "dist";
const LEGACY_ARTIFACT_DIR_NAME: &str = "artifacts";

pub fn resolve_data_file_path(name: impl AsRef<Path>) -> PathBuf {
    resolve_data_file_path_with_sources(
        name,
        std::env::current_dir().ok(),
        std::env::current_exe().ok(),
    )
}

pub fn migrate_legacy_data_file(name: impl AsRef<Path>) {
    let name = name.as_ref();
    let destination = resolve_data_file_path(name);
    let Ok(working_dir) = std::env::current_dir() else {
        return;
    };
    let source = working_dir.join(name);
    if source == destination || !source.is_file() || destination.exists() {
        return;
    }
    if let Some(parent) = destination.parent()
        && fs::create_dir_all(parent).is_err()
    {
        return;
    }
    let _ = fs::rename(&source, &destination).or_else(|_| {
        fs::copy(&source, &destination)?;
        fs::remove_file(&source)
    });
}

pub fn resolve_packaged_file_path(name: impl AsRef<Path>) -> PathBuf {
    resolve_packaged_file_path_with_sources(
        name,
        std::env::current_dir().ok(),
        std::env::current_exe().ok(),
    )
}

pub fn resolve_data_file_path_with_sources(
    name: impl AsRef<Path>,
    working_dir: Option<PathBuf>,
    executable_path: Option<PathBuf>,
) -> PathBuf {
    let name = name.as_ref();
    if let Some(root) = working_dir.and_then(find_root_from_runtime_dir) {
        return resolve_workspace_data_path(&root, name);
    }
    if let Some(root) = executable_path
        .as_deref()
        .and_then(Path::parent)
        .and_then(find_root_from_runtime_dir)
    {
        return resolve_workspace_data_path(&root, name);
    }
    name.to_path_buf()
}

pub fn resolve_packaged_file_path_with_sources(
    name: impl AsRef<Path>,
    working_dir: Option<PathBuf>,
    executable_path: Option<PathBuf>,
) -> PathBuf {
    let name = name.as_ref();
    let executable_dir = executable_path.as_deref().and_then(Path::parent);
    let working_root = working_dir.clone().and_then(find_root_from_runtime_dir);
    let executable_root = executable_dir.and_then(find_root_from_runtime_dir);

    let mut candidates = Vec::new();
    if let Some(dir) = executable_dir {
        candidates.push(dir.join(name));
    }
    if let Some(dir) = working_dir.as_deref() {
        candidates.push(dir.join(name));
    }
    if let Some(root) = working_root.as_deref() {
        candidates.extend(workspace_packaged_candidates(root, name));
    }
    if let Some(root) = executable_root.as_deref() {
        candidates.extend(workspace_packaged_candidates(root, name));
    }
    if let Some(existing) = candidates.iter().find(|candidate| candidate.is_file()) {
        return existing.clone();
    }

    if let Some(dir) = executable_dir
        && (executable_root.is_none() || !is_build_output_dir(dir, executable_root.as_deref()))
    {
        return dir.join(name);
    }
    if working_root.is_none()
        && let Some(dir) = working_dir.as_deref()
    {
        return dir.join(name);
    }
    if let Some(root) = working_root {
        return root.join(DIST_DIR_NAME).join(name);
    }
    if let Some(root) = executable_root {
        return root.join(DIST_DIR_NAME).join(name);
    }
    if let Some(dir) = working_dir {
        return dir.join(name);
    }
    if let Some(dir) = executable_dir {
        return dir.join(name);
    }
    name.to_path_buf()
}

fn workspace_packaged_candidates(root: &Path, name: &Path) -> Vec<PathBuf> {
    vec![
        root.join(DIST_DIR_NAME).join(name),
        root.join(LEGACY_ARTIFACT_DIR_NAME).join(name),
    ]
}

fn resolve_workspace_data_path(root: &Path, name: &Path) -> PathBuf {
    let mut relative = PathBuf::new();
    let mut components = name.components();
    match components
        .next()
        .map(|component| component.as_os_str().to_string_lossy().into_owned())
    {
        Some(first) if first == RUNTIME_DIR_NAME => {
            relative.push(RUNTIME_DIR_NAME);
            relative.extend(components);
            root.join(relative)
        }
        Some(first) if first == DATA_DIR_NAME || first == LOG_DIR_NAME => {
            relative.push(RUNTIME_DIR_NAME);
            relative.push(first);
            relative.extend(components);
            root.join(relative)
        }
        Some(first) => {
            relative.push(RUNTIME_DIR_NAME);
            relative.push(DATA_DIR_NAME);
            relative.push(first);
            relative.extend(components);
            root.join(relative)
        }
        None => root.join(RUNTIME_DIR_NAME).join(DATA_DIR_NAME),
    }
}

fn is_build_output_dir(dir: &Path, root: Option<&Path>) -> bool {
    let Some(root) = root else {
        return false;
    };
    let Ok(relative) = dir.strip_prefix(root) else {
        return false;
    };
    relative
        .components()
        .next()
        .is_some_and(|component| component.as_os_str() == "target")
}

pub fn find_root_from_runtime_dir(start: impl AsRef<Path>) -> Option<PathBuf> {
    let start = start.as_ref();
    if let Some(root) = find_nearest_workspace_dir(start) {
        return Some(root);
    }
    if start.file_name().is_some_and(|name| {
        name == DIST_DIR_NAME || name == LEGACY_ARTIFACT_DIR_NAME || name == RUNTIME_DIR_NAME
    }) {
        return start.parent().map(Path::to_path_buf);
    }
    None
}

fn find_nearest_workspace_dir(start: &Path) -> Option<PathBuf> {
    let mut current = start.to_path_buf();
    loop {
        let cargo_manifest = current.join("Cargo.toml");
        if cargo_manifest.is_file() && manifest_declares_workspace(&cargo_manifest) {
            return Some(current);
        }
        if !current.pop() {
            return None;
        }
    }
}

fn manifest_declares_workspace(path: &Path) -> bool {
    let Ok(content) = fs::read_to_string(path) else {
        return false;
    };
    content.lines().any(|line| line.trim() == "[workspace]")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn uses_runtime_parent_as_root() {
        let temp = tempdir().unwrap();
        let root = temp.path().join("workspace").join("hdd-autopilot");
        let runtime = root.join(RUNTIME_DIR_NAME);
        fs::create_dir_all(&runtime).unwrap();
        fs::write(root.join("Cargo.toml"), "[workspace]\nmembers = []\n").unwrap();

        assert_eq!(find_root_from_runtime_dir(&runtime), Some(root));
    }

    #[test]
    fn uses_dist_parent_as_root() {
        let temp = tempdir().unwrap();
        let root = temp.path().join("workspace").join("hdd-autopilot");
        let dist = root.join(DIST_DIR_NAME);
        fs::create_dir_all(&dist).unwrap();
        fs::write(root.join("Cargo.toml"), "[workspace]\nmembers = []\n").unwrap();

        assert_eq!(find_root_from_runtime_dir(&dist), Some(root));
    }

    #[test]
    fn uses_legacy_artifacts_parent_as_root() {
        let temp = tempdir().unwrap();
        let root = temp.path().join("workspace").join("hdd-autopilot");
        let artifacts = root.join(LEGACY_ARTIFACT_DIR_NAME);
        fs::create_dir_all(&artifacts).unwrap();
        fs::write(root.join("Cargo.toml"), "[workspace]\nmembers = []\n").unwrap();

        assert_eq!(find_root_from_runtime_dir(&artifacts), Some(root));
    }

    #[test]
    fn prefers_workspace_root_over_nested_crate_manifest() {
        let temp = tempdir().unwrap();
        let root = temp.path();
        fs::write(root.join("Cargo.toml"), "[workspace]\nmembers = []\n").unwrap();
        let nested = root.join("crates").join("demo");
        fs::create_dir_all(&nested).unwrap();
        fs::write(
            nested.join("Cargo.toml"),
            "[package]\nname = \"demo\"\nversion = \"0.1.0\"\n",
        )
        .unwrap();

        assert_eq!(
            find_root_from_runtime_dir(&nested),
            Some(root.to_path_buf())
        );
    }

    #[test]
    fn ignores_non_workspace_cargo_manifest() {
        let temp = tempdir().unwrap();
        let root = temp.path();
        fs::write(
            root.join("Cargo.toml"),
            "[package]\nname = \"demo\"\nversion = \"0.1.0\"\n",
        )
        .unwrap();
        let nested = root.join("src").join("bin");
        fs::create_dir_all(&nested).unwrap();

        assert_eq!(find_root_from_runtime_dir(&nested), None);
    }

    #[test]
    fn data_files_default_under_var_data() {
        let temp = tempdir().unwrap();
        let root = temp.path().join("workspace").join("hdd-autopilot");
        fs::create_dir_all(root.join(DIST_DIR_NAME)).unwrap();
        fs::write(root.join("Cargo.toml"), "[workspace]\nmembers = []\n").unwrap();

        let got = resolve_data_file_path_with_sources(
            "auth.json",
            Some(temp.path().join("outside")),
            Some(
                root.join(DIST_DIR_NAME)
                    .join("hdd-autopilot-x86_64-pc-windows-msvc.exe"),
            ),
        );
        assert_eq!(got, root.join("var").join("data").join("auth.json"));
    }

    #[test]
    fn log_files_default_under_var_log() {
        let temp = tempdir().unwrap();
        let root = temp.path().join("workspace").join("hdd-autopilot");
        let src_dir = root.join("src");
        fs::create_dir_all(&src_dir).unwrap();
        fs::write(root.join("Cargo.toml"), "[workspace]\nmembers = []\n").unwrap();

        let got =
            resolve_data_file_path_with_sources("log/checkin/checkin.log", Some(src_dir), None);
        assert_eq!(
            got,
            root.join("var")
                .join("log")
                .join("checkin")
                .join("checkin.log")
        );
    }

    #[test]
    fn packaged_file_prefers_executable_directory() {
        let temp = tempdir().unwrap();
        let root = temp.path().join("workspace").join("hdd-autopilot");
        let portable = temp.path().join("portable");
        fs::create_dir_all(root.join(LEGACY_ARTIFACT_DIR_NAME)).unwrap();
        fs::create_dir_all(&portable).unwrap();
        fs::write(root.join("Cargo.toml"), "[workspace]\nmembers = []\n").unwrap();
        fs::write(
            root.join(LEGACY_ARTIFACT_DIR_NAME)
                .join("mining-cuda-win-x64.exe"),
            "artifact",
        )
        .unwrap();
        fs::write(portable.join("mining-cuda-win-x64.exe"), "portable").unwrap();

        let got = resolve_packaged_file_path_with_sources(
            "mining-cuda-win-x64.exe",
            Some(root),
            Some(portable.join("hdd-autopilot-x86_64-pc-windows-msvc.exe")),
        );
        assert_eq!(got, portable.join("mining-cuda-win-x64.exe"));
    }

    #[test]
    fn packaged_file_falls_back_to_working_directory() {
        let temp = tempdir().unwrap();
        let portable = temp.path().join("portable");
        fs::create_dir_all(&portable).unwrap();

        let got = resolve_packaged_file_path_with_sources(
            "mining-cuda-win-x64.exe",
            Some(portable.clone()),
            None,
        );
        assert_eq!(got, portable.join("mining-cuda-win-x64.exe"));
    }

    #[test]
    fn packaged_file_falls_back_to_workspace_dist() {
        let temp = tempdir().unwrap();
        let root = temp.path().join("workspace").join("hdd-autopilot");
        let target_release = root.join("target").join("release");
        fs::create_dir_all(root.join(DIST_DIR_NAME)).unwrap();
        fs::create_dir_all(&target_release).unwrap();
        fs::write(root.join("Cargo.toml"), "[workspace]\nmembers = []\n").unwrap();

        let got = resolve_packaged_file_path_with_sources(
            "mining-cuda-win-x64.exe",
            Some(root.clone()),
            Some(target_release.join("hdd-autopilot.exe")),
        );
        assert_eq!(
            got,
            root.join(DIST_DIR_NAME).join("mining-cuda-win-x64.exe")
        );
    }

    #[test]
    fn packaged_file_still_falls_back_to_workspace_legacy_artifacts() {
        let temp = tempdir().unwrap();
        let root = temp.path().join("workspace").join("hdd-autopilot");
        let target_release = root.join("target").join("release");
        let legacy_artifacts = root.join(LEGACY_ARTIFACT_DIR_NAME);
        fs::create_dir_all(&legacy_artifacts).unwrap();
        fs::create_dir_all(&target_release).unwrap();
        fs::write(root.join("Cargo.toml"), "[workspace]\nmembers = []\n").unwrap();
        fs::write(
            legacy_artifacts.join("mining-cuda-win-x64.exe"),
            b"legacy-binary",
        )
        .unwrap();

        let got = resolve_packaged_file_path_with_sources(
            "mining-cuda-win-x64.exe",
            Some(root.clone()),
            Some(target_release.join("hdd-autopilot.exe")),
        );
        assert_eq!(
            got,
            root.join(LEGACY_ARTIFACT_DIR_NAME)
                .join("mining-cuda-win-x64.exe")
        );
    }

    #[test]
    fn migrates_legacy_data_file_into_var_data() {
        let temp = tempdir().unwrap();
        let root = temp.path();
        fs::write(root.join("Cargo.toml"), "[workspace]\nmembers = []\n").unwrap();
        fs::write(root.join("auth.json"), "{}\n").unwrap();
        let previous = std::env::current_dir().unwrap();
        std::env::set_current_dir(root).unwrap();

        migrate_legacy_data_file("auth.json");

        std::env::set_current_dir(previous).unwrap();
        assert!(!root.join("auth.json").exists());
        assert!(root.join("var").join("data").join("auth.json").exists());
    }

    #[test]
    fn falls_back_to_relative_name_when_no_root_found() {
        let got =
            resolve_data_file_path_with_sources("log/mining/system/invite-codes.txt", None, None);
        assert_eq!(got, PathBuf::from("log/mining/system/invite-codes.txt"));
    }
}
