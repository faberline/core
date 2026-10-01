use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use crate::domain::checker_config::argus_config::ArgusConfig;
use crate::domain::python_env::environment::{DetectedEnv, EnvInfo, VenvType};

/// Detect Python environment for a project
///
/// This function implements the configuration priority:
/// 1. Explicit `[tool.cclab_lens.python]` configuration
/// 2. `PYTHONPATH` environment variable
/// 3. Auto-detected virtual environments
pub fn detect_python_environment(project_root: &Path) -> EnvInfo {
    let config = ArgusConfig::from_pyproject(project_root);
    detect_with_config(project_root, &config)
}

/// Detect Python environment with a pre-loaded configuration
pub fn detect_with_config(project_root: &Path, config: &ArgusConfig) -> EnvInfo {
    let mut info = EnvInfo {
        python_version: config.python_version.clone(),
        ..Default::default()
    };

    // Detect all virtual environments first
    info.detected_envs = detect_all_venvs(project_root);

    // Priority 1: Explicit venv_path configuration
    if let Some(ref venv_path) = config.python.venv_path {
        let venv_abs = if venv_path.is_absolute() {
            venv_path.clone()
        } else {
            project_root.join(venv_path)
        };

        if venv_abs.exists() {
            let site_packages = find_site_packages(&venv_abs, config.python_version.as_deref());
            // Try to determine the venv type by checking if it matches any detected env
            let env_type = info
                .detected_envs
                .iter()
                .find(|e| e.path == venv_abs)
                .map(|e| e.env_type.clone())
                .unwrap_or(VenvType::Venv); // Default to Venv if not in detected list

            info.active_venv = Some(DetectedEnv {
                path: venv_abs,
                env_type,
                site_packages,
            });
        }
    }

    // If no explicit config, use first detected env
    if info.active_venv.is_none() && !info.detected_envs.is_empty() {
        info.active_venv = Some(info.detected_envs[0].clone());
    }

    // Build search paths in priority order
    let mut search_paths = Vec::new();

    // 1. Explicit search_paths from config
    for path in &config.python.search_paths {
        let abs_path = if path.is_absolute() {
            path.clone()
        } else {
            project_root.join(path)
        };
        if abs_path.exists() {
            search_paths.push(abs_path);
        }
    }

    // 2. PYTHONPATH environment variable
    if let Ok(pythonpath) = env::var("PYTHONPATH") {
        for path_str in pythonpath.split(':') {
            let path = PathBuf::from(path_str);
            if path.exists() && !search_paths.contains(&path) {
                search_paths.push(path);
            }
        }
    }

    // 3. Project root
    if !search_paths.contains(&project_root.to_path_buf()) {
        search_paths.push(project_root.to_path_buf());
    }

    // 4. Site-packages from active venv (unless ignored)
    if !config.python.ignore_site_packages {
        if let Some(ref venv) = info.active_venv {
            if let Some(ref site_packages) = venv.site_packages {
                if !search_paths.contains(site_packages) {
                    search_paths.push(site_packages.clone());
                }
            }
        }
    }

    info.search_paths = search_paths;
    info
}

/// Detect all virtual environments in a project directory
pub fn detect_all_venvs(project_root: &Path) -> Vec<DetectedEnv> {
    let mut envs = Vec::new();

    // Check VIRTUAL_ENV environment variable first
    if let Ok(venv_path) = env::var("VIRTUAL_ENV") {
        let path = PathBuf::from(&venv_path);
        if path.exists() {
            let site_packages = find_site_packages(&path, None);
            envs.push(DetectedEnv {
                path,
                env_type: VenvType::Venv,
                site_packages,
            });
        }
    }

    // Check common venv directory names
    let common_venv_dirs = [".venv", "venv", "env", ".env"];
    for dir_name in common_venv_dirs {
        let venv_path = project_root.join(dir_name);
        if is_venv_directory(&venv_path) {
            // Avoid duplicates if already added via VIRTUAL_ENV
            if !envs.iter().any(|e| e.path == venv_path) {
                let site_packages = find_site_packages(&venv_path, None);
                envs.push(DetectedEnv {
                    path: venv_path,
                    env_type: VenvType::Venv,
                    site_packages,
                });
            }
        }
    }

    // Check for Poetry managed environment
    if project_root.join("poetry.lock").exists() {
        // Poetry stores envs in a different location, try to find it
        if let Some(poetry_env) = find_poetry_venv(project_root) {
            if !envs.iter().any(|e| e.path == poetry_env) {
                let site_packages = find_site_packages(&poetry_env, None);
                envs.push(DetectedEnv {
                    path: poetry_env,
                    env_type: VenvType::Poetry,
                    site_packages,
                });
            }
        }
    }

    // Check for Pipenv
    if project_root.join("Pipfile").exists() || project_root.join("Pipfile.lock").exists() {
        if let Some(pipenv_path) = find_pipenv_venv(project_root) {
            if !envs.iter().any(|e| e.path == pipenv_path) {
                let site_packages = find_site_packages(&pipenv_path, None);
                envs.push(DetectedEnv {
                    path: pipenv_path,
                    env_type: VenvType::Pipenv,
                    site_packages,
                });
            }
        }
    }

    envs
}

/// Check if a directory is a Python virtual environment
pub fn is_venv_directory(path: &Path) -> bool {
    if !path.is_dir() {
        return false;
    }

    // Check for pyvenv.cfg (modern venvs)
    if path.join("pyvenv.cfg").exists() {
        return true;
    }

    // Check for bin/python or Scripts/python.exe (cross-platform)
    let has_python_unix = path.join("bin").join("python").exists();
    let has_python_windows = path.join("Scripts").join("python.exe").exists();

    // Check for lib/pythonX.Y/site-packages structure
    let has_lib = path.join("lib").exists() || path.join("Lib").exists();

    (has_python_unix || has_python_windows) && has_lib
}

