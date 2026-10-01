use crate::domain::type_codegen::generator::CodeGenerator;
use crate::domain::type_codegen::request::{CodeGenRequest, DocstringStyle};
use crate::domain::type_codegen::result::CodeGenResult;

impl CodeGenerator {
    /// Generate docstring for a function/class.
    pub(super) fn generate_docstring(
        &self,
        request: &CodeGenRequest,
        style: DocstringStyle,
    ) -> CodeGenResult {
        let indent = " ".repeat(request.options.indent.max(self.default_options.indent));

        let docstring = match style {
            DocstringStyle::Google => self.google_docstring(&request.symbol, &indent),
            DocstringStyle::NumPy => self.numpy_docstring(&request.symbol, &indent),
            DocstringStyle::Sphinx => self.sphinx_docstring(&request.symbol, &indent),
            DocstringStyle::RST => self.rst_docstring(&request.symbol, &indent),
        };

        CodeGenResult::new(docstring, request.file.clone())
    }

    fn google_docstring(&self, symbol: &str, indent: &str) -> String {
        format!(
            r#"{indent}"""Short description of {symbol}.

{indent}Longer description if needed.

{indent}Args:
{indent}    param1: Description of param1.
{indent}    param2: Description of param2.

{indent}Returns:
{indent}    Description of return value.

{indent}Raises:
{indent}    ValueError: If invalid input.
{indent}""""#,
            indent = indent,
            symbol = symbol
        )
    }

    fn numpy_docstring(&self, symbol: &str, indent: &str) -> String {
        format!(
            r#"{indent}"""
{indent}Short description of {symbol}.

{indent}Parameters
{indent}----------
{indent}param1 : type
{indent}    Description of param1.
{indent}param2 : type
{indent}    Description of param2.

{indent}Returns
{indent}-------
{indent}type
{indent}    Description of return value.
{indent}""""#,
            indent = indent,
            symbol = symbol
        )
    }

    fn sphinx_docstring(&self, symbol: &str, indent: &str) -> String {
        format!(
            r#"{indent}"""Short description of {symbol}.

{indent}:param param1: Description of param1.
{indent}:type param1: type
{indent}:param param2: Description of param2.
{indent}:type param2: type
{indent}:returns: Description of return value.
{indent}:rtype: type
{indent}:raises ValueError: If invalid input.
{indent}""""#,
            indent = indent,
            symbol = symbol
        )
    }

    fn rst_docstring(&self, symbol: &str, indent: &str) -> String {
        self.sphinx_docstring(symbol, indent)
    }
}
