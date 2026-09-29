use super::*;
use std::env;

#[test]
fn test_extract_imports() {
    let analyzer = ProjectAnalyzer::new(ProjectConfig::new(PathBuf::from("/test"))).unwrap();

    let source = r#"
import os
import sys, json
from pathlib import Path
from typing import Optional, List
import numpy as np
"#;

    let imports = analyzer.extract_imports(source);

    assert!(imports.contains(&"os".to_string()));
    assert!(imports.contains(&"sys".to_string()));
    assert!(imports.contains(&"json".to_string()));
    assert!(imports.contains(&"pathlib".to_string()));
    assert!(imports.contains(&"typing".to_string()));
    assert!(imports.contains(&"numpy".to_string()));
}

#[test]
fn test_project_analyzer_creation() {
    let config = ProjectConfig::new(PathBuf::from("/test"));
    let analyzer = ProjectAnalyzer::new(config).unwrap();

    assert!(analyzer.errors.is_empty());
}

#[test]
fn test_discover_files_in_temp() {
    // Create a temp directory structure
    let temp_dir = env::temp_dir().join("cclab_lens_test_project");
    let _ = fs::remove_dir_all(&temp_dir); // Clean up if exists
    fs::create_dir_all(temp_dir.join("src")).unwrap();
    fs::create_dir_all(temp_dir.join("venv")).unwrap();

    // Create some Python files
    fs::write(temp_dir.join("src/main.py"), "print('hello')").unwrap();
    fs::write(temp_dir.join("src/utils.py"), "def foo(): pass").unwrap();
    fs::write(temp_dir.join("venv/lib.py"), "# should be excluded").unwrap();

    let config = ProjectConfig::new(temp_dir.clone());
    let analyzer = ProjectAnalyzer::new(config).unwrap();

    let files = analyzer.discover_files();

    // Should find main.py and utils.py, but not venv/lib.py
    assert!(files.iter().any(|p| p.ends_with("main.py")));
    assert!(files.iter().any(|p| p.ends_with("utils.py")));
    assert!(!files.iter().any(|p| p.to_string_lossy().contains("venv")));

    // Cleanup
    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_build_graph_with_imports() {
    // Create a temp directory with files that import each other
    let temp_dir = env::temp_dir().join("cclab_lens_test_graph");
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).unwrap();

    // main.py imports utils
    fs::write(
        temp_dir.join("main.py"),
        "from utils import helper\n\ndef main():\n    helper()",
    )
    .unwrap();

    // utils.py is standalone
    fs::write(temp_dir.join("utils.py"), "def helper(): pass").unwrap();

    let config = ProjectConfig::new(temp_dir.clone());
    let mut analyzer = ProjectAnalyzer::new(config).unwrap();
    analyzer.build_graph();

    let graph = analyzer.graph();

    // Check that main imports utils
    let main = graph.get_module("main");
    assert!(main.is_some());
    assert!(main.unwrap().imports.contains("utils"));

    // Cleanup
    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_circular_import_detection() {
    let temp_dir = env::temp_dir().join("cclab_lens_test_circular");
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).unwrap();

    // Create circular imports: a -> b -> c -> a
    fs::write(temp_dir.join("a.py"), "from b import foo").unwrap();
    fs::write(temp_dir.join("b.py"), "from c import bar").unwrap();
    fs::write(temp_dir.join("c.py"), "from a import baz").unwrap();

    let config = ProjectConfig::new(temp_dir.clone());
    let mut analyzer = ProjectAnalyzer::new(config).unwrap();
    analyzer.build_graph();

    let cycles = analyzer.circular_imports();
    assert!(!cycles.is_empty(), "Should detect circular imports");

    // Cleanup
    let _ = fs::remove_dir_all(&temp_dir);
}
