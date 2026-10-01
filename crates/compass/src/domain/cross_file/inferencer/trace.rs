use std::path::PathBuf;

use crate::domain::cross_file::inferencer::DeepTypeInferencer;
use crate::type_inference::Type;

/// A step in a type trace.
#[derive(Debug, Clone)]
pub struct TypeTraceStep {
    /// Symbol name
    pub symbol: String,
    /// File containing the symbol
    pub file: PathBuf,
    /// Type at this step
    pub ty: Type,
    /// Line number
    pub line: u32,
}

// ============================================================================
// MCP Tool Functions
// ============================================================================

/// Deep type inference result for MCP.
#[derive(Debug, Clone)]
pub struct DeepInferenceResult {
    /// Inferred type
    pub ty: Type,
    /// Source file
    pub source_file: PathBuf,
    /// Dependencies
    pub dependencies: Vec<String>,
    /// Cross-file references
    pub cross_file_refs: Vec<CrossFileRef>,
}

/// Cross-file reference.
#[derive(Debug, Clone)]
pub struct CrossFileRef {
    /// File path
    pub file: PathBuf,
    /// Symbol name
    pub symbol: String,
    /// Line number
    pub line: u32,
}

/// Infer type with deep cross-file analysis.
pub fn infer_type_deep(
    inferencer: &DeepTypeInferencer,
    symbol: &str,
    file: &PathBuf,
) -> Option<DeepInferenceResult> {
    let binding = inferencer.context.get_binding(file, symbol)?;

    let cross_file_refs = binding
        .dependencies
        .iter()
        .filter_map(|dep| {
            inferencer.context.resolve_type(dep, file).map(|_| {
                // Find where this dependency is defined
                for (f, bindings) in &inferencer.context.bindings {
                    if let Some(b) = bindings.get(dep) {
                        return CrossFileRef {
                            file: f.clone(),
                            symbol: dep.clone(),
                            line: b.line,
                        };
                    }
                }
                CrossFileRef {
                    file: file.clone(),
                    symbol: dep.clone(),
                    line: 0,
                }
            })
        })
        .collect();

    Some(DeepInferenceResult {
        ty: binding.ty.clone(),
        source_file: file.clone(),
        dependencies: binding.dependencies.clone(),
        cross_file_refs,
    })
}

/// Trace type through call chain.
pub fn trace_type_chain(
    inferencer: &DeepTypeInferencer,
    symbol: &str,
    file: &PathBuf,
) -> Vec<TypeTraceStep> {
    inferencer.trace_type(symbol, file)
}
