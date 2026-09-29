use crate::domain::type_codegen::generator::CodeGenerator;
use crate::domain::type_codegen::request::{CodeGenRequest, TestFramework};
use crate::domain::type_codegen::result::CodeGenResult;

impl CodeGenerator {
    /// Generate test stubs.
    pub(super) fn generate_test_stub(
        &self,
        request: &CodeGenRequest,
        framework: TestFramework,
    ) -> CodeGenResult {
        let code = match framework {
            TestFramework::Pytest => self.pytest_stub(&request.symbol),
            TestFramework::Unittest => self.unittest_stub(&request.symbol),
            TestFramework::Doctest => self.doctest_stub(&request.symbol),
        };

        let mut result = CodeGenResult::new(code, request.file.clone());

        match framework {
            TestFramework::Pytest => {
                result = result.with_import("import pytest");
            }
            TestFramework::Unittest => {
                result = result.with_import("import unittest");
            }
            TestFramework::Doctest => {}
        }

        result
    }

    fn pytest_stub(&self, symbol: &str) -> String {
        format!(
            r#"import pytest

class Test{symbol}:
    """Tests for {symbol}."""

    def test_{symbol_lower}_basic(self):
        """Test basic functionality."""
        # Arrange

        # Act

        # Assert
        assert True

    def test_{symbol_lower}_edge_case(self):
        """Test edge cases."""
        # Arrange

        # Act

        # Assert
        assert True

    @pytest.mark.parametrize("input,expected", [
        ("input1", "expected1"),
        ("input2", "expected2"),
    ])
    def test_{symbol_lower}_parametrized(self, input, expected):
        """Test with various inputs."""
        assert True
"#,
            symbol = symbol,
            symbol_lower = symbol.to_lowercase()
        )
    }

    fn unittest_stub(&self, symbol: &str) -> String {
        format!(
            r#"import unittest

class Test{symbol}(unittest.TestCase):
    """Tests for {symbol}."""

    def setUp(self):
        """Set up test fixtures."""
        pass

    def tearDown(self):
        """Tear down test fixtures."""
        pass

    def test_{symbol_lower}_basic(self):
        """Test basic functionality."""
        self.assertTrue(True)

    def test_{symbol_lower}_edge_case(self):
        """Test edge cases."""
        self.assertTrue(True)

if __name__ == "__main__":
    unittest.main()
"#,
            symbol = symbol,
            symbol_lower = symbol.to_lowercase()
        )
    }

    fn doctest_stub(&self, symbol: &str) -> String {
        format!(
            r#"def {symbol_lower}():
    """
    Description of {symbol}.

    Examples
    --------
    >>> {symbol_lower}()
    expected_output

    >>> {symbol_lower}(arg)
    expected_output
    """
    pass
"#,
            symbol = symbol,
            symbol_lower = symbol.to_lowercase()
        )
    }
}
