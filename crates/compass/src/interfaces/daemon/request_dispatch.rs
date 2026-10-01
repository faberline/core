use crate::application::analysis::request_handler::RequestHandler;
use crate::application::daemon::protocol::*;

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
}
