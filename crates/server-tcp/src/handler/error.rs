use std::error::Error;

/// Why a [`TcpHandler`](crate::TcpHandler) ended a connection with a failure.
///
/// The accept loop logs it with `%error` and records the connection as
/// failed; it never inspects the cause. A handler wraps its own error with
/// [`TcpHandlerError::other`], so the logged text is that error's own
/// `Display`.
#[derive(Debug, thiserror::Error)]
pub enum TcpHandlerError {
    /// A failure raised by the handler. The message is the wrapped error's
    /// own.
    #[error(transparent)]
    Other(Box<dyn Error + Send + Sync>),
}

impl TcpHandlerError {
    /// Wrap a handler error, such as an `anyhow::Error`, a `String` or an
    /// `std::io::Error`.
    pub fn other(error: impl Into<Box<dyn Error + Send + Sync>>) -> Self {
        Self::Other(error.into())
    }
}

#[cfg(test)]
mod tests;
