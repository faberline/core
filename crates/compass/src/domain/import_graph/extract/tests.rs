use super::*;

#[test]
fn test_extract_python_imports() {
    let src = "import os\nfrom collections import OrderedDict\nfrom .utils import helper\nfrom ..base import Base\n";
    let imps = extract_python_imports(src);
    assert_eq!(imps.len(), 4);
    assert_eq!(imps[0].path, "os");
    assert_eq!(imps[1].path, "collections");
    assert_eq!(imps[2].path, ".utils");
    assert_eq!(imps[3].path, "..base");
}

#[test]
fn test_extract_js_imports() {
    let src =
        "import React from 'react';\nimport foo from './foo';\nconst baz = require('./baz');\n";
    let imps = extract_js_imports(src);
    assert_eq!(imps.len(), 2);
    assert_eq!(imps[0].path, "./foo");
    assert_eq!(imps[1].path, "./baz");
}

#[test]
fn test_extract_rust_imports() {
    let src = "mod utils;\npub mod config;\nuse crate::server::handler;\nuse super::types;\n";
    let imps = extract_rust_imports(src);
    assert_eq!(imps.len(), 4);
    assert_eq!(imps[0].path, "mod:utils");
    assert_eq!(imps[1].path, "mod:config");
    assert_eq!(imps[2].path, "crate::server::handler");
    assert_eq!(imps[3].path, "super::types");
}

#[test]
fn test_extract_go_imports() {
    let src = "package main\n\nimport \"fmt\"\n\nimport (\n    \"os\"\n    myalias \"github.com/user/pkg\"\n)\n";
    let imps = extract_go_imports(src);
    assert_eq!(imps.len(), 3);
    assert_eq!(imps[0].path, "fmt");
    assert_eq!(imps[1].path, "os");
    assert_eq!(imps[2].path, "github.com/user/pkg");
}

#[test]
fn test_extract_imports_whitespace_is_ascii() {
    // A no-break or ideographic space does not separate an import keyword.
    let py = extract_python_imports("import\u{a0}os\nfrom\u{3000}x import y\nimport sys\n");
    assert_eq!(py.len(), 1);
    assert_eq!(py[0].path, "sys");
    let go = extract_go_imports("import (\n\tf\u{e9}\u{e9} \"fmt\"\n\tbar \"os\"\n)\n");
    assert_eq!(go.len(), 1);
    assert_eq!(go[0].path, "os");
}
