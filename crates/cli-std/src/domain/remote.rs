//! How a call to a remote HTTP service fails. The HTTP ports (the GitHub API,
//! courier, a node's status endpoints and release downloads) return
//! [`RemoteError`]. The adapter keeps the transport error as the source, so
//! the error chain reads the same as the `anyhow` context it replaces.

/// A transport error kept as the source of a [`RemoteError`].
pub(crate) type BoxError = Box<dyn std::error::Error + Send + Sync>;

/// A failed call to a remote HTTP service.
#[derive(Debug, thiserror::Error)]
pub(crate) enum RemoteError {
    /// The HTTP client could not be built.
    #[error("build HTTP client")]
    Client(#[source] BoxError),
    /// The request was not sent, or no response came back. `target` is the
    /// URL, or a short name of the request, such as `issue`.
    #[error("{method} {target}")]
    Send {
        method: &'static str,
        target: String,
        #[source]
        source: BoxError,
    },
    /// The service answered with an error status.
    #[error("{service} error for {url}")]
    Status {
        service: &'static str,
        url: String,
        #[source]
        source: BoxError,
    },
    /// GitHub refused a write; `message` is the one in its response.
    #[error("GitHub returned {status}: {message}")]
    Rejected { status: String, message: String },
    /// A response body is not the JSON expected; `what` names the body.
    #[error("parse {what}")]
    Parse {
        what: &'static str,
        #[source]
        source: BoxError,
    },
    /// A download failed or answered with an error status.
    #[error("download {url}")]
    Download {
        url: String,
        #[source]
        source: BoxError,
    },
    /// The body of a download could not be read.
    #[error("read download body")]
    Body(#[source] BoxError),
}
