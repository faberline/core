//! Deterministic desired-state hashing for Fleet child objects.

use sha2::{Digest, Sha256};

/// Recursively sort object keys in a JSON value at every depth.
///
/// Because the workspace builds `serde_json` with `preserve_order`, maps preserve
/// insertion order. Sorting keys ensures canonical formatting across differing key orders.
pub fn canonicalize_json(value: &serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(map) => {
            let mut entries: Vec<(&String, &serde_json::Value)> = map.iter().collect();
            entries.sort_by_key(|(k1, _)| *k1);
            let mut sorted_map = serde_json::Map::new();
            for (k, v) in entries {
                sorted_map.insert(k.clone(), canonicalize_json(v));
            }
            serde_json::Value::Object(sorted_map)
        }
        serde_json::Value::Array(arr) => {
            let sorted_arr = arr.iter().map(canonicalize_json).collect();
            serde_json::Value::Array(sorted_arr)
        }
        other => other.clone(),
    }
}

/// Compute the SHA-256 hex digest of the canonical JSON representation of a desired object.
///
/// Keys at every depth are sorted before hashing so the result is key-order independent.
pub fn desired_hash(value: &serde_json::Value) -> String {
    let canonical = canonicalize_json(value);
    let bytes = serde_json::to_vec(&canonical)
        .expect("canonical JSON serialization of Value should never fail");
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    let digest = hasher.finalize();
    let mut hex = String::with_capacity(64);
    for byte in digest {
        use std::fmt::Write;
        write!(&mut hex, "{:02x}", byte).expect("writing to string should never fail");
    }
    hex
}
