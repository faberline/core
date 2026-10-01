/// Why [`FileBearerAuth::new`](crate::FileBearerAuth::new) rejected its
/// arguments.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum FileBearerAuthError {
    /// The token path is empty.
    #[error("bearer token path must not be empty")]
    EmptyTokenPath,
    /// The token path is not valid UTF-8, so a generated client could not
    /// embed it.
    #[error("bearer token path must be valid UTF-8")]
    NonUtf8TokenPath,
    /// The hostname suffix does not start with exactly one dot, or is only
    /// dots, or ends with one.
    #[error("bearer hostname suffix must start with one dot")]
    SuffixNotDotted,
    /// A label of the hostname suffix is not a lowercase DNS label.
    #[error("bearer hostname suffix must be lowercase DNS labels")]
    SuffixNotDnsLabels,
    /// No URL scheme was allowed.
    #[error("bearer auth needs at least one HTTP scheme")]
    NoSchemes,
}
