use crate::application::daemon::protocol::*;

use super::request_handler::RequestHandler;
use super::timestamp::format_unix_timestamp;

impl RequestHandler {
    /// Get index status
    pub(crate) async fn handle_index_status(&self) -> Result<serde_json::Value, RpcError> {
        let cache = self.cache.read().await;

        let total_symbols: usize = cache
            .values()
            .map(|a| a.symbol_table.all_symbols().len())
            .sum();

        // Report the most recent diagnostic update timestamp across all cached files
        let last_updated: Option<String> =
            cache
                .values()
                .map(|a| a.last_updated_secs)
                .max()
                .map(|secs| {
                    // Format as ISO 8601 date-time string (UTC, seconds precision)
                    let datetime = format_unix_timestamp(secs);
                    datetime
                });

        let status = IndexStatus {
            indexed_files: cache.len(),
            total_symbols,
            last_updated,
            is_ready: true,
        };

        serde_json::to_value(status).map_err(|e| RpcError::internal_error(e.to_string()))
    }

    /// Invalidate cache for files
    pub(crate) async fn handle_invalidate(
        &self,
        params: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, RpcError> {
        #[derive(serde::Deserialize)]
        struct InvalidateParams {
            files: Vec<String>,
        }

        let params: InvalidateParams = serde_json::from_value(
            params.ok_or_else(|| RpcError::invalid_params("Missing params"))?,
        )
        .map_err(|e| RpcError::invalid_params(format!("Invalid params: {}", e)))?;

        let mut cache = self.cache.write().await;
        let mut invalidated = 0;

        for file in params.files {
            let path = self.resolve_path(&file);
            if cache.remove(&path).is_some() {
                invalidated += 1;
            }
        }

        serde_json::to_value(serde_json::json!({ "invalidated": invalidated }))
            .map_err(|e| RpcError::internal_error(e.to_string()))
    }

    /// Shutdown the daemon
    pub(crate) async fn handle_shutdown(&self) -> Result<serde_json::Value, RpcError> {
        // The actual shutdown is handled by the daemon
        Ok(serde_json::json!({ "status": "shutting_down" }))
    }
}
