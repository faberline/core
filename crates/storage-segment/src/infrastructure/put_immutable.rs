use storage_object::{ObjectStore, ObjectStoreError, PutCondition};

use crate::domain::{
    hex_sha256, ArchiveObject, ArchivedObject, ArchivedObjectVersion, Result, SegmentError,
};

pub(crate) fn put_immutable(
    store: &dyn ObjectStore,
    object: ArchiveObject,
) -> Result<ArchivedObject> {
    let sha256 = hex_sha256(&object.bytes);
    let meta = match store.put(
        &object.key,
        &object.bytes,
        &object.content_type,
        PutCondition::IfAbsent,
    ) {
        Ok(meta) => meta,
        Err(ObjectStoreError::PreconditionFailed { .. }) => {
            let existing = store.get(&object.key)?;
            if existing.bytes != object.bytes || existing.meta.content_type != object.content_type {
                return Err(SegmentError::ImmutableObjectChanged { key: object.key });
            }
            existing.meta
        }
        Err(error) => return Err(error.into()),
    };
    Ok(ArchivedObject {
        key: object.key,
        size: object.bytes.len() as u64,
        content_type: object.content_type,
        sha256,
        version: ArchivedObjectVersion::new(meta.version.as_str()),
    })
}
