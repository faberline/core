use storage_object::ObjectStoreError;

use crate::domain::SegmentError;

impl From<ObjectStoreError> for SegmentError {
    fn from(error: ObjectStoreError) -> Self {
        Self::ObjectStore(Box::new(error))
    }
}

#[cfg(test)]
mod tests;
