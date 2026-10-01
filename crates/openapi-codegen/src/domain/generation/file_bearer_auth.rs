use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use super::FileBearerAuthError;

/// A URL scheme that may use a generated file-backed bearer token.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum FileBearerScheme {
    /// Plain HTTP.
    Http,
    /// HTTP over TLS.
    Https,
}

impl FileBearerScheme {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Http => "http",
            Self::Https => "https",
        }
    }
}

/// A validated, generation-time opt-in bearer-token provider.
///
/// This is deliberately a generic file provider. It does not encode an
/// application or platform identity policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileBearerAuth {
    token_path: PathBuf,
    hostname_suffix: String,
    schemes: BTreeSet<FileBearerScheme>,
}

impl FileBearerAuth {
    pub fn new(
        token_path: impl Into<PathBuf>,
        hostname_suffix: impl Into<String>,
        schemes: impl IntoIterator<Item = FileBearerScheme>,
    ) -> Result<Self, FileBearerAuthError> {
        let token_path = token_path.into();
        if token_path.as_os_str().is_empty() {
            return Err(FileBearerAuthError::EmptyTokenPath);
        }
        if token_path.to_str().is_none() {
            return Err(FileBearerAuthError::NonUtf8TokenPath);
        }

        let hostname_suffix = hostname_suffix.into();
        let dns = hostname_suffix
            .strip_prefix('.')
            .filter(|suffix| !suffix.is_empty() && !suffix.ends_with('.'))
            .ok_or(FileBearerAuthError::SuffixNotDotted)?;
        if !dns.split('.').all(valid_dns_label) {
            return Err(FileBearerAuthError::SuffixNotDnsLabels);
        }

        let schemes: BTreeSet<_> = schemes.into_iter().collect();
        if schemes.is_empty() {
            return Err(FileBearerAuthError::NoSchemes);
        }

        Ok(Self {
            token_path,
            hostname_suffix,
            schemes,
        })
    }

    pub(crate) fn token_path(&self) -> &Path {
        &self.token_path
    }

    pub(crate) fn hostname_suffix(&self) -> &str {
        &self.hostname_suffix
    }

    pub(crate) fn schemes(&self) -> impl Iterator<Item = FileBearerScheme> + '_ {
        self.schemes.iter().copied()
    }
}

fn valid_dns_label(label: &str) -> bool {
    let bytes = label.as_bytes();
    !bytes.is_empty()
        && bytes.len() <= 63
        && bytes.first().is_some_and(u8::is_ascii_alphanumeric)
        && bytes.last().is_some_and(u8::is_ascii_alphanumeric)
        && bytes
            .iter()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'-')
}
