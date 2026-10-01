use std::error::Error;

/// The error of the projection ports and of descriptor validation.
///
/// Product implementations of `Projection`, `ProjectionSource` and
/// `ProjectionReadSession` wrap their own errors with
/// [`ProjectionError::other`]; an `anyhow::Error` converts directly.
#[derive(Debug, thiserror::Error)]
pub enum ProjectionError {
    /// The descriptor name is blank or contains `/` or NUL.
    #[error("projection name is invalid")]
    InvalidName,
    /// A failure raised by a product implementation. The message is the
    /// wrapped error's own.
    #[error(transparent)]
    Other(Box<dyn Error + Send + Sync>),
}

impl ProjectionError {
    /// Wrap an implementation error, such as an `anyhow::Error` or an
    /// `std::io::Error`.
    pub fn other(error: impl Into<Box<dyn Error + Send + Sync>>) -> Self {
        Self::Other(error.into())
    }
}

#[cfg(test)]
mod tests;
