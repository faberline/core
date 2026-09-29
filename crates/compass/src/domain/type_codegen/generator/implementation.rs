use crate::domain::type_codegen::generator::CodeGenerator;
use crate::domain::type_codegen::request::CodeGenRequest;
use crate::domain::type_codegen::result::CodeGenResult;

impl CodeGenerator {
    /// Generate implementation from protocol.
    pub(super) fn generate_implementation(
        &self,
        request: &CodeGenRequest,
        protocol: &str,
    ) -> CodeGenResult {
        let code = format!(
            r#"class {symbol}({protocol}):
    """Implementation of {protocol}."""

    def __init__(self) -> None:
        """Initialize {symbol}."""
        pass

    # TODO: Implement required methods from {protocol}
"#,
            symbol = request.symbol,
            protocol = protocol
        );

        CodeGenResult::new(code, request.file.clone())
    }

    /// Generate constructor.
    pub(super) fn generate_constructor(&self, request: &CodeGenRequest) -> CodeGenResult {
        let code = format!(
            r#"    def __init__(self) -> None:
        """Initialize {symbol}."""
        pass
"#,
            symbol = request.symbol
        );

        CodeGenResult::new(code, request.file.clone())
    }

    /// Generate property accessors.
    pub(super) fn generate_properties(
        &self,
        request: &CodeGenRequest,
        fields: &[String],
    ) -> CodeGenResult {
        let mut code = String::new();

        for field in fields {
            code.push_str(&format!(
                r#"    @property
    def {field}(self) -> Any:
        """Get {field}."""
        return self._{field}

    @{field}.setter
    def {field}(self, value: Any) -> None:
        """Set {field}."""
        self._{field} = value

"#,
                field = field
            ));
        }

        CodeGenResult::new(code, request.file.clone()).with_import("from typing import Any")
    }
}
