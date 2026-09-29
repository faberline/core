mod detect;

use std::path::PathBuf;

use crate::domain::package::manager::Dependency;

// ============================================================================
// Detector
// ============================================================================

/// Package manager detector
pub struct PackageManagerDetector {
    /// Project root directory
    root: PathBuf,
}

impl PackageManagerDetector {
    /// Parse dependencies from pyproject.toml
    ///
    /// Supports both [project.dependencies] and [tool.poetry.dependencies]
    fn parse_pyproject_dependencies(&self, content: &str) -> Vec<Dependency> {
        let mut dependencies = Vec::new();

        // Parse [project] dependencies array
        // Format: dependencies = ["django>=4.0", "fastapi[all]"]
        if let Some(deps_section) = Self::extract_array_section(content, "dependencies") {
            for line in deps_section.lines() {
                if let Some(dep) = Self::parse_dependency_line(line) {
                    dependencies.push(dep);
                }
            }
        }

        // Also check [tool.poetry.dependencies] for Poetry
        if content.contains("[tool.poetry.dependencies]") {
            if let Some(poetry_deps) =
                Self::extract_toml_section(content, "[tool.poetry.dependencies]")
            {
                for line in poetry_deps.lines() {
                    if let Some(dep) = Self::parse_poetry_dependency(line) {
                        dependencies.push(dep);
                    }
                }
            }
        }

        // Check [tool.poetry.dev-dependencies] for dev deps
        if content.contains("[tool.poetry.dev-dependencies]") {
            if let Some(dev_deps) =
                Self::extract_toml_section(content, "[tool.poetry.dev-dependencies]")
            {
                for line in dev_deps.lines() {
                    if let Some(dep) = Self::parse_poetry_dependency(line) {
                        dependencies.push(dep.as_dev());
                    }
                }
            }
        }

        dependencies
    }

    /// Parse dependencies from Pipfile
    ///
    /// Format:
    /// [packages]
    /// django = ">=4.0"
    /// fastapi = {extras = ["all"], version = "^0.100"}
    fn parse_pipfile_dependencies(&self, content: &str) -> Vec<Dependency> {
        let mut dependencies = Vec::new();

        // Parse [packages] section
        if let Some(packages) = Self::extract_toml_section(content, "[packages]") {
            for line in packages.lines() {
                if let Some(dep) = Self::parse_pipfile_dependency(line) {
                    dependencies.push(dep);
                }
            }
        }

        // Parse [dev-packages] section
        if let Some(dev_packages) = Self::extract_toml_section(content, "[dev-packages]") {
            for line in dev_packages.lines() {
                if let Some(dep) = Self::parse_pipfile_dependency(line) {
                    dependencies.push(dep.as_dev());
                }
            }
        }

        dependencies
    }

    /// Parse requirements.txt format
    ///
    /// Supports:
    /// - Simple: django>=4.0
    /// - Extras: fastapi[all]>=0.100
    /// - Comments: # this is a comment
    /// - Editable: -e git+https://...#egg=package
    fn parse_requirements_txt(&self, content: &str) -> Vec<Dependency> {
        let mut dependencies = Vec::new();

        for line in content.lines() {
            let line = line.trim();

            // Skip comments and empty lines
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            // Skip -r, -c flags (include/constraints files)
            if line.starts_with("-r") || line.starts_with("-c") {
                continue;
            }

            // Handle editable installs: -e git+https://...#egg=package
            if line.starts_with("-e") {
                if let Some(egg_idx) = line.find("#egg=") {
                    let name = line[egg_idx + 5..].trim().to_string();
                    dependencies.push(Dependency::new(name));
                }
                continue;
            }

            // Parse normal dependency line
            if let Some(dep) = Self::parse_dependency_line(line) {
                dependencies.push(dep);
            }
        }

        dependencies
    }

