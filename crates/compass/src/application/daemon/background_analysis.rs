use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::{broadcast, mpsc, RwLock};

use crate::application::analysis::request_handler::RequestHandler;
use crate::infrastructure::watch_bridge::bridge::BridgeEvent;

/// Status of the analysis queue
#[derive(Debug, Clone, Default)]
pub struct AnalysisQueueStatus {
    /// Number of files pending analysis
    pub pending_count: usize,
    /// Last time analysis was performed
    pub last_analysis: Option<Instant>,
}

/// Run background analysis loop
///
/// Consumes file watcher events, maintains a debounced queue, and
/// re-analyzes changed files in the background.
pub(crate) async fn run_background_analysis(
    watch_rx: &mut mpsc::Receiver<BridgeEvent>,
    handler: Arc<RequestHandler>,
    queue_status: Arc<RwLock<AnalysisQueueStatus>>,
    debounce: Duration,
    shutdown_rx: &mut broadcast::Receiver<()>,
) {
    // Pending files to analyze (debounce queue)
    let mut pending_files: HashSet<PathBuf> = HashSet::new();
    let mut last_change: Option<Instant> = None;

    // Analysis interval for debouncing
    let mut debounce_interval = tokio::time::interval(Duration::from_millis(50));

    loop {
        tokio::select! {
            // Receive watch events
            event = watch_rx.recv() => {
                match event {
                    Some(BridgeEvent::FilesChanged(paths)) => {
                        // Add files to pending queue
                        for path in paths {
                            pending_files.insert(path);
                        }
                        last_change = Some(Instant::now());

                        // Update queue status
                        {
                            let mut status = queue_status.write().await;
                            status.pending_count = pending_files.len();
                        }
                    }
                    Some(BridgeEvent::Error(e)) => {
                        eprintln!("Watch error: {}", e);
                    }
                    Some(BridgeEvent::Ready) => {
                        println!("File watcher ready");
                    }
                    Some(BridgeEvent::Stopped) | None => {
                        println!("File watcher stopped");
                        break;
                    }
                }
            }

            // Check debounce interval
            _ = debounce_interval.tick() => {
                // Check if we should flush the queue
                if let Some(last) = last_change {
                    if last.elapsed() >= debounce && !pending_files.is_empty() {
                        // Take the files and analyze them
                        let files: Vec<PathBuf> = pending_files.drain().collect();
                        last_change = None;

                        // Update queue status
                        {
                            let mut status = queue_status.write().await;
                            status.pending_count = 0;
                        }

                        // Analyze files in a blocking task
                        let handler_clone = Arc::clone(&handler);
                        let queue_status_clone = Arc::clone(&queue_status);

                        // Spawn blocking analysis
                        tokio::task::spawn_blocking(move || {
                            for path in &files {
                                // Invalidate cache for the file
                                let rt = tokio::runtime::Handle::current();
                                rt.block_on(async {
                                    handler_clone.invalidate_file(path).await;
                                });
                            }

                            // Re-analyze files by triggering a check
                            // The handler will re-analyze on next access
                            let rt = tokio::runtime::Handle::current();
                            rt.block_on(async {
                                for path in &files {
                                    // Pre-warm the cache by analyzing the file
                                    if let Some(path_str) = path.to_str() {
                                        let _ = handler_clone.analyze_file_async(path_str).await;
                                    }
                                }

                                // Update last analysis time
                                let mut status = queue_status_clone.write().await;
                                status.last_analysis = Some(Instant::now());
                            });
                        });
                    }
                }
            }

            // Handle shutdown
            _ = shutdown_rx.recv() => {
                break;
            }
        }
    }
}
