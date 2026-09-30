use crate::domain::daemon::protocol::*;
use crate::syntax::Language;

use super::request_handler::RequestHandler;

impl RequestHandler {
    /// Get type at position
    pub(crate) async fn handle_type_at(
        &self,
        params: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, RpcError> {
        let params: TypeAtParams = params
            .ok_or_else(|| RpcError::invalid_params("Missing params"))?
            .try_into()
            .map_err(|e| RpcError::invalid_params(format!("Invalid params: {}", e)))?;

        let path = self.resolve_path(&params.file);
        self.ensure_analyzed(&path).await?;

        let cache = self.cache.read().await;
        let analysis = cache
            .get(&path)
            .ok_or_else(|| RpcError::invalid_params("File not found in cache"))?;

        // First try SemanticModel for type info
        if let Some(type_info) = analysis.semantic_model.type_at(params.line, params.column) {
            return serde_json::to_value(type_info.display())
                .map_err(|e| RpcError::internal_error(e.to_string()));
        }

        // Fall back to symbol table
        let symbol = analysis
            .symbol_table
            .find_at_position(params.line, params.column);

        match symbol {
            Some(sym) => {
                let type_str = sym
                    .type_info
                    .as_ref()
                    .map(|t| format!("{:?}", t))
                    .unwrap_or_else(|| "Unknown".to_string());
                serde_json::to_value(type_str).map_err(|e| RpcError::internal_error(e.to_string()))
            }
            None => Ok(serde_json::Value::Null),
        }
    }

    /// List symbols in a file
    pub(crate) async fn handle_symbols(
        &self,
        params: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, RpcError> {
        let params: SymbolsParams = params
            .ok_or_else(|| RpcError::invalid_params("Missing params"))?
            .try_into()
            .map_err(|e| RpcError::invalid_params(format!("Invalid params: {}", e)))?;

        let path = self.resolve_path(&params.file);
        self.ensure_analyzed(&path).await?;

        let cache = self.cache.read().await;
        let analysis = cache
            .get(&path)
            .ok_or_else(|| RpcError::invalid_params("File not found in cache"))?;

        let symbols: Vec<SymbolInfo> = analysis
            .symbol_table
            .all_symbols()
            .iter()
            .map(|sym| SymbolInfo {
                name: sym.name.clone(),
                kind: format!("{:?}", sym.kind),
                line: sym.location.start.line,
                column: sym.location.start.character,
                type_info: sym.type_info.as_ref().map(|t| format!("{:?}", t)),
            })
            .collect();

        serde_json::to_value(symbols).map_err(|e| RpcError::internal_error(e.to_string()))
    }

    /// Get hover information
    pub(crate) async fn handle_hover(
        &self,
        params: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, RpcError> {
        let params: HoverParams = params
            .ok_or_else(|| RpcError::invalid_params("Missing params"))?
            .try_into()
            .map_err(|e| RpcError::invalid_params(format!("Invalid params: {}", e)))?;

        let path = self.resolve_path(&params.file);
        self.ensure_analyzed(&path).await?;

        let cache = self.cache.read().await;
        let analysis = cache
            .get(&path)
            .ok_or_else(|| RpcError::invalid_params("File not found in cache"))?;

        // First try SemanticModel for hover info
        if let Some(hover_content) = analysis.semantic_model.hover_at(params.line, params.column) {
            let response = serde_json::json!({
                "contents": {
                    "kind": "markdown",
                    "value": hover_content
                }
            });
            return serde_json::to_value(response)
                .map_err(|e| RpcError::internal_error(e.to_string()));
        }

        // Fall back to symbol table
        let symbol = analysis
            .symbol_table
            .find_at_position(params.line, params.column);

        match symbol {
            Some(sym) => {
                let content = sym.hover_content(Language::Python);
                serde_json::to_value(content).map_err(|e| RpcError::internal_error(e.to_string()))
            }
            None => Ok(serde_json::Value::Null),
        }
    }

    /// Go to definition
    pub(crate) async fn handle_definition(
        &self,
        params: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, RpcError> {
        let params: DefinitionParams = params
            .ok_or_else(|| RpcError::invalid_params("Missing params"))?
            .try_into()
            .map_err(|e| RpcError::invalid_params(format!("Invalid params: {}", e)))?;

        let path = self.resolve_path(&params.file);
        self.ensure_analyzed(&path).await?;

        let cache = self.cache.read().await;
        let analysis = cache
            .get(&path)
            .ok_or_else(|| RpcError::invalid_params("File not found in cache"))?;

        // First try SemanticModel for definition
        if let Some(symbol_data) = analysis
            .semantic_model
            .definition_at(params.line, params.column)
        {
            let loc = LocationInfo {
                file: symbol_data.file_path.to_string_lossy().to_string(),
                line: symbol_data.def_range.start.line,
                column: symbol_data.def_range.start.character,
                end_line: symbol_data.def_range.end.line,
                end_column: symbol_data.def_range.end.character,
            };
            return serde_json::to_value(loc).map_err(|e| RpcError::internal_error(e.to_string()));
        }

        // Fall back to symbol table
        let symbol = analysis
            .symbol_table
            .find_definition_at(params.line, params.column);

        match symbol {
            Some(sym) => {
                let loc = LocationInfo {
                    file: path.to_string_lossy().to_string(),
                    line: sym.location.start.line,
                    column: sym.location.start.character,
                    end_line: sym.location.end.line,
                    end_column: sym.location.end.character,
                };
                serde_json::to_value(loc).map_err(|e| RpcError::internal_error(e.to_string()))
            }
            None => Ok(serde_json::Value::Null),
        }
    }

    /// Find references
    pub(crate) async fn handle_references(
        &self,
        params: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, RpcError> {
        let params: ReferencesParams = params
            .ok_or_else(|| RpcError::invalid_params("Missing params"))?
            .try_into()
            .map_err(|e| RpcError::invalid_params(format!("Invalid params: {}", e)))?;

        let path = self.resolve_path(&params.file);
        self.ensure_analyzed(&path).await?;

        let cache = self.cache.read().await;
        let analysis = cache
            .get(&path)
            .ok_or_else(|| RpcError::invalid_params("File not found in cache"))?;

        // First try SemanticModel for references
        let sem_refs = analysis.semantic_model.references_at(
            params.line,
            params.column,
            params.include_declaration,
        );
        if !sem_refs.is_empty() {
            let locations: Vec<LocationInfo> = sem_refs
                .into_iter()
                .map(|r| LocationInfo {
                    file: path.to_string_lossy().to_string(),
                    line: r.range.start.line,
                    column: r.range.start.character,
                    end_line: r.range.end.line,
                    end_column: r.range.end.character,
                })
                .collect();
            return serde_json::to_value(locations)
                .map_err(|e| RpcError::internal_error(e.to_string()));
        }

        // Fall back to symbol table
        let refs = analysis.symbol_table.find_references_at(
            params.line,
            params.column,
            params.include_declaration,
        );

        let locations: Vec<LocationInfo> = refs
            .into_iter()
            .map(|r| LocationInfo {
                file: path.to_string_lossy().to_string(),
                line: r.start.line,
                column: r.start.character,
                end_line: r.end.line,
                end_column: r.end.character,
            })
            .collect();

        serde_json::to_value(locations).map_err(|e| RpcError::internal_error(e.to_string()))
    }
}
