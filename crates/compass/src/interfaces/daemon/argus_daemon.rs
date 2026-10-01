use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use tokio::net::UnixListener;
use tokio::sync::{broadcast, mpsc, RwLock};

use crate::application::analysis::request_handler::RequestHandler;
use crate::application::daemon::background_analysis::{
    run_background_analysis, AnalysisQueueStatus,
};
use crate::application::daemon::config::DaemonConfig;
use crate::infrastructure::watch_bridge::bridge::{
    spawn_watch_bridge, BridgeEvent, WatchBridgeHandle,
};

use super::connection::handle_connection;

/// Argus Daemon server
pub struct ArgusDaemon {
    config: DaemonConfig,
    handler: Arc<RequestHandler>,
    /// Per-scope handlers for scoped toolchain binding (#1127)
    scope_handlers: Arc<HashMap<String, Arc<RequestHandler>>>,
    /// Scope roots sorted by length descending (for longest prefix match)
    scope_roots: Arc<Vec<(PathBuf, String)>>,
    shutdown_tx: broadcast::Sender<()>,
    is_running: Arc<RwLock<bool>>,
    /// Analysis queue status
    queue_status: Arc<RwLock<AnalysisQueueStatus>>,
}

impl ArgusDaemon {
    /// Create a new daemon
    pub fn new(config: DaemonConfig) -> Result<Self, String> {
        let handler = RequestHandler::new(config.root.clone())?;
        let (shutdown_tx, _) = broadcast::channel(1);

        // Auto-discover scopes (#1127)
        let discovered =
            crate::infrastructure::scope_discovery::auto_discover::discover_scopes(&config.root);
        let mut scope_handlers = HashMap::new();
        let mut scope_roots = Vec::new();

        for scope in &discovered {
            let scope_root = config.root.join(&scope.root);
            match RequestHandler::new_with_scope(
                scope_root.clone(),
                &scope.id,
                &config.root,
                &scope.search_paths,
            ) {
                Ok(h) => {
                    eprintln!(
                        "[daemon] Scope '{}' ({}) at {}",
                        scope.id,
                        scope.lang,
                        scope.root.display()
                    );
                    scope_roots.push((scope.root.clone(), scope.id.clone()));
                    scope_handlers.insert(scope.id.clone(), Arc::new(h));
                }
                Err(e) => {
                    eprintln!("[daemon] Failed to init scope '{}': {}", scope.id, e);
                }
            }
        }

        // Sort by root length descending for longest prefix match
        scope_roots.sort_by(|a, b| b.0.as_os_str().len().cmp(&a.0.as_os_str().len()));

        Ok(Self {
            config,
            handler: Arc::new(handler),
            scope_handlers: Arc::new(scope_handlers),
            scope_roots: Arc::new(scope_roots),
            shutdown_tx,
            is_running: Arc::new(RwLock::new(false)),
            queue_status: Arc::new(RwLock::new(AnalysisQueueStatus::default())),
        })
    }

    /// Run the daemon
    pub async fn run(&self) -> Result<(), String> {
        // Remove existing socket if present
        if self.config.socket_path.exists() {
            std::fs::remove_file(&self.config.socket_path)
                .map_err(|e| format!("Failed to remove existing socket: {}", e))?;
        }

        // Create parent directory if needed
        if let Some(parent) = self.config.socket_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create socket directory: {}", e))?;
        }

        // Bind to socket
        let listener = UnixListener::bind(&self.config.socket_path)
            .map_err(|e| format!("Failed to bind socket: {}", e))?;

        println!("Lens daemon listening on {:?}", self.config.socket_path);

        // Mark as running
        {
            let mut running = self.is_running.write().await;
            *running = true;
        }

        // Start file watcher if enabled
        let (watch_rx, watcher_handle) = if self.config.watch {
            let (rx, handle) = self.start_watcher().await?;
            (Some(rx), Some(handle))
        } else {
            (None, None)
        };

        // Start the background analysis task if watching
        let analysis_task = if let Some(mut watch_rx) = watch_rx {
            let handler = Arc::clone(&self.handler);
            let queue_status = Arc::clone(&self.queue_status);
            let debounce = self.config.debounce;
            let mut shutdown_rx = self.shutdown_tx.subscribe();

            Some(tokio::spawn(async move {
                run_background_analysis(
                    &mut watch_rx,
                    handler,
                    queue_status,
                    debounce,
                    &mut shutdown_rx,
                )
                .await;
            }))
        } else {
            None
        };

        // Accept connections
        let mut shutdown_rx = self.shutdown_tx.subscribe();

        loop {
            tokio::select! {
                result = listener.accept() => {
                    match result {
                        Ok((stream, _addr)) => {
                            let handler = Arc::clone(&self.handler);
                            let mut conn_shutdown_rx = self.shutdown_tx.subscribe();

                            tokio::spawn(async move {
                                if let Err(e) = handle_connection(stream, handler, &mut conn_shutdown_rx).await {
                                    eprintln!("Connection error: {}", e);
                                }
                            });
                        }
                        Err(e) => {
                            eprintln!("Accept error: {}", e);
                        }
                    }
                }
                _ = shutdown_rx.recv() => {
                    println!("Daemon shutting down...");
                    break;
                }
            }
        }

        // Flush disk cache manifest before cleanup
        self.handler.flush_cache().await;

        // Cleanup
        {
            let mut running = self.is_running.write().await;
            *running = false;
        }

        // Stop the watcher
        if let Some(mut handle) = watcher_handle {
            handle.stop();
        }

        // Wait for analysis task to finish
        if let Some(task) = analysis_task {
            let _ = task.await;
        }

        // Remove socket file
        let _ = std::fs::remove_file(&self.config.socket_path);

        Ok(())
    }

    /// Start file watcher using the async bridge
    async fn start_watcher(
        &self,
    ) -> Result<(mpsc::Receiver<BridgeEvent>, WatchBridgeHandle), String> {
        spawn_watch_bridge(self.config.root.clone(), self.config.debounce)
    }

    /// Resolve the handler for a file path (#1127).
    ///
    /// Uses longest prefix match on scope roots. Falls back to the default handler.
    pub fn handler_for_file(&self, file_path: &std::path::Path) -> Arc<RequestHandler> {
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

    /// Request shutdown
    pub fn shutdown(&self) {
        let _ = self.shutdown_tx.send(());
    }

    /// Check if daemon is running
    pub async fn is_running(&self) -> bool {
        *self.is_running.read().await
    }

    /// Get the socket path
    pub fn socket_path(&self) -> &PathBuf {
        &self.config.socket_path
    }

    /// Get the analysis queue status
    pub async fn queue_status(&self) -> AnalysisQueueStatus {
        self.queue_status.read().await.clone()
    }
}
