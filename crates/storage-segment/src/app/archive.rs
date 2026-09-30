use std::sync::Arc;

use storage_object::ObjectStore;

use crate::application::ArchiveCoordinator;
use crate::infrastructure::ObjectStoreAdapter;

impl ArchiveCoordinator {
    pub fn new(store: Arc<dyn ObjectStore>) -> Self {
        Self::from_port(Arc::new(ObjectStoreAdapter::new(store)))
    }
}