    /// Parse a single dependency line
    ///
    /// Formats:
    /// - django>=4.0
    /// - fastapi[all]>=0.100
    /// - package
    pub fn parse_dependency_line(line: &str) -> Option<Dependency> {
        let line = line.trim().trim_matches(|c| c == '"' || c == '\'');

        if line.is_empty() {
            return None;
        }

        // Extract name and version: "package[extra]>=1.0"
        let (name_with_extras, version) = if let Some(op_idx) =
            line.find(|c| c == '=' || c == '>' || c == '<' || c == '~' || c == '^')
        {
            let name_part = line[..op_idx].trim();
            let version_part = line[op_idx..].trim();
            (name_part, Some(version_part.to_string()))
        } else {
            (line, None)
        };

        // Split name and extras: "package[extra1,extra2]"
        let (name, extras) = if let Some(bracket_idx) = name_with_extras.find('[') {
            let name = name_with_extras[..bracket_idx].trim().to_string();
            let extras_str = &name_with_extras[bracket_idx + 1..];
            let extras_end = extras_str.find(']')?;
            let extras: Vec<String> = extras_str[..extras_end]
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            (name, extras)
        } else {
            (name_with_extras.to_string(), Vec::new())
        };

        if name.is_empty() {
            return None;
        }

        let mut dep = Dependency::new(name);
        if let Some(v) = version {
            dep = dep.with_version(v);
        }
        if !extras.is_empty() {
            dep = dep.with_extras(extras);
        }

        Some(dep)
    }

    /// Parse Poetry dependency format: django = "^4.0"
    fn parse_poetry_dependency(line: &str) -> Option<Dependency> {
        let line = line.trim();
        if line.is_empty() {
            return None;
        }

        let parts: Vec<&str> = line.split('=').collect();
        if parts.len() < 2 {
            return None;
        }

        let name = parts[0].trim().to_string();
        let version_part = parts[1].trim().trim_matches(|c| c == '"' || c == '\'');

        // Skip python version requirement
        if name == "python" {
            return None;
        }

        if name.is_empty() {
            return None;
        }

        // Handle inline table: {version = "^1.0", extras = ["all"]}
        if version_part.starts_with('{') {
            // Simple parsing - look for version field
            if let Some(version_start) = version_part.find("version") {
                let version_str = &version_part[version_start..];
                if let Some(quote_start) = version_str.find('"') {
                    let quote_start = version_start + quote_start + 1;
                    if let Some(quote_end) = version_part[quote_start..].find('"') {
                        let version =
                            version_part[quote_start..quote_start + quote_end].to_string();
                        return Some(Dependency::new(name).with_version(version));
                    }
                }
            }
            // No version found in inline table
            return Some(Dependency::new(name));
        }

        Some(Dependency::new(name).with_version(version_part.to_string()))
    }

    /// Parse Pipfile dependency format
    fn parse_pipfile_dependency(line: &str) -> Option<Dependency> {
        // For now, same as Poetry format
        Self::parse_poetry_dependency(line)
    }

    /// Extract array section from TOML: dependencies = [...]
    ///
    /// Handles multi-line arrays:
    /// dependencies = [
    ///     "django>=4.0",
    ///     "fastapi[all]",
    /// ]
    fn extract_array_section(content: &str, key: &str) -> Option<String> {
        let pattern = format!("{} = [", key);
        if let Some(start_idx) = content.find(&pattern) {
            let start = start_idx + pattern.len();

            // Find matching closing bracket, accounting for nested brackets
            let mut depth = 1;
            let mut end = start;
            let chars: Vec<char> = content[start..].chars().collect();

            for (i, ch) in chars.iter().enumerate() {
                match ch {
                    '[' => depth += 1,
                    ']' => {
                        depth -= 1;
                        if depth == 0 {
                            end = start + i;
                            break;
                        }
                    }
                    _ => {}
                }
            }

            if depth == 0 {
                return Some(content[start..end].to_string());
            }
        }
        None
    }

    /// Extract a TOML section by header: [section.name]
    fn extract_toml_section(content: &str, header: &str) -> Option<String> {
        if let Some(start) = content.find(header) {
            let start = start + header.len();
            // Find next section or end of file
            if let Some(end) = content[start..].find("\n[") {
                return Some(content[start..start + end].to_string());
            } else {
                return Some(content[start..].to_string());
            }
        }
        None
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests;
