mod params;
mod visit;

use std::path::PathBuf;

use tree_sitter::Node;

use crate::diagnostic::Range;
use crate::domain::semantic_model::ids::ScopeId;
use crate::domain::semantic_model::model::SemanticModel;
use crate::domain::semantic_model::type_info::TypeInfo;
use crate::domain::type_system::annotation::parse_type_annotation;
use crate::syntax::ParsedFile;

// ============================================================================
// SemanticModel builder - produces an owned SemanticModel from type checking
// ============================================================================

/// Builder for creating SemanticModel from parsed code
///
/// This struct traverses the AST and collects type information,
/// producing an owned SemanticModel that can be cached and queried.
pub struct SemanticModelBuilder<'a> {
    source: &'a str,
    file_path: PathBuf,
    model: SemanticModel,
    current_scope: ScopeId,
    scope_stack: Vec<ScopeId>,
}

impl<'a> SemanticModelBuilder<'a> {
    /// Create a new builder for the given source
    pub fn new(source: &'a str, file_path: PathBuf) -> Self {
        let mut model = SemanticModel::new();
        let root_scope = model.add_scope(None, Range::default());

        Self {
            source,
            file_path,
            model,
            current_scope: root_scope,
            scope_stack: vec![root_scope],
        }
    }

    /// Build a SemanticModel from a parsed file
    pub fn build(mut self, file: &ParsedFile) -> SemanticModel {
        let root = file.tree.root_node();
        self.visit_node(&root);
        self.model.finalize();
        self.model
    }

    /// Push a new scope
    fn push_scope(&mut self, range: Range) -> ScopeId {
        let new_scope = self.model.add_scope(Some(self.current_scope), range);
        self.scope_stack.push(self.current_scope);
        self.current_scope = new_scope;
        new_scope
    }

    /// Pop the current scope
    fn pop_scope(&mut self) {
        if let Some(parent) = self.scope_stack.pop() {
            self.current_scope = parent;
        }
    }

    /// Parse a type from a node using the annotation parser
    fn parse_type_from_node(&self, node: &Node) -> TypeInfo {
        let ty = parse_type_annotation(self.source, node);
        TypeInfo::from_type(&ty)
    }

    /// Get text of a node
    fn node_text(&self, node: &Node) -> &str {
        node.utf8_text(self.source.as_bytes()).unwrap_or("")
    }
}

/// Create a SemanticModel from a parsed file
pub fn build_semantic_model(file: &ParsedFile, source: &str, file_path: PathBuf) -> SemanticModel {
    SemanticModelBuilder::new(source, file_path).build(file)
}
