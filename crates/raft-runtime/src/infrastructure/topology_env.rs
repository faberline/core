//! Environment adapters for the cluster topology: the replica-mode switch and
//! the peer-address overrides.

/// Whether the StatefulSet runs in replica/HA mode: `true` when
/// `REPLICAS_PER_SHARD > 1`. A single replica — or no cluster context (the env
/// unset, e.g. local dev) — is single-node. This is the **auto-mode** switch: a
/// service defaults to single-node and turns on raft only when k8s scales it out.
pub fn replica_mode() -> bool {
    std::env::var("REPLICAS_PER_SHARD")
        .ok()
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(1)
        > 1
}

/// Parse a `LUMEN_PEERS`-style override env var (`host[:port],host[:port],...`,
/// empty entries filtered) into an `index -> host[:port]` override list.
/// Empty when `env_var` is unset — callers then use the DNS-derived
/// addresses unmodified. Shared by `ClusterTopology::from_env` and any
/// caller enumerating its own peer records with the same override contract.
pub fn parse_peer_overrides(env_var: &str) -> Vec<String> {
    std::env::var(env_var)
        .ok()
        .map(|raw| {
            raw.split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests;
