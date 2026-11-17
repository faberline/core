//! Shard routing shared by Faberline services: the virtual-bucket map
//! (`domain::virtual_bucket_shard_map`), where a document or a search lands
//! (`domain::shard_route`), and shard host names (`domain::shard_index`).
//! Bucket moves, headers, scatter-gather and the write fence are other issues.

mod domain;

pub use domain::bucket_move::{bucket_moves, BucketMove};
pub use domain::shard_index::{document_shard_index, shard_host, shard_index};
pub use domain::shard_route::{SearchShardTarget, ShardRoute};
pub use domain::virtual_bucket_shard_map::{VirtualBucketShardMap, DEFAULT_VIRTUAL_BUCKET_COUNT};
