//! Async FileWatcher Bridge
//!
//! Bridges synchronous file watcher events into the async Tokio runtime.
//! This ensures the async event loop is not blocked by file system notifications.

pub use crate::domain::daemon::watch_event::BridgeEvent;
pub use crate::infrastructure::watch_bridge::bridge::{
    spawn_watch_bridge, AsyncWatchBridgeBuilder, WatchBridge, WatchBridgeConfig, WatchBridgeHandle,
};
