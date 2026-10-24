/// Why a caller-supplied service compatibility label was rejected.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ServiceCompatibilityError {
    /// The service compatibility label was empty or only whitespace.
    #[error("service compatibility label must not be empty or whitespace")]
    EmptyOrWhitespace,
}
