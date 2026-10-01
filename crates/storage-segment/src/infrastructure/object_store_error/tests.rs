use storage_object::ObjectStoreError;

use crate::domain::SegmentError;

#[test]
fn an_object_store_error_keeps_its_message() {
    let error = SegmentError::from(ObjectStoreError::NotFound {
        key: "catalog/pages/a.json".to_string(),
    });
    assert!(matches!(error, SegmentError::ObjectStore(_)));
    assert_eq!(
        error.to_string(),
        "object catalog/pages/a.json was not found"
    );
    let SegmentError::ObjectStore(inner) = error else {
        unreachable!("checked above");
    };
    assert!(matches!(
        inner.downcast_ref::<ObjectStoreError>(),
        Some(ObjectStoreError::NotFound { .. })
    ));
}
