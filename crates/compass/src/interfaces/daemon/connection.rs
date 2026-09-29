use std::sync::Arc;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use tokio::sync::broadcast;

use crate::application::analysis::request_handler::RequestHandler;
use crate::application::daemon::protocol::{Request, Response, RpcError};

/// Handle a single client connection
pub(super) async fn handle_connection(
    stream: UnixStream,
    handler: Arc<RequestHandler>,
    shutdown_rx: &mut broadcast::Receiver<()>,
) -> Result<(), String> {
    let (reader, mut writer) = stream.into_split();
    let mut reader = BufReader::new(reader);
    let mut line = String::new();

    loop {
        line.clear();

        tokio::select! {
            result = reader.read_line(&mut line) => {
                match result {
                    Ok(0) => {
                        // Connection closed
                        break;
                    }
                    Ok(_) => {
                        let response = process_request(&line, &handler).await;
                        let response_json = serde_json::to_string(&response)
                            .map_err(|e| format!("Failed to serialize response: {}", e))?;

                        writer.write_all(response_json.as_bytes()).await
                            .map_err(|e| format!("Failed to write response: {}", e))?;
                        writer.write_all(b"\n").await
                            .map_err(|e| format!("Failed to write newline: {}", e))?;
                        writer.flush().await
                            .map_err(|e| format!("Failed to flush: {}", e))?;

                        // Check for shutdown request
                        if line.contains("\"method\":\"shutdown\"") {
                            break;
                        }
                    }
                    Err(e) => {
                        return Err(format!("Read error: {}", e));
                    }
                }
            }
            _ = shutdown_rx.recv() => {
                break;
            }
        }
    }

    Ok(())
}

/// Process a single request
async fn process_request(line: &str, handler: &RequestHandler) -> Response {
    // Parse request
    let request: Request = match serde_json::from_str(line.trim()) {
        Ok(r) => r,
        Err(e) => {
            return Response::error(
                crate::application::daemon::protocol::RequestId::Number(0),
                RpcError::parse_error(format!("Invalid JSON: {}", e)),
            );
        }
    };

    // Handle request
    handler.handle(request).await
}
