use crate::domain::daemon::protocol::*;
use crate::semantic::{CfgBuilder, PdgJson, ProgramDependenceGraph};
use crate::syntax::{Language, ParsedFile};

use super::request_handler::RequestHandler;

impl RequestHandler {
    // =========================================================================
    // PDG tools (R101–R106)
    // =========================================================================

    /// Build and return the PDG for a Python file or function (R101)
    pub(crate) async fn handle_pdg(
        &self,
        params: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, RpcError> {
        let params: PdgParams = serde_json::from_value(
            params.ok_or_else(|| RpcError::invalid_params("Missing params"))?,
        )
        .map_err(|e| RpcError::invalid_params(e.to_string()))?;

        let path = self.resolve_path(&params.file);
        let source = self
            .get_document_content(&path)
            .await
            .map_err(|e| RpcError::invalid_params(e))?;

        let mut parser_guard = self.parser.lock().await;
        let parsed = parser_guard
            .parse(&source, Language::Python)
            .ok_or_else(|| RpcError::invalid_params("Failed to parse Python file"))?;
        drop(parser_guard);

        let pdg = if let Some(fn_name) = &params.function {
            // Build PDG for a specific function by finding the function node
            self.build_function_pdg(&source, &parsed, fn_name)
                .unwrap_or_else(|| ProgramDependenceGraph::build(&source, &parsed))
        } else {
            ProgramDependenceGraph::build(&source, &parsed)
        };

        let pdg = pdg.with_file_path(path);
        let json: PdgJson = (&pdg).into();

        serde_json::to_value(json).map_err(|e| RpcError::internal_error(e.to_string()))
    }

    /// Compute a program slice from a criterion line (R102)
    pub(crate) async fn handle_slice(
        &self,
        params: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, RpcError> {
        let params: SliceParams = serde_json::from_value(
            params.ok_or_else(|| RpcError::invalid_params("Missing params"))?,
        )
        .map_err(|e| RpcError::invalid_params(e.to_string()))?;

        let path = self.resolve_path(&params.file);
        let source = self
            .get_document_content(&path)
            .await
            .map_err(|e| RpcError::invalid_params(e))?;

        let mut parser_guard = self.parser.lock().await;
        let parsed = parser_guard
            .parse(&source, Language::Python)
            .ok_or_else(|| RpcError::invalid_params("Failed to parse Python file"))?;
        drop(parser_guard);

        let pdg = ProgramDependenceGraph::build(&source, &parsed);

        let slice = match params.direction.as_str() {
            "forward" => pdg.forward_slice(params.line),
            "backward" => pdg.backward_slice(params.line),
            other => {
                return Err(RpcError::invalid_params(format!(
                    "Invalid direction '{}': must be 'forward' or 'backward'",
                    other
                )))
            }
        };

        let nodes: Vec<SliceNodeInfo> = slice
            .nodes
            .iter()
            .map(|n| SliceNodeInfo {
                line: n.line,
                text: n.text.clone(),
                kind: format!("{:?}", n.kind),
            })
            .collect();

        let result = SliceResult {
            direction: params.direction,
            criterion_line: params.line,
            line_count: nodes.len(),
            nodes,
        };

        serde_json::to_value(result).map_err(|e| RpcError::internal_error(e.to_string()))
    }

    /// Compute change impact analysis with dependency tree (R103)
    pub(crate) async fn handle_impact(
        &self,
        params: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, RpcError> {
        let params: ImpactParams = serde_json::from_value(
            params.ok_or_else(|| RpcError::invalid_params("Missing params"))?,
        )
        .map_err(|e| RpcError::invalid_params(e.to_string()))?;

        let path = self.resolve_path(&params.file);
        let source = self
            .get_document_content(&path)
            .await
            .map_err(|e| RpcError::invalid_params(e))?;

        let mut parser_guard = self.parser.lock().await;
        let parsed = parser_guard
            .parse(&source, Language::Python)
            .ok_or_else(|| RpcError::invalid_params("Failed to parse Python file"))?;
        drop(parser_guard);

        let pdg = ProgramDependenceGraph::build(&source, &parsed);
        let impact = pdg.impact_analysis_tree(&params.changed_lines);

        // Convert tree nodes to protocol format
        fn convert_tree(nodes: &[crate::semantic::ImpactTreeNode]) -> Vec<ImpactNode> {
            nodes
                .iter()
                .map(|n| ImpactNode {
                    line: n.line,
                    text: n.text.clone(),
                    reason: format!("{:?}", n.reason).to_lowercase(),
                    variable: n.variable.clone(),
                    children: convert_tree(&n.children),
                })
                .collect()
        }

        let result = ImpactResult {
            changed_lines: impact.changed_lines,
            total_affected: impact.affected_lines.len(),
            affected_lines: impact.affected_lines,
            impact_tree: convert_tree(&impact.tree),
        };

        serde_json::to_value(result).map_err(|e| RpcError::internal_error(e.to_string()))
    }

    /// Trace tainted data from sources to sinks (R104)
    ///
    /// If sources/sinks are empty in params, auto-detects them from the code
    /// using pattern matching for known taint sources and sinks.
    pub(crate) async fn handle_taint(
        &self,
        params: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, RpcError> {
        let params: TaintParams = serde_json::from_value(
            params.ok_or_else(|| RpcError::invalid_params("Missing params"))?,
        )
        .map_err(|e| RpcError::invalid_params(e.to_string()))?;

        let path = self.resolve_path(&params.file);
        let source = self
            .get_document_content(&path)
            .await
            .map_err(|e| RpcError::invalid_params(e))?;

        let mut parser_guard = self.parser.lock().await;
        let parsed = parser_guard
            .parse(&source, Language::Python)
            .ok_or_else(|| RpcError::invalid_params("Failed to parse Python file"))?;
        drop(parser_guard);

        let pdg = ProgramDependenceGraph::build(&source, &parsed);

        let auto_detected = params.sources.is_empty() && params.sinks.is_empty();

        let (taint_paths, source_lines, sink_lines) = if auto_detected {
            // Use semantic auto-detection (R7)
            let analysis = pdg.semantic_taint_analysis();
            let src_lines: Vec<usize> = analysis.sources.iter().map(|s| s.line).collect();
            let snk_lines: Vec<usize> = analysis.sinks.iter().map(|s| s.line).collect();
            let paths = analysis.taint_paths;
            (paths, src_lines, snk_lines)
        } else {
            // Use explicit lines provided by caller
            let taint = pdg.taint_analysis_explicit(&params.sources, &params.sinks);
            (taint.taint_paths, params.sources, params.sinks)
        };

        // Enrich taint path info with node text
        let path_infos: Vec<TaintPathInfo> = taint_paths
            .iter()
            .map(|tp| {
                let src_text = pdg
                    .get_node_by_line(tp.source)
                    .map(|n| n.text.clone())
                    .unwrap_or_default();
                let snk_text = pdg
                    .get_node_by_line(tp.sink)
                    .map(|n| n.text.clone())
                    .unwrap_or_default();

                TaintPathInfo {
                    source_line: tp.source,
                    source_text: src_text,
                    source_kind: "taint_source".to_string(),
                    sink_line: tp.sink,
                    sink_text: snk_text,
                    sink_kind: "taint_sink".to_string(),
                    path: tp.path.clone(),
                }
            })
            .collect();

        let has_vulns = !path_infos.is_empty();

        let result = TaintResult {
            source_lines,
            sink_lines,
            has_vulnerabilities: has_vulns,
            auto_detected,
            taint_paths: path_infos,
        };

        serde_json::to_value(result).map_err(|e| RpcError::internal_error(e.to_string()))
    }

    /// Build PDG for a specific function by name
    fn build_function_pdg(
        &self,
        source: &str,
        file: &ParsedFile,
        fn_name: &str,
    ) -> Option<ProgramDependenceGraph> {
        let root = file.root_node();
        let mut cursor = root.walk();

        for node in root.children(&mut cursor) {
            if node.kind() == "function_definition" || node.kind() == "async_function_definition" {
                if let Some(name_node) = node.child_by_field_name("name") {
                    if file.node_text(&name_node) == fn_name {
                        let cfg = CfgBuilder::new(source).build_function(&node, file);
                        return Some(ProgramDependenceGraph::from_cfg(cfg, file));
                    }
                }
            }
        }

        None
    }
}
