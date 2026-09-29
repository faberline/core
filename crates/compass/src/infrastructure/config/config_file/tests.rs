use super::*;
use crate::domain::config::argus_config::{LanguageConfig, LintConfig};

#[test]
fn test_parse_empty_config() {
    let config = ArgusConfig::from_str("").unwrap();
    assert!(config.cclab_lens.python.lint.enabled);
}

#[test]
fn test_parse_full_config() {
    let toml = r#"
[cclab_lens]
languages = ["python", "typescript", "rust"]
lsp_port = 5007

[cclab_lens.python]
target_version = "3.11"
exclude = ["**/migrations/**"]

[cclab_lens.python.lint]
enabled = true
select = ["PY1", "PY2", "PY4", "PY5"]
ignore = ["PY103"]

[cclab_lens.python.isort]
enabled = true
known_first_party = ["cclab_nucleus"]

[cclab_lens.typescript]
enabled = true

[cclab_lens.rust]
enabled = true
"#;

    let config = ArgusConfig::from_str(toml).unwrap();

    assert_eq!(
        config.cclab_lens.languages,
        vec!["python", "typescript", "rust"]
    );
    assert_eq!(config.cclab_lens.lsp_port, 5007);
    assert_eq!(
        config.cclab_lens.python.target_version,
        Some("3.11".to_string())
    );
    assert_eq!(config.cclab_lens.python.exclude, vec!["**/migrations/**"]);
    assert_eq!(
        config.cclab_lens.python.lint.select,
        vec!["PY1", "PY2", "PY4", "PY5"]
    );
    assert_eq!(config.cclab_lens.python.lint.ignore, vec!["PY103"]);
    assert_eq!(
        config.cclab_lens.python.isort.known_first_party,
        vec!["cclab_nucleus"]
    );
    assert!(config.cclab_lens.typescript.enabled);
    assert!(config.cclab_lens.rust.enabled);
}

#[test]
fn test_language_config_rule_filtering() {
    let lint = LintConfig {
        enabled: true,
        select: vec!["PY1".to_string(), "PY2".to_string()],
        ignore: vec!["PY103".to_string()],
    };

    let config = LanguageConfig::from(&lint);

    // PY103 is explicitly ignored
    assert!(!config.is_rule_enabled("PY103"));

    // PY101 matches PY1 prefix
    assert!(config.is_rule_enabled("PY101"));

    // PY201 matches PY2 prefix
    assert!(config.is_rule_enabled("PY201"));

    // PY401 doesn't match any prefix
    assert!(!config.is_rule_enabled("PY401"));
}

#[test]
fn test_language_config_empty_select() {
    let lint = LintConfig {
        enabled: true,
        select: vec![],
        ignore: vec!["PY103".to_string()],
    };

    let config = LanguageConfig::from(&lint);

    // Empty select means all rules enabled (except ignored)
    assert!(config.is_rule_enabled("PY101"));
    assert!(config.is_rule_enabled("PY401"));
    assert!(!config.is_rule_enabled("PY103"));
}
