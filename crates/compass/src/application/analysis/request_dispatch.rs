use crate::application::analysis::request_handler::RequestHandler;
use crate::domain::daemon::protocol::*;

impl RequestHandler {
    /// Handle a JSON-RPC request
    pub async fn handle(&self, request: Request) -> Response {
        let result = match request.method.as_str() {
            "check" => self.handle_check(request.params).await,
            "type_at" => self.handle_type_at(request.params).await,
            "symbols" => self.handle_symbols(request.params).await,
            "diagnostics" => self.handle_diagnostics(request.params).await,
            "hover" => self.handle_hover(request.params).await,
            "definition" => self.handle_definition(request.params).await,
            "references" => self.handle_references(request.params).await,
            "index_status" => self.handle_index_status().await,
            "invalidate" => self.handle_invalidate(request.params).await,
            "shutdown" => self.handle_shutdown().await,
            // PDG tools (R101-R105)
            "pdg" => self.handle_pdg(request.params).await,
            "slice" => self.handle_slice(request.params).await,
            "impact" => self.handle_impact(request.params).await,
            "taint" => self.handle_taint(request.params).await,
            _ => Err(RpcError::method_not_found(&request.method)),
        };

        match result {
            Ok(value) => Response::success(request.id, value),
            Err(error) => Response::error(request.id, error),
        }
    }

    /// Handle one line of the socket protocol: parse it as a JSON-RPC
    /// request, dispatch it, and return the response as JSON.
    ///
    /// A line that is not a request gets a parse-error response with id 0.
    /// Errs only if the response cannot be serialized.
    pub(crate) async fn handle_json_line(&self, line: &str) -> Result<String, String> {
        let response = match serde_json::from_str::<Request>(line.trim()) {
            Ok(request) => self.handle(request).await,
            Err(e) => Response::error(
                RequestId::Number(0),
                RpcError::parse_error(format!("Invalid JSON: {}", e)),
            ),
        };
        serde_json::to_string(&response).map_err(|e| format!("Failed to serialize response: {}", e))
    }
}
