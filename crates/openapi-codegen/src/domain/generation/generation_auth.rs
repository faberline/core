use super::{FileBearerAuth, GenerationAuthError, ManagedAuth};

/// The auth selection of one generation run: at most one of the file-backed
/// bearer token and the Managed profile.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GenerationAuth {
    file_bearer: Option<FileBearerAuth>,
    managed: Option<ManagedAuth>,
}

impl GenerationAuth {
    pub fn new(
        file_bearer: Option<FileBearerAuth>,
        managed: Option<ManagedAuth>,
    ) -> Result<Self, GenerationAuthError> {
        if file_bearer.is_some() && managed.is_some() {
            return Err(GenerationAuthError::Conflict);
        }
        Ok(Self {
            file_bearer,
            managed,
        })
    }

    pub(crate) fn file_bearer(&self) -> Option<&FileBearerAuth> {
        self.file_bearer.as_ref()
    }

    // The emitters read this in #271-#273; no caller reads it yet.
    #[allow(dead_code)]
    pub(crate) fn managed(&self) -> Option<&ManagedAuth> {
        self.managed.as_ref()
    }
}
