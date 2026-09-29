use std::fs;
use std::path::PathBuf;

use crate::domain::package::manager::{PackageManager, PackageManagerDetection};
use crate::infrastructure::package::detector::PackageManagerDetector;

impl PackageManagerDetector {
    /// Create a new detector for a project root
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    /// Detect package manager and parse configuration
    ///
    /// Priority order: uv > Poetry > Pipenv > pip
    pub fn detect(&self) -> PackageManagerDetection {
        // 1. Try uv (highest priority)
        if let Some(detection) = self.detect_uv() {
            return detection;
        }

        // 2. Try Poetry
        if let Some(detection) = self.detect_poetry() {
            return detection;
        }

        // 3. Try Pipenv
        if let Some(detection) = self.detect_pipenv() {
            return detection;
        }

        // 4. Fallback to pip
        if let Some(detection) = self.detect_pip() {
            return detection;
        }

        // Nothing found
        PackageManagerDetection::unknown()
    }

    /// Detect uv project
    ///
    /// Looks for: pyproject.toml + uv.lock
    fn detect_uv(&self) -> Option<PackageManagerDetection> {
        let pyproject_path = self.root.join("pyproject.toml");
        let uv_lock_path = self.root.join("uv.lock");

        // Check for uv.lock (strong indicator)
        if !uv_lock_path.exists() {
            return None;
        }

        // Check for pyproject.toml
        if !pyproject_path.exists() {
            return None;
        }

        let content = fs::read_to_string(&pyproject_path).ok()?;

        // Must have [tool.uv] or [project] section
        if !content.contains("[tool.uv]") && !content.contains("[project]") {
            return None;
        }

        // Parse dependencies from pyproject.toml
        let dependencies = self.parse_pyproject_dependencies(&content);

        // Find virtual environment
        let venv_path = self.find_venv();

        Some(PackageManagerDetection {
            manager: PackageManager::Uv,
            config_file: pyproject_path,
            lock_file: Some(uv_lock_path),
            venv_path,
            dependencies,
            confidence: 0.95, // High confidence with lock file
        })
    }

    /// Detect Poetry project
    ///
    /// Looks for: pyproject.toml with [tool.poetry]
    fn detect_poetry(&self) -> Option<PackageManagerDetection> {
        let pyproject_path = self.root.join("pyproject.toml");
        let poetry_lock_path = self.root.join("poetry.lock");

        // Check for pyproject.toml
        if !pyproject_path.exists() {
            return None;
        }

        let content = fs::read_to_string(&pyproject_path).ok()?;

        // Must have [tool.poetry] section
        if !content.contains("[tool.poetry]") {
            return None;
        }

        // Parse dependencies
        let dependencies = self.parse_pyproject_dependencies(&content);

        // Check for lock file
        let has_lock_file = poetry_lock_path.exists();
        let lock_file = if has_lock_file {
            Some(poetry_lock_path)
        } else {
            None
        };

        let venv_path = self.find_venv();

        Some(PackageManagerDetection {
            manager: PackageManager::Poetry,
            config_file: pyproject_path,
            lock_file,
            venv_path,
            dependencies,
            confidence: if has_lock_file { 0.95 } else { 0.85 },
        })
    }

    /// Detect Pipenv project
    ///
    /// Looks for: Pipfile
    fn detect_pipenv(&self) -> Option<PackageManagerDetection> {
        let pipfile_path = self.root.join("Pipfile");
        let pipfile_lock_path = self.root.join("Pipfile.lock");

        if !pipfile_path.exists() {
            return None;
        }

        let content = fs::read_to_string(&pipfile_path).ok()?;

        // Parse Pipfile format
        let dependencies = self.parse_pipfile_dependencies(&content);

        let has_lock_file = pipfile_lock_path.exists();
        let lock_file = if has_lock_file {
            Some(pipfile_lock_path)
        } else {
            None
        };

        let venv_path = self.find_venv();

        Some(PackageManagerDetection {
            manager: PackageManager::Pipenv,
            config_file: pipfile_path,
            lock_file,
            venv_path,
            dependencies,
            confidence: if has_lock_file { 0.90 } else { 0.80 },
        })
    }

    /// Detect pip (requirements.txt)
    ///
    /// Looks for: requirements.txt or requirements/*.txt
    fn detect_pip(&self) -> Option<PackageManagerDetection> {
        // Look for requirements.txt or requirements/*.txt
        let requirements_paths = vec![
            self.root.join("requirements.txt"),
            self.root.join("requirements/base.txt"),
            self.root.join("requirements/prod.txt"),
            self.root.join("requirements/production.txt"),
        ];

        for path in &requirements_paths {
            if path.exists() {
                let content = fs::read_to_string(path).ok()?;
                let dependencies = self.parse_requirements_txt(&content);

                let venv_path = self.find_venv();

                return Some(PackageManagerDetection {
                    manager: PackageManager::Pip,
                    config_file: path.clone(),
                    lock_file: None,
                    venv_path,
                    dependencies,
                    confidence: 0.70, // Lower confidence without lock file
                });
            }
        }

        None
    }

    /// Find virtual environment directory
    ///
    /// Checks:
    /// 1. Common venv names: .venv, venv, .virtualenv, env
    /// 2. VIRTUAL_ENV environment variable
    fn find_venv(&self) -> Option<PathBuf> {
        // Check common venv names
        let venv_names = vec![".venv", "venv", ".virtualenv", "env"];

        for name in venv_names {
            let venv_path = self.root.join(name);
            if venv_path.exists() && venv_path.is_dir() {
                // Verify it's a valid venv (has bin/python or Scripts/python.exe)
                let bin_path = venv_path.join("bin/python");
                let scripts_path = venv_path.join("Scripts/python.exe");

                if bin_path.exists() || scripts_path.exists() {
                    return Some(venv_path);
                }
            }
        }

        // Check VIRTUAL_ENV environment variable
        if let Ok(venv_path) = std::env::var("VIRTUAL_ENV") {
            let path = PathBuf::from(venv_path);
            if path.exists() {
                return Some(path);
            }
        }

        None
    }
}
