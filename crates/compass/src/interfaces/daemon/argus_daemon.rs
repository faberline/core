use std::path::PathBuf;
use std::sync::Arc;

use tokio::net::UnixListener;
use tokio::sync::{broadcast, RwLock};

use crate::application::analysis::request_handler::RequestHandler;
use crate::application::daemon::background_analysis::AnalysisQueueStatus;
use crate::application::daemon::service::DaemonService;

use super::connection::handle_connection;

/// Argus Daemon server
///
/// `ArgusDaemon::new` lives in the composition root (src/app/daemon.rs): it
/// discovers the scopes and wires the watch bridge into the service.
pub struct ArgusDaemon {
    service: DaemonService,
    shutdown_tx: broadcast::Sender<()>,
    is_running: Arc<RwLock<bool>>,
}

impl ArgusDaemon {
    /// A daemon serving `service`.
    pub(crate) fn from_service(service: DaemonService) -> Self {
        let (shutdown_tx, _) = broadcast::channel(1);
        Self {
            service,
            shutdown_tx,
            is_running: Arc::new(RwLock::new(false)),
        }
    }

    /// Run the daemon
    pub async fn run(&self) -> Result<(), String> {
        let socket_path = self.service.socket_path();

        // Remove existing socket if present
        if socket_path.exists() {
            std::fs::remove_file(socket_path)
                .map_err(|e| format!("Failed to remove existing socket: {}", e))?;
        }

        // Create parent directory if needed
        if let Some(parent) = socket_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create socket directory: {}", e))?;
        }

        // Bind to socket
        let listener =
            UnixListener::bind(socket_path).map_err(|e| format!("Failed to bind socket: {}", e))?;

        println!("Lens daemon listening on {:?}", socket_path);

        // Mark as running
        {
            let mut running = self.is_running.write().await;
            *running = true;
        }

        // Start the file watcher and the background analysis task if enabled
        let background = self
            .service
            .start_background(self.shutdown_tx.subscribe())?;

        // Accept connections
        let mut shutdown_rx = self.shutdown_tx.subscribe();

        loop {
            tokio::select! {
                result = listener.accept() => {
                    match result {
                        Ok((stream, _addr)) => {
                            let handler = self.service.handler();
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
        self.service.flush_cache().await;

        // Cleanup
        {
            let mut running = self.is_running.write().await;
            *running = false;
        }

        // Stop the watcher and wait for the analysis task to finish
        if let Some(background) = background {
            background.finish().await;
        }

        // Remove socket file
        let _ = std::fs::remove_file(socket_path);

        Ok(())
    }

    /// Resolve the handler for a file path (#1127).
    ///
    /// Uses longest prefix match on scope roots. Falls back to the default handler.
    pub fn handler_for_file(&self, file_path: &std::path::Path) -> Arc<RequestHandler> {
        self.service.handler_for_file(file_path)
    }

    /// List discovered scopes (#1127).
    pub fn list_scopes(&self) -> Vec<(String, PathBuf)> {
        self.service.list_scopes()
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
        self.service.socket_path()
    }

    /// Get the analysis queue status
    pub async fn queue_status(&self) -> AnalysisQueueStatus {
        self.service.queue_status().await
    }
}
