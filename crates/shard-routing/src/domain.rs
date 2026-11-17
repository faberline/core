//! Pure routing domain: no I/O, no clock, no network.

pub(crate) mod bucket_move;
pub(crate) mod shard_index;
pub(crate) mod shard_route;
pub(crate) mod virtual_bucket_shard_map;

#[cfg(test)]
mod tests;
