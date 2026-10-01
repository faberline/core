//! The storage-object adapter for the write-once object store port: the
//! page put that accepts only a byte-identical retry, the archive put that
//! also compares the content type, and the conversion of object-store errors
//! into `SegmentError`.

mod object_store_adapter;
mod object_store_error;
mod put_immutable;

pub(crate) use object_store_adapter::ObjectStoreAdapter;
