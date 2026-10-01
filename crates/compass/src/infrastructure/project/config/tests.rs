use super::*;

#[test]
fn test_project_config_default() {
    let config = ProjectConfig::new(PathBuf::from("/project"));

    assert_eq!(config.root, PathBuf::from("/project"));
    assert!(!config.strict);
    assert_eq!(config.type_checking_mode, "standard");
}

#[test]
fn test_should_exclude() {
    let config = ProjectConfig::new(PathBuf::from("/project"));

    assert!(config.should_exclude(Path::new("/project/venv/lib")));
    assert!(config.should_exclude(Path::new("/project/__pycache__/file.pyc")));
    assert!(config.should_exclude(Path::new("/project/.git/config")));
    assert!(!config.should_exclude(Path::new("/project/src/main.py")));
}

#[test]
fn test_parse_pyproject() {
    let content = r#"
[tool.cclab_lens]
python_version = "3.11"
strict = true
type_checking_mode = "strict"

[tool.other]
key = "value"
"#;

    let mut config = ProjectConfig::new(PathBuf::from("/project"));
    config.parse_pyproject(content);

    assert_eq!(config.python_version, Some("3.11".to_string()));
    assert!(config.strict);
    assert_eq!(config.type_checking_mode, "strict");
}

#[test]
fn test_pyright_config_parsing() {
    let content = r#"
[tool.pyright]
pythonVersion = "3.10"
strict = true
typeCheckingMode = "strict"
"#;

    let mut config = ProjectConfig::new(PathBuf::from("/project"));
    config.parse_pyproject(content);

    // Should parse pyright config too
    assert_eq!(config.python_version, Some("3.10".to_string()));
    assert_eq!(config.type_checking_mode, "strict");
}