/// Find site-packages directory within a virtual environment
pub fn find_site_packages(venv_path: &Path, python_version: Option<&str>) -> Option<PathBuf> {
    // Try Unix-style paths first (lib/pythonX.Y/site-packages)
    let lib_dir = venv_path.join("lib");
    if lib_dir.exists() {
        // If we have a specific version, try that first
        if let Some(version) = python_version {
            let specific_path = lib_dir
                .join(format!("python{}", version))
                .join("site-packages");
            if specific_path.exists() {
                return Some(specific_path);
            }
        }

        // Otherwise, find any pythonX.Y directory
        if let Ok(entries) = fs::read_dir(&lib_dir) {
            for entry in entries.flatten() {
                let name = entry.file_name();
                let name_str = name.to_string_lossy();
                if name_str.starts_with("python") {
                    let site_packages = entry.path().join("site-packages");
                    if site_packages.exists() {
                        return Some(site_packages);
                    }
                }
            }
        }
    }

    // Try Windows-style path (Lib/site-packages)
    let windows_site_packages = venv_path.join("Lib").join("site-packages");
    if windows_site_packages.exists() {
        return Some(windows_site_packages);
    }

    None
}

/// Find Poetry's virtual environment for a project
fn find_poetry_venv(project_root: &Path) -> Option<PathBuf> {
    // Poetry creates venvs in {cache-dir}/virtualenvs/{project-name}-{hash}-py{version}
    // We can also check for in-project venv if poetry.toml has virtualenvs.in-project = true

    // First check for in-project venv
    let in_project_venv = project_root.join(".venv");
    if is_venv_directory(&in_project_venv) {
        return Some(in_project_venv);
    }

    // Try to find poetry cache directory
    let poetry_cache = get_poetry_cache_dir()?;
    let virtualenvs_dir = poetry_cache.join("virtualenvs");

    if !virtualenvs_dir.exists() {
        return None;
    }

    // Get project name from pyproject.toml or directory name
    let project_name = project_root
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown");

    // Find matching venv (poetry uses {name}-{hash}-py{version} pattern)
    if let Ok(entries) = fs::read_dir(&virtualenvs_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str.starts_with(project_name) && is_venv_directory(&entry.path()) {
                return Some(entry.path());
            }
        }
    }

    None
}

/// Get Poetry's cache directory
fn get_poetry_cache_dir() -> Option<PathBuf> {
    // Check POETRY_CACHE_DIR environment variable
    if let Ok(cache_dir) = env::var("POETRY_CACHE_DIR") {
        return Some(PathBuf::from(cache_dir));
    }

    // Default locations
    #[cfg(target_os = "macos")]
    {
        if let Ok(home) = env::var("HOME") {
            return Some(PathBuf::from(home).join("Library/Caches/pypoetry"));
        }
    }

    #[cfg(target_os = "linux")]
    {
        if let Ok(xdg_cache) = env::var("XDG_CACHE_HOME") {
            return Some(PathBuf::from(xdg_cache).join("pypoetry"));
        }
        if let Ok(home) = env::var("HOME") {
            return Some(PathBuf::from(home).join(".cache/pypoetry"));
        }
    }

    #[cfg(target_os = "windows")]
    {
        if let Ok(local_app_data) = env::var("LOCALAPPDATA") {
            return Some(PathBuf::from(local_app_data).join("pypoetry/Cache"));
        }
    }

    None
}

/// Find Pipenv's virtual environment for a project
fn find_pipenv_venv(project_root: &Path) -> Option<PathBuf> {
    // Pipenv creates venvs in {WORKON_HOME}/{project-name}-{hash}
    // Default WORKON_HOME is ~/.local/share/virtualenvs on Linux/macOS

    // Check WORKON_HOME first
    let workon_home = if let Ok(home) = env::var("WORKON_HOME") {
        PathBuf::from(home)
    } else if let Ok(home) = env::var("HOME") {
        PathBuf::from(home).join(".local/share/virtualenvs")
    } else {
        return None;
    };

    if !workon_home.exists() {
        return None;
    }

    // Get project name from directory
    let project_name = project_root
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown");

    // Find matching venv
    if let Ok(entries) = fs::read_dir(&workon_home) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str.starts_with(project_name) && is_venv_directory(&entry.path()) {
                return Some(entry.path());
            }
        }
    }

    None
}

/// Get the Python version from a virtual environment
pub fn get_venv_python_version(venv_path: &Path) -> Option<String> {
    // Try to read from pyvenv.cfg
    let pyvenv_cfg = venv_path.join("pyvenv.cfg");
    if pyvenv_cfg.exists() {
        if let Ok(content) = fs::read_to_string(&pyvenv_cfg) {
            for line in content.lines() {
                if let Some(version) = line.strip_prefix("version = ") {
                    // Extract major.minor from full version (e.g., "3.11.4" -> "3.11")
                    let parts: Vec<&str> = version.trim().split('.').collect();
                    if parts.len() >= 2 {
                        return Some(format!("{}.{}", parts[0], parts[1]));
                    }
                }
            }
        }
    }

    // Fallback: try to detect from lib directory name
    let lib_dir = venv_path.join("lib");
    if let Ok(entries) = fs::read_dir(&lib_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if let Some(version) = name_str.strip_prefix("python") {
                return Some(version.to_string());
            }
        }
    }

    None
}

#[cfg(test)]
mod tests;
