use std::path::{Path, PathBuf};

/// Whether a client should attach a projected ServiceAccount token, and where
/// to read it from when it does.
///
/// This is the configuration/mode half of the portable projected-token contract:
/// pure data in domain, performing no filesystem I/O.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectedTokenMode {
    /// Never attach a token. In this mode, no file is ever opened, stated,
    /// or checked.
    Disabled,
    /// Read the projected token from `path` on every request.
    Required { path: PathBuf },
}

impl ProjectedTokenMode {
    /// Disabled mode: never touch the filesystem, return no token.
    pub fn disabled() -> Self {
        Self::Disabled
    }

    /// Required mode with the given token path.
    pub fn required(path: impl Into<PathBuf>) -> Self {
        Self::Required { path: path.into() }
    }

    /// Whether this mode is `Disabled`.
    pub fn is_disabled(&self) -> bool {
        matches!(self, Self::Disabled)
    }

    /// Whether this mode is `Required`.
    pub fn is_required(&self) -> bool {
        matches!(self, Self::Required { .. })
    }

    /// The path configured for this mode, if any.
    pub fn path(&self) -> Option<&Path> {
        match self {
            Self::Disabled => None,
            Self::Required { path } => Some(path.as_path()),
        }
    }
}
