use super::SUPPORTED_SCHEMES;

/// Why [`BackupDestination::from_uri`](crate::BackupDestination::from_uri)
/// rejected a URI.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum DestinationError {
    /// The URI is empty or only whitespace.
    #[error("backup destination URI is empty")]
    Empty,
    /// A `file://` URI with nothing after the scheme.
    #[error("file backup URI has no path")]
    MissingPath,
    /// An `s3://` or `gs://` URI with no bucket.
    #[error("{scheme} backup URI has no bucket")]
    MissingBucket {
        /// The scheme without `://`: `s3` or `gs`.
        scheme: &'static str,
    },
    /// A URI whose scheme is not in [`SUPPORTED_SCHEMES`]. The message names
    /// every scheme in the table, in parse order.
    #[error("unsupported backup destination URI `{uri}`; use {}", scheme_list())]
    UnsupportedScheme {
        /// The rejected URI, trimmed.
        uri: String,
    },
}

fn scheme_list() -> String {
    SUPPORTED_SCHEMES
        .iter()
        .map(|s| s.scheme)
        .collect::<Vec<_>>()
        .join(", ")
}
