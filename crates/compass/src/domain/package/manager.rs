use std::path::PathBuf;

// ============================================================================
// Types
// ============================================================================

/// Package manager type
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum PackageManager {
    /// uv - Modern, fast package manager
    Uv,
    /// Poetry - Dependency resolution and packaging
    Poetry,
    /// Pipenv - Virtual environment management
    Pipenv,
    /// pip - Standard package installer
    Pip,
    /// Unknown or not detected
    Unknown,
}

impl PackageManager {
    /// Get human-readable name
    pub fn display_name(&self) -> &str {
        match self {
            Self::Uv => "uv",
            Self::Poetry => "Poetry",
            Self::Pipenv => "Pipenv",
            Self::Pip => "pip",
            Self::Unknown => "Unknown",
        }
    }

    /// Get typical config file name
    pub fn config_file_name(&self) -> &str {
        match self {
            Self::Uv => "pyproject.toml",
            Self::Poetry => "pyproject.toml",
            Self::Pipenv => "Pipfile",
            Self::Pip => "requirements.txt",
            Self::Unknown => "",
        }
    }

    /// Get lock file name
    pub fn lock_file_name(&self) -> Option<&str> {
        match self {
            Self::Uv => Some("uv.lock"),
            Self::Poetry => Some("poetry.lock"),
            Self::Pipenv => Some("Pipfile.lock"),
            Self::Pip => None,
            Self::Unknown => None,
        }
    }
}

/// Dependency information
#[derive(Debug, Clone, PartialEq)]
pub struct Dependency {
    /// Package name
    pub name: String,
    /// Version constraint (e.g., ">=4.0", "^0.100")
    pub version: Option<String>,
    /// Extras (e.g., ["all", "dev"])
    pub extras: Vec<String>,
    /// Development dependency
    pub is_dev: bool,
    /// Optional dependency
    pub is_optional: bool,
}

impl Dependency {
    /// Create a new dependency
    pub fn new(name: String) -> Self {
        Self {
            name,
            version: None,
            extras: Vec::new(),
            is_dev: false,
            is_optional: false,
        }
    }

    /// With version constraint
    pub fn with_version(mut self, version: String) -> Self {
        self.version = Some(version);
        self
    }

    /// With extras
    pub fn with_extras(mut self, extras: Vec<String>) -> Self {
        self.extras = extras;
        self
    }

    /// Mark as dev dependency
    pub fn as_dev(mut self) -> Self {
        self.is_dev = true;
        self
    }

    /// Check if this is a framework package
    pub fn is_framework(&self) -> bool {
        matches!(
            self.name.as_str(),
            "django" | "flask" | "fastapi" | "pydantic" | "sqlalchemy" | "celery"
        )
    }
}

/// Package manager detection result
#[derive(Debug, Clone)]
pub struct PackageManagerDetection {
    /// Detected package manager
    pub manager: PackageManager,
    /// Configuration file path (pyproject.toml, Pipfile, requirements.txt)
    pub config_file: PathBuf,
    /// Lock file path (uv.lock, poetry.lock, Pipfile.lock)
    pub lock_file: Option<PathBuf>,
    /// Virtual environment path (.venv, venv, etc.)
    pub venv_path: Option<PathBuf>,
    /// Parsed dependencies
    pub dependencies: Vec<Dependency>,
    /// Detection confidence (0.0 to 1.0)
    pub confidence: f64,
}

impl PackageManagerDetection {
    /// Create empty detection (Unknown manager)
    pub fn unknown() -> Self {
        Self {
            manager: PackageManager::Unknown,
            config_file: PathBuf::new(),
            lock_file: None,
            venv_path: None,
            dependencies: Vec::new(),
            confidence: 0.0,
        }
    }

    /// Get framework dependencies
    pub fn framework_dependencies(&self) -> Vec<&Dependency> {
        self.dependencies
            .iter()
            .filter(|d| d.is_framework())
            .collect()
    }

    /// Check if a specific package is present
    pub fn has_dependency(&self, name: &str) -> bool {
        self.dependencies.iter().any(|d| d.name == name)
    }

    /// Get dependency by name
    pub fn get_dependency(&self, name: &str) -> Option<&Dependency> {
        self.dependencies.iter().find(|d| d.name == name)
    }
}
