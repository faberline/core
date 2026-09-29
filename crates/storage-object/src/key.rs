use crate::{ObjectStoreError, Result};

pub(crate) fn validate_key(key: &str) -> Result<&str> {
    let key = key.trim_matches('/');
    if key.is_empty()
        || key.contains('\0')
        || key.contains('\\')
        || key
            .split('/')
            .any(|component| component.is_empty() || matches!(component, "." | ".."))
    {
        return Err(ObjectStoreError::InvalidKey {
            key: key.to_string(),
        });
    }
    Ok(key)
}
