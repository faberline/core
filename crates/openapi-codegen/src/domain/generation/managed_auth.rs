use std::path::{Path, PathBuf};

use super::{FileBearerScheme, ManagedAuthError};

/// The default service-account token path that a Managed profile reads.
pub const MANAGED_TOKEN_PATH: &str = "/var/run/secrets/kubernetes.io/serviceaccount/token";

/// A validated Managed profile: a CA bundle path and a token path (the
/// service-account path unless `with_token_path` sets another).
/// HTTPS is the only scheme it accepts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManagedAuth {
    ca_path: PathBuf,
    token_path: String,
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

        Ok(Self {
            ca_path,
            token_path: MANAGED_TOKEN_PATH.to_string(),
        })
    }

    /// Sets another absolute token path than the service-account default.
    pub fn with_token_path(
        mut self,
        token_path: impl Into<PathBuf>,
    ) -> Result<Self, ManagedAuthError> {
        let token_path = token_path.into();
        if token_path.as_os_str().is_empty() {
            return Err(ManagedAuthError::EmptyTokenPath);
        }
        let token_path = token_path
            .to_str()
            .ok_or(ManagedAuthError::NonUtf8TokenPath)?;
        if !Path::new(token_path).is_absolute() {
            return Err(ManagedAuthError::RelativeTokenPath);
        }
        self.token_path = token_path.to_string();
        Ok(self)
    }

    pub(crate) fn ca_path(&self) -> &Path {
        &self.ca_path
    }

    pub(crate) fn token_path(&self) -> &str {
        &self.token_path
    }
}
