use super::*;

#[test]
fn test_parse_pyproject_toml() {
    let toml_content = r#"
[tool.cclab_lens]
python_version = "3.10"
strict = true
exclude = ["venv/**", "__pycache__/**"]

[[tool.cclab_lens.overrides]]
pattern = "tests/**/*.py"
check_untyped_defs = false
"#;

    let pyproject: PyProject = toml::from_str(toml_content).unwrap();
    let config = pyproject.tool.unwrap().cclab_lens.unwrap();

    assert_eq!(config.python_version, Some("3.10".to_string()));
    assert!(config.strict);
    assert_eq!(config.exclude.len(), 2);
    assert_eq!(config.overrides.len(), 1);
    assert_eq!(config.overrides[0].pattern, "tests/**/*.py");
}

#[test]
fn test_parse_python_env_config() {
    let toml_content = r#"
[tool.cclab_lens]
python_version = "3.11"

[tool.cclab_lens.python]
search_paths = ["./lib", "./src"]
venv_path = "./custom_env"
ignore_site_packages = true
"#;

    let pyproject: PyProject = toml::from_str(toml_content).unwrap();
    let config = pyproject.tool.unwrap().cclab_lens.unwrap();

    assert_eq!(config.python_version, Some("3.11".to_string()));
    assert_eq!(config.python.search_paths.len(), 2);
    assert_eq!(config.python.search_paths[0], PathBuf::from("./lib"));
    assert_eq!(config.python.search_paths[1], PathBuf::from("./src"));
    assert_eq!(config.python.venv_path, Some(PathBuf::from("./custom_env")));
    assert!(config.python.ignore_site_packages);
}

#[test]
fn test_parse_python_env_config_partial() {
    let toml_content = r#"
[tool.cclab_lens]
python_version = "3.10"

[tool.cclab_lens.python]
venv_path = ".venv"
"#;

    let pyproject: PyProject = toml::from_str(toml_content).unwrap();
    let config = pyproject.tool.unwrap().cclab_lens.unwrap();

    assert!(config.python.search_paths.is_empty());
    assert_eq!(config.python.venv_path, Some(PathBuf::from(".venv")));
    assert!(!config.python.ignore_site_packages);
}
