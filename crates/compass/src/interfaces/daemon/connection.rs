use std::sync::Arc;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use tokio::sync::broadcast;

use crate::application::analysis::request_handler::RequestHandler;

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
                        let response_json = handler.handle_json_line(&line).await?;

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
