mod docstring;
mod implementation;
mod test_stub;
mod type_stub;

use crate::domain::type_codegen::request::{CodeGenKind, CodeGenOptions, CodeGenRequest};
use crate::domain::type_codegen::result::CodeGenResult;
use crate::type_inference::TypeContext;

// ============================================================================
// Code Generator
// ============================================================================

/// Code generator with type awareness.
pub struct CodeGenerator {
    /// Type context for type information
    type_context: TypeContext,
    /// Default options
    default_options: CodeGenOptions,
}

impl CodeGenerator {
    /// Create a new code generator.
    pub fn new() -> Self {
        Self {
            type_context: TypeContext::new(),
            default_options: CodeGenOptions {
                include_types: true,
                include_examples: true,
                async_support: false,
                indent: 4,
            },
        }
    }

    /// Create with type context.
    pub fn with_context(type_context: TypeContext) -> Self {
        Self {
            type_context,
            default_options: CodeGenOptions::default(),
        }
    }

    /// Generate code based on request.
    pub fn generate(&self, request: &CodeGenRequest) -> CodeGenResult {
        match &request.kind {
            CodeGenKind::Docstring { style } => self.generate_docstring(request, *style),
            CodeGenKind::TestStub { framework } => self.generate_test_stub(request, *framework),
            CodeGenKind::TypeStub => self.generate_type_stub(request),
            CodeGenKind::ModuleStub => self.generate_module_stub(request),
            CodeGenKind::Implementation { protocol } => {
                self.generate_implementation(request, protocol)
            }
            CodeGenKind::Constructor => self.generate_constructor(request),
            CodeGenKind::Properties { fields } => self.generate_properties(request, fields),
        }
    }
}

impl Default for CodeGenerator {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests;
