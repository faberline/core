use std::path::PathBuf;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;

use crate::application::daemon::config::DaemonConfig;
use crate::application::daemon::protocol::{Request, Response};

/// Client for connecting to daemon
pub struct DaemonClient {
    socket_path: PathBuf,
}

impl DaemonClient {
    /// Create a client for the given socket
    pub fn new(socket_path: PathBuf) -> Self {
        Self { socket_path }
    }

    /// Create a client for the default socket of a workspace
    pub fn for_workspace(root: &PathBuf) -> Self {
        let socket_path = DaemonConfig::default_socket_path(root);
        Self::new(socket_path)
    }

    /// Check if daemon is running
    pub async fn is_daemon_running(&self) -> bool {
        UnixStream::connect(&self.socket_path).await.is_ok()
    }

    /// Send a request and get response
    pub async fn request(
        &self,
        method: &str,
        params: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, String> {
        let stream = UnixStream::connect(&self.socket_path)
            .await
            .map_err(|e| format!("Failed to connect to daemon: {}", e))?;

        let (reader, mut writer) = stream.into_split();

        // Build request
        let request = Request::new(1i64, method, params);
        let request_json = serde_json::to_string(&request)
            .map_err(|e| format!("Failed to serialize request: {}", e))?;

        // Send request
        writer
            .write_all(request_json.as_bytes())
            .await
            .map_err(|e| format!("Failed to send request: {}", e))?;
        writer
            .write_all(b"\n")
            .await
            .map_err(|e| format!("Failed to send newline: {}", e))?;
        writer
            .flush()
            .await
            .map_err(|e| format!("Failed to flush: {}", e))?;

        // Read response
        let mut reader = BufReader::new(reader);
        let mut response_line = String::new();
        reader
            .read_line(&mut response_line)
            .await
            .map_err(|e| format!("Failed to read response: {}", e))?;

        // Parse response
        let response: Response = serde_json::from_str(response_line.trim())
            .map_err(|e| format!("Failed to parse response: {}", e))?;

        if let Some(error) = response.error {
            Err(format!("RPC error {}: {}", error.code, error.message))
        } else {
            Ok(response.result.unwrap_or(serde_json::Value::Null))
        }
    }

    /// Convenience methods
    pub async fn check(&self, path: &str) -> Result<serde_json::Value, String> {
        self.request("check", Some(serde_json::json!({ "path": path })))
            .await
    }

    pub async fn type_at(
        &self,
        file: &str,
        line: u32,
        column: u32,
    ) -> Result<serde_json::Value, String> {
        self.request(
            "type_at",
            Some(serde_json::json!({
                "file": file,
                "line": line,
                "column": column
            })),
        )
        .await
    }

    pub async fn symbols(&self, file: &str) -> Result<serde_json::Value, String> {
        self.request("symbols", Some(serde_json::json!({ "file": file })))
            .await
    }

    pub async fn diagnostics(&self, file: Option<&str>) -> Result<serde_json::Value, String> {
        let params = file.map(|f| serde_json::json!({ "file": f }));
        self.request("diagnostics", params).await
    }

    pub async fn index_status(&self) -> Result<serde_json::Value, String> {
        self.request("index_status", None).await
    }

    pub async fn shutdown(&self) -> Result<(), String> {
        self.request("shutdown", None).await?;
        Ok(())
    }

    /// Invalidate cache for specific files
    ///
    /// This removes the cached analysis for the specified files, forcing
    /// them to be re-analyzed on the next request.
    pub async fn invalidate(&self, files: &[&str]) -> Result<serde_json::Value, String> {
        let file_list: Vec<String> = files.iter().map(|s| s.to_string()).collect();
        self.request(
            "invalidate",
            Some(serde_json::json!({ "files": file_list })),
        )
        .await
    }

    /// Request hover information at a position
    pub async fn hover(
        &self,
        file: &str,
        line: u32,
        column: u32,
    ) -> Result<serde_json::Value, String> {
        self.request(
            "hover",
            Some(serde_json::json!({
                "file": file,
                "line": line,
                "column": column
            })),
        )
        .await
    }

    /// Request definition location at a position
    pub async fn definition(
        &self,
        file: &str,
        line: u32,
        column: u32,
    ) -> Result<serde_json::Value, String> {
        self.request(
            "definition",
            Some(serde_json::json!({
                "file": file,
                "line": line,
                "column": column
            })),
        )
        .await
    }

    /// Request all references at a position
    pub async fn references(
        &self,
        file: &str,
        line: u32,
        column: u32,
        include_declaration: bool,
    ) -> Result<serde_json::Value, String> {
        self.request(
            "references",
            Some(serde_json::json!({
                "file": file,
                "line": line,
                "column": column,
                "include_declaration": include_declaration
            })),
        )
        .await
    }
}
