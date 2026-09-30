//! The daemon service: request handlers per scope, the file watch and the
//! background analysis loop it feeds.
//!
//! `ArgusDaemon` (interfaces) serves sockets on top of it; the composition
//! root builds it with the handlers, the discovered scopes and a watch
//! starter over the infrastructure watch bridge.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{broadcast, mpsc, RwLock};
use tokio::task::JoinHandle;

use crate::application::analysis::request_handler::RequestHandler;
use crate::application::daemon::background_analysis::{
    run_background_analysis, AnalysisQueueStatus,
};
use crate::domain::daemon::config::DaemonConfig;
use crate::domain::daemon::watch_event::BridgeEvent;

/// Stops a running file watch.
pub type WatchStop = Box<dyn FnOnce() + Send>;

/// Starts a file watch on a root with a debounce: the event feed and the
/// function that stops it.
pub type WatchStart = Box<
    dyn Fn(PathBuf, Duration) -> Result<(mpsc::Receiver<BridgeEvent>, WatchStop), String>
        + Send
        + Sync,
>;

/// A running background analysis: the file watch and the loop task.
pub struct BackgroundAnalysis {
    stop: WatchStop,
    task: JoinHandle<()>,
}

impl BackgroundAnalysis {
    /// Stop the watch, then wait for the loop to finish.
    pub async fn finish(self) {
        (self.stop)();
        let _ = self.task.await;
    }
}

/// The daemon's analysis side.
pub struct DaemonService {
    config: DaemonConfig,
    handler: Arc<RequestHandler>,
    /// Per-scope handlers for scoped toolchain binding (#1127)
    scope_handlers: Arc<HashMap<String, Arc<RequestHandler>>>,
    /// Scope roots sorted by length descending (for longest prefix match)
    scope_roots: Arc<Vec<(PathBuf, String)>>,
    /// Analysis queue status
    queue_status: Arc<RwLock<AnalysisQueueStatus>>,
    watch_start: WatchStart,
}

impl DaemonService {
    /// A service for `config` with the default `handler`, one handler per
    /// scope as `(scope root, scope id, handler)`, and the watch starter.
    pub fn new(
        config: DaemonConfig,
        handler: RequestHandler,
        scopes: Vec<(PathBuf, String, RequestHandler)>,
        watch_start: WatchStart,
    ) -> Self {
        let mut scope_handlers = HashMap::new();
        let mut scope_roots = Vec::new();
        for (root, id, scope_handler) in scopes {
            scope_roots.push((root, id.clone()));
            scope_handlers.insert(id, Arc::new(scope_handler));
        }

        // Sort by root length descending for longest prefix match
        scope_roots.sort_by(|a, b| b.0.as_os_str().len().cmp(&a.0.as_os_str().len()));

        Self {
            config,
            handler: Arc::new(handler),
            scope_handlers: Arc::new(scope_handlers),
            scope_roots: Arc::new(scope_roots),
            queue_status: Arc::new(RwLock::new(AnalysisQueueStatus::default())),
            watch_start,
        }
    }

    /// The socket the daemon listens on.
    pub fn socket_path(&self) -> &PathBuf {
        &self.config.socket_path
    }

    /// The default (unscoped) handler.
    pub fn handler(&self) -> Arc<RequestHandler> {
        Arc::clone(&self.handler)
    }

    /// Resolve the handler for a file path (#1127).
    ///
    /// Uses longest prefix match on scope roots. Falls back to the default handler.
    pub fn handler_for_file(&self, file_path: &Path) -> Arc<RequestHandler> {
        // Try to find a scope whose root is a prefix of the file path
        for (root, scope_id) in self.scope_roots.iter() {
            if file_path.starts_with(root) {
                if let Some(h) = self.scope_handlers.get(scope_id) {
                    return Arc::clone(h);
                }
            }
        }
        // Fallback to default handler
        Arc::clone(&self.handler)
    }

    /// List discovered scopes (#1127).
    pub fn list_scopes(&self) -> Vec<(String, PathBuf)> {
        self.scope_roots
            .iter()
            .map(|(root, id)| (id.clone(), root.clone()))
            .collect()
    }

    /// Get the analysis queue status
    pub async fn queue_status(&self) -> AnalysisQueueStatus {
        self.queue_status.read().await.clone()
    }

    /// Start the file watch and the background analysis loop when the
    /// config enables watching; the loop ends on `shutdown_rx`.
    pub fn start_background(
        &self,
        mut shutdown_rx: broadcast::Receiver<()>,
    ) -> Result<Option<BackgroundAnalysis>, String> {
        if !self.config.watch {
            return Ok(None);
        }

        let (mut watch_rx, stop) =
            (self.watch_start)(self.config.root.clone(), self.config.debounce)?;

        let handler = Arc::clone(&self.handler);
        let queue_status = Arc::clone(&self.queue_status);
        let debounce = self.config.debounce;

        let task = tokio::spawn(async move {
            run_background_analysis(
                &mut watch_rx,
                handler,
                queue_status,
                debounce,
                &mut shutdown_rx,
            )
            .await;
        });

        Ok(Some(BackgroundAnalysis { stop, task }))
    }

    /// Flush the default handler's disk cache manifest. Call on shutdown.
    pub async fn flush_cache(&self) {
        self.handler.flush_cache().await;
    }
}
