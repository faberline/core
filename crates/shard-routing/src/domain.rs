//! Pure routing domain: no I/O, no clock, no network.

pub(crate) mod bucket_move;
pub(crate) mod forwarding;
pub(crate) mod request_routing;
pub(crate) mod scatter;
pub(crate) mod shard_index;
pub(crate) mod shard_route;
pub(crate) mod virtual_bucket_shard_map;
pub(crate) mod write_fence;

#[cfg(test)]
mod tests;
