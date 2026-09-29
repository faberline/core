//! Shared object-store boundary for local durable storage and cloud archives.

mod error;
#[cfg(feature = "gcs")]
mod gcs;
mod key;
mod local;
mod object;
#[cfg(feature = "s3")]
mod s3;
mod store;

pub use error::{ObjectStoreError, Result};
#[cfg(feature = "gcs")]
pub use gcs::GcsObjectStore;
pub(crate) use key::validate_key;
pub use local::LocalObjectStore;
pub use object::{Object, ObjectMeta, ObjectVersion, PutCondition};
#[cfg(feature = "s3")]
pub use s3::S3ObjectStore;
pub use store::ObjectStore;
