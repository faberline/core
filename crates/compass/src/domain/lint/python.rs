use crate::domain::syntax::parsed_file::NodeRange;
use std::collections::HashSet;

use crate::diagnostic::{Diagnostic, DiagnosticCategory};
use crate::domain::check::lint_config::LintConfig;
use crate::syntax::{Language, ParsedFile};

mod basic_rules;
mod complexity_rules;
mod import_rules;
mod inspection_rules;
mod pydantic_rules;
mod scope_rules;

/// Python checker
pub struct PythonChecker {
    builtins: HashSet<&'static str>,
}

impl PythonChecker {
    pub fn new() -> Self {
        Self {
            builtins: [
                "abs",
                "all",
                "any",
                "ascii",
                "bin",
                "bool",
                "breakpoint",
                "bytearray",
                "bytes",
                "callable",
                "chr",
                "classmethod",
                "compile",
                "complex",
                "delattr",
                "dict",
                "dir",
                "divmod",
                "enumerate",
                "eval",
                "exec",
                "filter",
                "float",
                "format",
                "frozenset",
                "getattr",
                "globals",
                "hasattr",
                "hash",
                "help",
                "hex",
                "id",
                "input",
                "int",
                "isinstance",
                "issubclass",
                "iter",
                "len",
                "list",
                "locals",
                "map",
                "max",
                "memoryview",
                "min",
                "next",
                "object",
                "oct",
                "open",
                "ord",
                "pow",
                "print",
                "property",
                "range",
                "repr",
                "reversed",
                "round",
                "set",
                "setattr",
                "slice",
                "sorted",
                "staticmethod",
                "str",
                "sum",
                "super",
                "tuple",
                "type",
                "vars",
                "zip",
                "__import__",
                // Common exceptions
                "Exception",
                "BaseException",
                "TypeError",
                "ValueError",
                "KeyError",
                "IndexError",
                "AttributeError",
                "RuntimeError",
                "StopIteration",
                "NotImplementedError",
                "AssertionError",
                "ImportError",
                "OSError",
                // Constants
                "True",
                "False",
                "None",
                "Ellipsis",
                "NotImplemented",
            ]
            .into_iter()
            .collect(),
        }
    }
}

impl Default for PythonChecker {
    fn default() -> Self {
        Self::new()
    }
}

impl super::checker::Checker for PythonChecker {
    fn language(&self) -> Language {
        Language::Python
    }

    fn check(&self, file: &ParsedFile, _config: &LintConfig) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();

        // Check for syntax errors from tree-sitter
        if file.has_errors {
            file.walk(|node, _depth| {
                if node.is_error() || node.is_missing() {
                    diagnostics.push(Diagnostic::error(
                        node.to_range(),
                        "PY000",
                        DiagnosticCategory::Syntax,
                        "Syntax error",
                    ));
                }
                true
            });
        }

        // Run all checks
        diagnostics.extend(self.check_unused_imports(file));
        diagnostics.extend(self.check_mutable_default(file));
        diagnostics.extend(self.check_bare_except(file));
        diagnostics.extend(self.check_unreachable_code(file));
        diagnostics.extend(self.check_shadowed_builtins(file));
        // New JetBrains-inspired checks
        diagnostics.extend(self.check_private_member_access(file));
        diagnostics.extend(self.check_duplicate_dict_keys(file));
        diagnostics.extend(self.check_simplify_boolean(file));
        diagnostics.extend(self.check_none_comparison(file));
        diagnostics.extend(self.check_statement_no_effect(file));
        // Scope-based checks
        diagnostics.extend(self.check_scope_issues(file));
        // Pydantic checks
        diagnostics.extend(self.check_pydantic_mutable_default(file));
        diagnostics.extend(self.check_pydantic_deprecated_validator(file));
        diagnostics.extend(self.check_pydantic_deprecated_config(file));
        // Import sorting checks
        diagnostics.extend(self.check_import_sorting(file));
        // Complexity checks
        diagnostics.extend(self.check_function_complexity(file));
        // Security checks (delegated to python_security module)
        diagnostics.extend(super::python_security::check_eval_usage(file));
        diagnostics.extend(super::python_security::check_exec_usage(file));
        diagnostics.extend(super::python_security::check_pickle_usage(file));
        diagnostics.extend(super::python_security::check_subprocess_shell(file));
        let source_lines: Vec<&str> = file.source.lines().collect();
        diagnostics.extend(super::python_security::check_hardcoded_secrets(
            &source_lines,
        ));

        // R2: Run the full type-inference engine (control-flow aware narrowing,
        // TypeVar generic resolution, @overload dispatch, Protocol structural typing,
        // TypeGuard / TypeIs predicates) and surface type errors as diagnostics.
        // The TypeChecker internally uses TypeNarrower to narrow types through
        // isinstance(), is None, TypeGuard, TypeIs branches.
        if file.language == crate::syntax::Language::Python && !file.has_errors {
            let mut type_checker = crate::type_inference::TypeChecker::new(&file.source);
            let type_diags = type_checker.check_file(file);
            diagnostics.extend(type_diags);
        }

        diagnostics
    }

    fn available_rules(&self) -> Vec<&'static str> {
        vec![
            "PY000", // Syntax error
            "PY102", // Unused import
            "PY103", // Unused variable/parameter
            "PY104", // Shadowed builtin
            "PY106", // Variable redeclaration
            "PY201", // Mutable default argument
            "PY202", // Bare except
            "PY203", // Unreachable code
            // JetBrains-inspired
            "PY402", // Private member access
            "PY403", // Duplicate dict keys
            "PY404", // Simplify boolean
            "PY405", // == None instead of is None
            "PY406", // Statement no effect
            // Pydantic
            "PY501", // Mutable default in Pydantic model
            "PY502", // Deprecated @validator/@root_validator
            "PY503", // Deprecated Config class
            // Import sorting (isort-like)
            "PY601", // Imports not sorted
            "PY602", // Import groups not separated
            "PY603", // Duplicate import
            "PY604", // Import should be at top of file
            // Complexity (pylint-like)
            "PY701", // Too many arguments
            "PY702", // Function too long
            // Security
            "PY301", // eval() usage
            "PY302", // exec() usage
            "PY303", // pickle.loads()/pickle.load() usage
            "PY304", // subprocess with shell=True
            "PY305", // Hardcoded secrets
        ]
    }
}
