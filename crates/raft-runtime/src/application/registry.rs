//! Multi-group Raft registry.
//!
//! Holds the hosts of multiple independently durable consensus groups, keyed by
//! [`GroupId`]. The interfaces layer serves them behind a single `/raft/*`
//! HTTP/2 listener ([`RaftRegistry::router`]), routing each incoming RPC to its
//! designated group by `group_id`.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use super::RaftHost;
use crate::domain::GroupId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegistryError {
    AlreadyRegistered(GroupId),
}

impl std::fmt::Display for RegistryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RegistryError::AlreadyRegistered(gid) => {
                write!(f, "group {:?} is already registered", gid.as_str())
            }
        }
    }
}

impl std::error::Error for RegistryError {}

struct RegistryShared {
    groups: Mutex<HashMap<GroupId, Arc<RaftHost>>>,
}

/// Registry holding multiple [`RaftHost`]s multiplexed across a single `/raft/*` router.
#[derive(Clone, Default)]
pub struct RaftRegistry {
    shared: Arc<RegistryShared>,
}

pub type GroupRegistry = RaftRegistry;

impl Default for RegistryShared {
    fn default() -> Self {
        Self {
            groups: Mutex::new(HashMap::new()),
        }
    }
}

impl RaftRegistry {
    /// Create a new empty multi-group registry.
    pub fn new() -> Self {
        Self {
            shared: Arc::new(RegistryShared::default()),
        }
    }

    /// Register a host under its configured `group_id`.
    ///
    /// Returns `Err(RegistryError::AlreadyRegistered)` if the group ID is already registered,
    /// keeping the previously registered host serving and unmodified.
    pub fn register(&self, host: impl Into<Arc<RaftHost>>) -> Result<(), RegistryError> {
        let host = host.into();
        let gid = host.group_id().clone();
        let mut groups = self.shared.groups.lock().unwrap();
        if groups.contains_key(&gid) {
            return Err(RegistryError::AlreadyRegistered(gid));
        }
        groups.insert(gid, host);
        Ok(())
    }

    /// Look up a registered host by `group_id`.
    pub fn get(&self, group_id: &GroupId) -> Option<Arc<RaftHost>> {
        let groups = self.shared.groups.lock().unwrap();
        groups.get(group_id).cloned()
    }

    /// The host registered under the group id a peer envelope carries.
    pub(crate) fn host(&self, group_id: &str) -> Option<Arc<RaftHost>> {
        self.get(&GroupId::new(group_id))
    }

    /// The only registered host, when exactly one is registered.
    pub(crate) fn sole_host(&self) -> Option<Arc<RaftHost>> {
        let groups = self.shared.groups.lock().unwrap();
        if groups.len() == 1 {
            groups.values().next().cloned()
        } else {
            None
        }
    }

    /// Every registered host.
    pub(crate) fn hosts(&self) -> Vec<Arc<RaftHost>> {
        let groups = self.shared.groups.lock().unwrap();
        groups.values().cloned().collect()
    }
}
