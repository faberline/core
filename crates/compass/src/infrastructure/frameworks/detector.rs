use std::path::PathBuf;

use crate::domain::frameworks::detection::{Framework, FrameworkDetection};
use crate::domain::package::manager::PackageManagerDetection;
use crate::infrastructure::package::detector::PackageManagerDetector;

/// Detect frameworks in a project.
pub struct FrameworkDetector {
    /// Project root
    root: PathBuf,
}

impl FrameworkDetector {
    /// Create a new detector.
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    /// Detect frameworks in the project.
    ///
    /// Uses multiple detection strategies:
    /// 1. Package manager dependencies (HIGH CONFIDENCE - from lockfiles)
    /// 2. File-based detection (project structure, config files)
    pub fn detect(&self) -> FrameworkDetection {
        let mut result = FrameworkDetection::empty();

        // NEW: Detect via package manager (highest confidence when lockfile exists)
        let pkg_detector = PackageManagerDetector::new(self.root.clone());
        let pkg_detection = pkg_detector.detect();

        // Add frameworks detected from dependencies
        self.detect_from_dependencies(&pkg_detection, &mut result);

        // Continue with file-based detection (may increase confidence)
        self.detect_django(&mut result);
        self.detect_flask(&mut result);
        self.detect_fastapi(&mut result);

        result
    }

    /// Detect frameworks from package manager dependencies
    ///
    /// This provides high-confidence detection when a lockfile is present.
    fn detect_from_dependencies(
        &self,
        pkg_detection: &PackageManagerDetection,
        result: &mut FrameworkDetection,
    ) {
        // Base confidence: higher with lockfile, lower without
        let base_confidence = if pkg_detection.lock_file.is_some() {
            0.95 // Very high confidence with lockfile
        } else {
            0.85 // Still high confidence from explicit dependencies
        };

        // Check each framework dependency
        for dep in &pkg_detection.dependencies {
            match dep.name.as_str() {
                "django" => {
                    result.add_framework(Framework::Django, base_confidence);
                }
                "fastapi" => {
                    result.add_framework(Framework::FastAPI, base_confidence);
                }
                "flask" => {
                    result.add_framework(Framework::Flask, base_confidence);
                }
                "pydantic" => {
                    // Pydantic alone doesn't mean the project uses it as a framework
                    // Only add if no other framework detected
                    if !result.has_framework(&Framework::Django)
                        && !result.has_framework(&Framework::FastAPI)
                    {
                        result.add_framework(Framework::Pydantic, base_confidence * 0.7);
                    }
                }
                "sqlalchemy" => {
                    result.add_framework(Framework::SQLAlchemy, base_confidence);
                }
                "celery" => {
                    result.add_framework(Framework::Celery, base_confidence);
                }
                _ => {}
            }
        }
    }

    /// Check for Django.
    fn detect_django(&self, result: &mut FrameworkDetection) {
        let mut confidence: f64 = 0.0;
        let mut indicators = 0;

        // 1. Check for manage.py (strong indicator)
        let manage_py = self.root.join("manage.py");
        if manage_py.exists() {
            confidence += 0.4;
            indicators += 1;
        }

        // 2. Check for settings.py or settings module
        if self.find_files_recursive("settings.py", 2) {
            confidence += 0.3;
            indicators += 1;
        }

        // 3. Check for models.py files
        if self.find_files_recursive("models.py", 2) {
            confidence += 0.15;
            indicators += 1;
        }

        // 4. Check requirements files for Django (no version check needed)
        if self.check_requirements_for("django", 0.0) {
            confidence += 0.25;
            indicators += 1;
        }

        // 5. Check pyproject.toml for Django
        if self.check_pyproject_for("django") {
            confidence += 0.2;
            indicators += 1;
        }

        if indicators > 0 {
            result.add_framework(Framework::Django, confidence.min(1.0));
        }
    }

    /// Check for Flask.
    fn detect_flask(&self, result: &mut FrameworkDetection) {
        let mut confidence: f64 = 0.0;
        let mut indicators = 0;

        // 1. Check requirements for Flask (no version check needed)
        if self.check_requirements_for("flask", 0.0) {
            confidence += 0.4;
            indicators += 1;
        }

        // 2. Check pyproject.toml
        if self.check_pyproject_for("flask") {
            confidence += 0.3;
            indicators += 1;
        }

        // 3. Check for app.py or similar Flask app files with Flask code
        let app_files = ["app.py", "application.py", "wsgi.py"];
        for app_file in &app_files {
            let path = self.root.join(app_file);
            if path.exists() {
                // Check if file contains Flask code
                if let Ok(content) = std::fs::read_to_string(&path) {
                    if content.contains("from flask")
                        || content.contains("import flask")
                        || content.contains("Flask(__name__)")
                    {
                        confidence += 0.2;
                        indicators += 1;
                        break;
                    }
                }
            }
        }

        // 4. Look for blueprints directory
        if self.root.join("blueprints").is_dir() || self.find_files_recursive("blueprints.py", 2) {
            confidence += 0.1;
            indicators += 1;
        }

        if indicators > 0 {
            result.add_framework(Framework::Flask, confidence.min(1.0));
        }
    }

