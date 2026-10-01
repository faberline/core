use std::path::Path;

use crate::domain::lint::custom::{CustomLintEngine, CustomRulesFile};

impl CustomLintEngine {
    /// Load rules from an explicit file path.
    pub fn load_from_path(path: &Path) -> std::io::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let rules_file: CustomRulesFile = toml::from_str(&content)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        Ok(Self::from_rules_file(&rules_file))
    }

    /// Convenience loader: reads from `{workspace_root}/cclab/.index/rules.toml`.
    ///
    /// Returns `None` if the file does not exist or cannot be parsed (errors
    /// are logged at `WARN` level so CI pipelines can still proceed).
    pub fn load_from_workspace(workspace_root: &Path) -> Option<Self> {
        let rules_path = workspace_root
            .join("cclab")
            .join(".index")
            .join("rules.toml");

        if !rules_path.exists() {
            return None;
        }

        match Self::load_from_path(&rules_path) {
            Ok(engine) => Some(engine),
            Err(e) => {
                tracing::warn!("Failed to load custom rules from {:?}: {}", rules_path, e);
                None
            }
        }
    }
}
