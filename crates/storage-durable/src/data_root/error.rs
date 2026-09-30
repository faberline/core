//! The error a data root and its policy return.

use std::{error::Error, io, path::PathBuf};

/// Why a [`DataRoot`](crate::DataRoot) could not open or replace its
/// manifest, or why a [`DataRootPolicy`](crate::DataRootPolicy) refused one.
///
/// Every variant but [`Other`](Self::Other) is raised by this crate. A policy
/// wraps its own failures with [`DataRootError::other`]. The text is what the
/// `anyhow` version of this API printed, and a failed I/O or JSON call stays
/// the [`source`](Error::source), so an `anyhow` caller sees the same `{:#}`
/// chain.
#[derive(Debug, thiserror::Error)]
pub enum DataRootError {
    /// A path inside the data root, or the root itself, is a symlink.
    #[error("data path must not be a symlink: {}", path.display())]
    Symlink { path: PathBuf },
    /// A path the root needs as a directory is something else.
    #[error("data path must be a real directory: {}", path.display())]
    NotADirectory { path: PathBuf },
    /// The manifest path is something other than a regular file.
    #[error("data path must be a regular file: {}", path.display())]
    NotARegularFile { path: PathBuf },
    /// A policy directory is absolute or leaves the root.
    #[error("data-root directory must be a safe relative path: {relative}")]
    UnsafeDirectory { relative: String },
    /// A root without a manifest holds a policy's legacy marker. This is the
    /// default [`legacy_error`](crate::DataRootPolicy::legacy_error).
    #[error("legacy {product} data at {} is not compatible", marker.display())]
    LegacyData {
        product: &'static str,
        marker: PathBuf,
    },
    /// The manifest file is not valid JSON for the policy's manifest type.
    #[error("decode layout {}", path.display())]
    ManifestDecode {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    /// The manifest could not be encoded as JSON.
    #[error("encode data-root layout")]
    ManifestEncode {
        #[source]
        source: serde_json::Error,
    },
    /// A filesystem call failed. `context` says what the root was doing.
    #[error("{context}")]
    Io {
        context: String,
        #[source]
        source: io::Error,
    },
    /// A failure raised by a policy implementation. The message is the
    /// wrapped error's own.
    #[error(transparent)]
    Other(Box<dyn Error + Send + Sync>),
}

impl DataRootError {
    /// Wrap a policy error, such as an `anyhow::Error`, a `String` or an
    /// `std::io::Error`.
    pub fn other(error: impl Into<Box<dyn Error + Send + Sync>>) -> Self {
        Self::Other(error.into())
    }

    /// An [`atomic_write`](crate::atomic_write) failure. Each is one context
    /// line over an `io::Error`, which becomes [`Io`](Self::Io).
    pub(super) fn from_atomic_write(error: anyhow::Error) -> Self {
        let context = error.to_string();
        match error.downcast::<io::Error>() {
            Ok(source) => Self::Io { context, source },
            Err(error) => Self::other(error),
        }
    }

    /// The `anyhow` shape the public path functions have always returned: an
    /// I/O failure is its context over the `io::Error`, so a direct
    /// `downcast_ref::<io::Error>()` still finds it.
    pub(super) fn into_anyhow(self) -> anyhow::Error {
        match self {
            Self::Io { context, source } => anyhow::Error::new(source).context(context),
            error => anyhow::Error::new(error),
        }
    }
}

/// `with_context` for an `io::Result`, giving [`DataRootError::Io`].
pub(super) trait IoContext<T> {
    fn io_context(self, context: impl FnOnce() -> String) -> Result<T, DataRootError>;
}

impl<T> IoContext<T> for io::Result<T> {
    fn io_context(self, context: impl FnOnce() -> String) -> Result<T, DataRootError> {
        self.map_err(|source| DataRootError::Io {
            context: context(),
            source,
        })
    }
}