    /// Check for FastAPI.
    fn detect_fastapi(&self, result: &mut FrameworkDetection) {
        let mut confidence: f64 = 0.0;
        let mut indicators = 0;

        // 1. Check requirements for FastAPI (no version check needed)
        if self.check_requirements_for("fastapi", 0.0) {
            confidence += 0.5;
            indicators += 1;
        }

        // 2. Check pyproject.toml
        if self.check_pyproject_for("fastapi") {
            confidence += 0.3;
            indicators += 1;
        }

        // 3. Check for main.py or app.py with FastAPI code
        let app_files = ["main.py", "app.py", "api.py"];
        for app_file in &app_files {
            let path = self.root.join(app_file);
            if path.exists() {
                // Check if file contains FastAPI code
                if let Ok(content) = std::fs::read_to_string(&path) {
                    if content.contains("from fastapi")
                        || content.contains("import fastapi")
                        || content.contains("FastAPI()")
                    {
                        confidence += 0.15;
                        indicators += 1;
                        break;
                    }
                }
            }
        }

        // 4. Check for routers directory
        if self.root.join("routers").is_dir() || self.root.join("api").is_dir() {
            confidence += 0.05;
            indicators += 1;
        }

        if indicators > 0 {
            result.add_framework(Framework::FastAPI, confidence.min(1.0));
        }
    }

    // Helper methods

    /// Find files recursively with given name.
    fn find_files_recursive(&self, filename: &str, max_depth: usize) -> bool {
        self.find_files_recursive_impl(&self.root, filename, max_depth, 0)
    }

    fn find_files_recursive_impl(
        &self,
        dir: &PathBuf,
        filename: &str,
        max_depth: usize,
        current_depth: usize,
    ) -> bool {
        if current_depth > max_depth {
            return false;
        }

        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();

                // Check if this is the file we're looking for
                if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
                    if file_name == filename {
                        return true;
                    }
                }

                // If it's a directory (not hidden), recurse into it
                if path.is_dir() {
                    if let Some(dir_name) = path.file_name().and_then(|n| n.to_str()) {
                        if !dir_name.starts_with('.') {
                            if self.find_files_recursive_impl(
                                &path,
                                filename,
                                max_depth,
                                current_depth + 1,
                            ) {
                                return true;
                            }
                        }
                    }
                }
            }
        }
        false
    }

    /// Check requirements files for a package.
    fn check_requirements_for(&self, package: &str, min_version: f64) -> bool {
        let req_files = [
            "requirements.txt",
            "requirements/base.txt",
            "requirements/production.txt",
            "dev-requirements.txt",
        ];

        for req_file in &req_files {
            let path = self.root.join(req_file);
            if path.exists() {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    for line in content.lines() {
                        let line = line.trim();
                        if line.to_lowercase().starts_with(package) {
                            // Found the package, check version if needed
                            if min_version > 0.0 {
                                // Simple version check (could be enhanced)
                                if let Some(version_part) = line.split(">=").nth(1) {
                                    if let Some(version_str) =
                                        version_part.split(&['<', '=', ','][..]).next()
                                    {
                                        if let Ok(version) = version_str.trim().parse::<f64>() {
                                            return version >= min_version;
                                        }
                                    }
                                }
                            }
                            return true;
                        }
                    }
                }
            }
        }
        false
    }

    /// Check pyproject.toml for a dependency.
    fn check_pyproject_for(&self, package: &str) -> bool {
        let pyproject = self.root.join("pyproject.toml");
        if pyproject.exists() {
            if let Ok(content) = std::fs::read_to_string(&pyproject) {
                // Simple check - could be enhanced with proper TOML parsing
                return content.to_lowercase().contains(&format!("\"{}\"", package))
                    || content.to_lowercase().contains(&format!("'{}'", package))
                    || content.to_lowercase().contains(&format!("{} = ", package));
            }
        }
        false
    }
}

#[cfg(test)]
mod tests;
