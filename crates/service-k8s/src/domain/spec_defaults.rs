//! Defaults of the shared service spec, as the operators copy them.
//!
//! keep's grace period (30 s) and body limit (16 MiB) are product values, not
//! these defaults.

#[cfg(test)]
mod tests;

/// Default storage size of an instance volume.
pub const DEFAULT_STORAGE_SIZE: &str = "10Gi";

/// Default termination grace period, in seconds.
pub const DEFAULT_GRACE_SECS: u64 = 10;

/// Default request body limit, in bytes (8 MiB).
pub const DEFAULT_BODY_LIMIT_BYTES: u64 = 8 * 1024 * 1024;

/// Default auth mode.
pub const DEFAULT_AUTH_MODE: &str = "off";
