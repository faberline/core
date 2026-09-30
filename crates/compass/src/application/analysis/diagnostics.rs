use crate::domain::daemon::protocol::*;

use super::request_handler::RequestHandler;

impl RequestHandler {
    /// Check files/directories for issues
    pub(crate) async fn handle_check(
        &self,
        params: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, RpcError> {
        let params: CheckParams = params
            .ok_or_else(|| RpcError::invalid_params("Missing params"))?
            .try_into()
            .map_err(|e| RpcError::invalid_params(format!("Invalid params: {}", e)))?;

        let path = self.resolve_path(&params.path);

        let mut all_diagnostics = Vec::new();
        let mut files_checked = 0;

        if path.is_file() {
            if let Some(diags) = self.check_file(&path).await {
                files_checked = 1;
                all_diagnostics.extend(diags);
            }
        } else if path.is_dir() {
            let files = self.collect_files(&path);
            for file in files {
                if let Some(diags) = self.check_file(&file).await {
                    files_checked += 1;
                    all_diagnostics.extend(diags);
                }
            }
        } else {
            return Err(RpcError::invalid_params(format!(
                "Path not found: {}",
                params.path
            )));
        }

        let errors = all_diagnostics
            .iter()
            .filter(|d| d.severity == "error")
            .count();
        let warnings = all_diagnostics
            .iter()
            .filter(|d| d.severity == "warning")
            .count();

        let result = CheckResult {
            diagnostics: all_diagnostics,
            files_checked,
            errors,
            warnings,
        };

        serde_json::to_value(result).map_err(|e| RpcError::internal_error(e.to_string()))
    }

    /// Get diagnostics
    pub(crate) async fn handle_diagnostics(
        &self,
        params: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, RpcError> {
        let params: DiagnosticsParams = params
            .map(|p| serde_json::from_value(p).ok())
            .flatten()
            .unwrap_or(DiagnosticsParams { file: None });

        let cache = self.cache.read().await;

        let diagnostics: Vec<DiagnosticInfo> = if let Some(file) = params.file {
            let path = self.resolve_path(&file);
            cache
                .get(&path)
                .map(|a| self.convert_diagnostics(&path, &a.diagnostics))
                .unwrap_or_default()
        } else {
            cache
                .iter()
                .flat_map(|(path, analysis)| self.convert_diagnostics(path, &analysis.diagnostics))
                .collect()
        };

        serde_json::to_value(diagnostics).map_err(|e| RpcError::internal_error(e.to_string()))
    }
}
