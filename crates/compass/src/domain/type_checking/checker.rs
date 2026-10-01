//! Type checking - verifies type compatibility and generates diagnostics

mod assignability;
mod calls;
mod control_flow;
mod declarations;
mod statements;
mod variance;

use tree_sitter::Node;

use crate::diagnostic::Diagnostic;
use crate::domain::narrowing::narrower::TypeNarrower;
use crate::domain::python_inference::inferencer::TypeInferencer;
use crate::domain::type_system::ty::Type;
use crate::syntax::ParsedFile;

/// Function context for tracking return types
#[derive(Debug, Clone)]
struct FunctionContext {
    name: String,
    return_type: Type,
    has_return: bool,
}

/// Position in a function signature for variance checking
#[derive(Debug, Clone, Copy)]
enum VariancePosition {
    /// Input position (parameters) - contravariant
    Input,
    /// Output position (return type) - covariant
    Output,
}

/// Type checker that combines inference with compatibility checking
pub struct TypeChecker<'a> {
    /// Type inferencer
    inferencer: TypeInferencer<'a>,
    /// Collected diagnostics
    diagnostics: Vec<Diagnostic>,
    /// Source code
    source: &'a str,
    /// Stack of function contexts (for nested functions)
    function_stack: Vec<FunctionContext>,
    /// Type narrower for control flow analysis
    narrower: TypeNarrower,
}

impl<'a> TypeChecker<'a> {
    pub fn new(source: &'a str) -> Self {
        Self {
            inferencer: TypeInferencer::new(source),
            diagnostics: Vec::new(),
            source,
            function_stack: Vec::new(),
            narrower: TypeNarrower::new(),
        }
    }

    /// Get current function context
    fn current_function(&self) -> Option<&FunctionContext> {
        self.function_stack.last()
    }

    /// Mark current function as having a return statement
    fn mark_has_return(&mut self) {
        if let Some(ctx) = self.function_stack.last_mut() {
            ctx.has_return = true;
        }
    }

    /// Check a file and return diagnostics
    pub fn check_file(&mut self, file: &ParsedFile) -> Vec<Diagnostic> {
        let root = file.tree.root_node();
        self.check_node(&root);
        std::mem::take(&mut self.diagnostics)
    }

    /// Get text of a node
    fn node_text(&self, node: &Node) -> &str {
        node.utf8_text(self.source.as_bytes()).unwrap_or("")
    }

    /// Get collected diagnostics
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }
}

#[cfg(test)]
mod tests;
