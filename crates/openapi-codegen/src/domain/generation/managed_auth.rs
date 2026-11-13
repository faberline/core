use std::path::{Path, PathBuf};

use super::{FileBearerScheme, ManagedAuthError};

/// The service-account token path that a Managed profile always reads.
pub const MANAGED_TOKEN_PATH: &str = "/var/run/secrets/kubernetes.io/serviceaccount/token";

/// A validated Managed profile: a CA bundle path and the fixed token path.
/// HTTPS is the only scheme it accepts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManagedAuth {
    ca_path: PathBuf,
}

impl ManagedAuth {
    pub fn new(
        ca_path: impl Into<PathBuf>,
        schemes: impl IntoIterator<Item = FileBearerScheme>,
    ) -> Result<Self, ManagedAuthError> {
        let ca_path = ca_path.into();
        if ca_path.as_os_str().is_empty() {
            return Err(ManagedAuthError::EmptyCaPath);
        }
        if ca_path.to_str().is_none() {
            return Err(ManagedAuthError::NonUtf8CaPath);
        }

        let schemes: Vec<FileBearerScheme> = schemes.into_iter().collect();
        if schemes.is_empty() {
            return Err(ManagedAuthError::NoSchemes);
        }
        if schemes
            .iter()
            .any(|scheme| *scheme != FileBearerScheme::Https)
        {
            return Err(ManagedAuthError::NonHttpsScheme);
        }

        Ok(Self { ca_path })
    }

    // The emitters read this in #271-#273; no non-test caller reads it yet.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn ca_path(&self) -> &Path {
        &self.ca_path
    }

    // The emitters read this in #271-#273; no non-test caller reads it yet.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) const fn token_path(&self) -> &'static str {
        MANAGED_TOKEN_PATH
    }
}
