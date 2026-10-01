use super::*;

#[test]
fn test_default_config() {
    let config = ArgusConfig::new();
    assert!(!config.strict);
    assert!(!config.strict_optional);
    assert!(config.exclude.is_empty());
}

#[test]
fn test_strict_config() {
    let config = ArgusConfig::strict();
    assert!(config.strict);
    assert!(config.strict_optional);
    assert!(config.warn_return_any);
    assert!(config.disallow_untyped_defs);
}

#[test]
fn test_glob_matching() {
    assert!(glob_matches("*.py", "test.py"));
    assert!(glob_matches("tests/*.py", "tests/test_main.py"));
    assert!(glob_matches("tests/**/*.py", "tests/unit/test_main.py"));
    assert!(!glob_matches("*.py", "src/test.txt"));
}

#[test]
fn test_effective_config_with_override() {
    let config = ArgusConfig {
        strict: false,
        overrides: vec![OverrideConfig {
            pattern: "tests/**/*.py".to_string(),
            check_untyped_defs: Some(false),
            ignore_missing_imports: Some(true),
            ..Default::default()
        }],
        ..Default::default()
    };

    let effective = config.effective_for(Path::new("tests/unit/test_main.py"));
    assert!(!effective.check_untyped_defs);
    assert!(effective.ignore_missing_imports);

    let effective_src = config.effective_for(Path::new("src/main.py"));
    assert!(!effective_src.check_untyped_defs); // Default
    assert!(!effective_src.ignore_missing_imports);
}

#[test]
fn test_should_exclude() {
    let config = ArgusConfig {
        exclude: vec!["venv/**".to_string(), "__pycache__/**".to_string()],
        include: vec!["venv/important.py".to_string()],
        ..Default::default()
    };

    assert!(config.should_exclude(Path::new("venv/lib/site-packages/foo.py")));
    assert!(!config.should_exclude(Path::new("venv/important.py"))); // Explicitly included
    assert!(!config.should_exclude(Path::new("src/main.py")));
}

#[test]
fn test_python_env_config_default() {
    let config = PythonEnvConfig::default();
    assert!(config.search_paths.is_empty());
    assert!(config.venv_path.is_none());
    assert!(!config.ignore_site_packages);
}
