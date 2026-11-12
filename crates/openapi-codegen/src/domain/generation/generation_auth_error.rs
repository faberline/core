/// Why [`GenerationAuth::new`](crate::GenerationAuth::new) rejected its
/// arguments.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum GenerationAuthError {
    /// A file-backed bearer token and a Managed profile were both given.
    #[error("file bearer auth and managed auth cannot be set together")]
    Conflict,
}
