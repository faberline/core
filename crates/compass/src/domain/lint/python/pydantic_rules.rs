use super::PythonChecker;
use crate::diagnostic::{Diagnostic, DiagnosticCategory};
use crate::domain::syntax::parsed_file::NodeRange;
use crate::syntax::ParsedFile;

impl PythonChecker {
    // ===== Pydantic Checks =====

    /// Check if a class inherits from BaseModel (Pydantic)
    fn is_pydantic_model(node: &tree_sitter::Node<'_>, file: &ParsedFile) -> bool {
        if node.kind() != "class_definition" {
            return false;
        }

        // Check superclass_list for BaseModel
        if let Some(bases) = node.child_by_field_name("superclasses") {
            let mut cursor = bases.walk();
            for child in bases.children(&mut cursor) {
                let text = file.node_text(&child);
                // Match BaseModel, pydantic.BaseModel, etc.
                if text == "BaseModel"
                    || text.ends_with(".BaseModel")
                    || text == "BaseSettings"
                    || text.ends_with(".BaseSettings")
                {
                    return true;
                }
            }
        }
        false
    }

    /// PY501: Mutable default in Pydantic model field
    pub(super) fn check_pydantic_mutable_default(&self, file: &ParsedFile) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();

        file.walk(|node, _depth| {
            if Self::is_pydantic_model(node, file) {
                // Check class body for field definitions
                if let Some(body) = node.child_by_field_name("body") {
                    let mut cursor = body.walk();
                    for child in body.children(&mut cursor) {
                        // Look for annotated assignments: field: type = value
                        if child.kind() == "expression_statement" {
                            if let Some(expr) = child.child(0) {
                                if expr.kind() == "assignment" {
                                    if let Some(value) = expr.child_by_field_name("right") {
                                        let value_kind = value.kind();
                                        // Mutable defaults: [], {}, set()
                                        if value_kind == "list"
                                            || value_kind == "dictionary"
                                            || value_kind == "set"
                                        {
                                            diagnostics.push(Diagnostic::warning(
                                                value.to_range(),
                                                "PY501",
                                                DiagnosticCategory::Logic,
                                                "Mutable default in Pydantic model. Use Field(default_factory=...) instead",
                                            ));
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            true
        });

        diagnostics
    }

    /// PY502: Deprecated @validator decorator (Pydantic V1 -> V2)
    pub(super) fn check_pydantic_deprecated_validator(&self, file: &ParsedFile) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();

        file.walk(|node, _depth| {
            if node.kind() == "decorator" {
                let text = file.node_text(node);
                // @validator(...) is deprecated in V2, use @field_validator
                if text.starts_with("@validator") && !text.starts_with("@validator_") {
                    diagnostics.push(Diagnostic::warning(
                        node.to_range(),
                        "PY502",
                        DiagnosticCategory::Style,
                        "Deprecated: @validator is Pydantic V1. Use @field_validator in V2",
                    ));
                }
                // @root_validator is deprecated, use @model_validator
                if text.starts_with("@root_validator") {
                    diagnostics.push(Diagnostic::warning(
                        node.to_range(),
                        "PY502",
                        DiagnosticCategory::Style,
                        "Deprecated: @root_validator is Pydantic V1. Use @model_validator in V2",
                    ));
                }
            }
            true
        });

        diagnostics
    }

    /// PY503: Deprecated Config class in Pydantic model (V1 -> V2)
    pub(super) fn check_pydantic_deprecated_config(&self, file: &ParsedFile) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();

        file.walk(|node, _depth| {
            if Self::is_pydantic_model(node, file) {
                // Check class body for nested Config class
                if let Some(body) = node.child_by_field_name("body") {
                    let mut cursor = body.walk();
                    for child in body.children(&mut cursor) {
                        if child.kind() == "class_definition" {
                            if let Some(name) = child.child_by_field_name("name") {
                                if file.node_text(&name) == "Config" {
                                    diagnostics.push(Diagnostic::warning(
                                        child.to_range(),
                                        "PY503",
                                        DiagnosticCategory::Style,
                                        "Deprecated: class Config is Pydantic V1. Use model_config = ConfigDict(...) in V2",
                                    ));
                                }
                            }
                        }
                    }
                }
            }
            true
        });

        diagnostics
    }
}
