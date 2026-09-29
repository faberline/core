use super::*;
use std::path::PathBuf;

use crate::domain::type_codegen::request::{DocstringStyle, TestFramework};

#[test]
fn test_docstring_generation() {
    let gen = CodeGenerator::new();
    let request = CodeGenRequest {
        kind: CodeGenKind::Docstring {
            style: DocstringStyle::Google,
        },
        file: PathBuf::from("test.py"),
        symbol: "my_function".to_string(),
        options: CodeGenOptions::default(),
    };

    let result = gen.generate(&request);
    assert!(result.code.contains("my_function"));
    assert!(result.code.contains("Args:"));
}

#[test]
fn test_pytest_generation() {
    let gen = CodeGenerator::new();
    let request = CodeGenRequest {
        kind: CodeGenKind::TestStub {
            framework: TestFramework::Pytest,
        },
        file: PathBuf::from("test.py"),
        symbol: "MyClass".to_string(),
        options: CodeGenOptions::default(),
    };

    let result = gen.generate(&request);
    assert!(result.code.contains("class TestMyClass"));
    assert!(result.imports.contains(&"import pytest".to_string()));
}

#[test]
fn test_type_stub_generation() {
    let gen = CodeGenerator::new();
    let request = CodeGenRequest {
        kind: CodeGenKind::TypeStub,
        file: PathBuf::from("module.py"),
        symbol: "my_func".to_string(),
        options: CodeGenOptions::default(),
    };

    let result = gen.generate(&request);
    assert!(result.target_file.extension().unwrap() == "pyi");
}

#[test]
fn test_module_stub_generation() {
    use crate::type_inference::{Param, ParamKind, Type, TypeBinding, TypeContext};

    // Create a CodeGenerator with a pre-populated TypeContext
    let mut type_context = TypeContext::new();
    let file = PathBuf::from("mymodule.py");

    // Add a function binding
    let binding = TypeBinding {
        ty: Type::Callable {
            params: vec![Param {
                name: "x".to_string(),
                ty: Type::Int,
                has_default: false,
                kind: ParamKind::Positional,
            }],
            ret: Box::new(Type::Str),
        },
        source_file: file.clone(),
        symbol: "my_function".to_string(),
        line: 1,
        is_exported: true,
        dependencies: vec![],
        is_propagated: false,
    };
    type_context.add_binding(file.clone(), binding);

    let gen = CodeGenerator {
        type_context,
        default_options: CodeGenOptions::default(),
    };

    let request = CodeGenRequest {
        kind: CodeGenKind::ModuleStub,
        file: file.clone(),
        symbol: "my_function".to_string(),
        options: CodeGenOptions::default(),
    };

    let result = gen.generate(&request);

    // Verify .pyi extension
    assert_eq!(result.target_file.extension().unwrap(), "pyi");
    assert_eq!(result.target_file.file_stem().unwrap(), "mymodule");

    // Verify stub contains imports
    assert!(result.code.contains("from typing import Any"));

    // Verify stub contains __all__ list
    assert!(result.code.contains("__all__ = [\"my_function\"]"));

    // Verify function stub
    assert!(result.code.contains("def my_function(x: int) -> str: ..."));
}
