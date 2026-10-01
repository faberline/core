//! `ArgusDaemon::new`: the daemon wired to the discovered scopes, their
//! request handlers and the file-watch bridge.

use crate::application::analysis::request_handler::RequestHandler;
use crate::application::daemon::service::{DaemonService, WatchStart};
use crate::domain::daemon::config::DaemonConfig;
use crate::infrastructure::scope_discovery::auto_discover::discover_scopes;
use crate::infrastructure::watch_bridge::bridge::spawn_watch_bridge;
use crate::interfaces::daemon::argus_daemon::ArgusDaemon;

impl ArgusDaemon {
    /// Create a new daemon
    pub fn new(config: DaemonConfig) -> Result<Self, String> {
        let handler = RequestHandler::new(config.root.clone())?;

        // Auto-discover scopes (#1127)
        let discovered = discover_scopes(&config.root);
        let mut scopes = Vec::new();

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
                    scopes.push((scope.root.clone(), scope.id.clone(), h));
                }
                Err(e) => {
                    eprintln!("[daemon] Failed to init scope '{}': {}", scope.id, e);
                }
            }
        }

        let watch_start: WatchStart = Box::new(|root, debounce| {
            let (rx, mut handle) = spawn_watch_bridge(root, debounce)?;
            Ok((rx, Box::new(move || handle.stop())))
        });

        Ok(Self::from_service(DaemonService::new(
            config,
            handler,
            scopes,
            watch_start,
        )))
    }
}
