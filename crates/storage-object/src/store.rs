use crate::{Object, ObjectMeta, PutCondition, Result};

/// Object I/O boundary. Implementations own conditional-write mechanics.
pub trait ObjectStore: Send + Sync + 'static {
    fn put(
        &self,
        key: &str,
        bytes: &[u8],
        content_type: &str,
        condition: PutCondition,
    ) -> Result<ObjectMeta>;

    fn get(&self, key: &str) -> Result<Object>;
    fn head(&self, key: &str) -> Result<ObjectMeta>;
    fn list(&self, prefix: &str) -> Result<Vec<ObjectMeta>>;
    fn delete(&self, key: &str) -> Result<()>;
}
