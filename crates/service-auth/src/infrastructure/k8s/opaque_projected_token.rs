use std::fmt;
use std::path::{Path, PathBuf};

use crate::domain::k8s::{ProjectedToken, ProjectedTokenMode};

/// Why an opaque projected token cannot be read from the filesystem.
///
/// Every variant names the path; no variant holds token material or any part of it.
#[derive(Debug)]
pub enum OpaqueProjectedTokenError {
    /// The token file is absent.
    Missing { path: PathBuf },
    /// The file exists but could not be read (e.g. permission denied or the path is a directory).
    Unreadable {
        path: PathBuf,
        source: std::io::Error,
    },
    /// The file exists and is empty or contains only whitespace.
    Empty { path: PathBuf },
    /// The file contains content that cannot be a bearer token
    /// (non-UTF-8, or interior whitespace or control characters).
    Malformed { path: PathBuf },
}

impl OpaqueProjectedTokenError {
    /// The path associated with this error.
    pub fn path(&self) -> &Path {
        match self {
            Self::Missing { path } => path,
            Self::Unreadable { path, .. } => path,
            Self::Empty { path } => path,
            Self::Malformed { path } => path,
        }
    }
}

impl fmt::Display for OpaqueProjectedTokenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Missing { path } => write!(
                f,
                "projected token file at {} does not exist",
                path.display()
            ),
            Self::Unreadable { path, source } => write!(
                f,
                "cannot read projected token file at {}: {source}",
                path.display()
            ),
            Self::Empty { path } => {
                write!(f, "projected token file at {} is empty", path.display())
            }
            Self::Malformed { path } => write!(
                f,
                "projected token file at {} contains malformed token content; \
                 its contents are withheld because they may still be a credential",
                path.display()
            ),
        }
    }
}

impl std::error::Error for OpaqueProjectedTokenError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Unreadable { source, .. } => Some(source),
            _ => None,
        }
    }
}

/// A portable, opaque projected ServiceAccount token source.
///
/// In `Disabled` mode, it never touches the filesystem and yields `Ok(None)`.
/// In `Required` mode, it re-reads the caller-selected file on every call,
/// trims surrounding whitespace, and returns the content as an opaque
/// `ProjectedToken` without JWT decoding.
#[derive(Debug, Clone)]
pub struct OpaqueProjectedTokenSource {
    mode: ProjectedTokenMode,
}

impl OpaqueProjectedTokenSource {
    /// Create a new token source with the given mode.
    pub fn new(mode: ProjectedTokenMode) -> Self {
        Self { mode }
    }

    /// Create a token source in `Disabled` mode.
    pub fn disabled() -> Self {
        Self::new(ProjectedTokenMode::Disabled)
    }

    /// Create a token source in `Required` mode pointing to `path`.
    pub fn required(path: impl Into<PathBuf>) -> Self {
        Self::new(ProjectedTokenMode::Required { path: path.into() })
    }

    /// Convenient alias for required mode.
    pub fn from_path(path: impl Into<PathBuf>) -> Self {
        Self::required(path)
    }

    /// The mode this source was configured with.
    pub fn mode(&self) -> &ProjectedTokenMode {
        &self.mode
    }

    /// The path configured for this source, if any.
    pub fn path(&self) -> Option<&Path> {
        self.mode.path()
    }

    /// Read the projected token as it stands right now.
    ///
    /// In `Disabled` mode, this returns `Ok(None)` without touching any path.
    /// In `Required` mode, this reads the configured file from disk without caching,
    /// trims surrounding whitespace, verifies basic bearer-value framing, and returns
    /// `Ok(Some(ProjectedToken))` without JWT decoding.
    pub fn read(&self) -> Result<Option<ProjectedToken>, OpaqueProjectedTokenError> {
        let path = match &self.mode {
            ProjectedTokenMode::Disabled => return Ok(None),
            ProjectedTokenMode::Required { path } => path,
        };

        let bytes = match std::fs::read(path) {
            Ok(b) => b,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                return Err(OpaqueProjectedTokenError::Missing { path: path.clone() });
            }
            Err(source) => {
                return Err(OpaqueProjectedTokenError::Unreadable {
                    path: path.clone(),
                    source,
                });
            }
        };

        let text = match std::str::from_utf8(&bytes) {
            Ok(t) => t,
            Err(_) => {
                return Err(OpaqueProjectedTokenError::Malformed { path: path.clone() });
            }
        };

        let trimmed = text.trim();
        if trimmed.is_empty() {
            return Err(OpaqueProjectedTokenError::Empty { path: path.clone() });
        }

        // Must not contain interior whitespace or control characters.
        if trimmed.chars().any(|c| c.is_whitespace() || c.is_control()) {
            return Err(OpaqueProjectedTokenError::Malformed { path: path.clone() });
        }

        Ok(Some(ProjectedToken::new(trimmed.to_string())))
    }
}

/// Convenience aliases matching caller naming patterns.
pub type OpaqueProjectedTokenReader = OpaqueProjectedTokenSource;
pub type ProjectedTokenReader = OpaqueProjectedTokenSource;
pub type OpaqueProjectedTokenFile = OpaqueProjectedTokenSource;

/// Read an opaque projected token according to `mode`.
pub fn read_projected_token(
    mode: &ProjectedTokenMode,
) -> Result<Option<ProjectedToken>, OpaqueProjectedTokenError> {
    OpaqueProjectedTokenSource::new(mode.clone()).read()
}

#[cfg(test)]
mod tests;
