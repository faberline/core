/// Why [`ManagedAuth::new`](crate::ManagedAuth::new) rejected its arguments.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ManagedAuthError {
    /// The CA path is empty.
    #[error("managed auth CA path must not be empty")]
    EmptyCaPath,
    /// The CA path is not valid UTF-8, so a generated client could not embed it.
    #[error("managed auth CA path must be valid UTF-8")]
    NonUtf8CaPath,
    /// No URL scheme was allowed.
    #[error("managed auth needs the https scheme")]
    NoSchemes,
    /// A scheme other than HTTPS was allowed.
    #[error("managed auth accepts only the https scheme")]
    NonHttpsScheme,
}
