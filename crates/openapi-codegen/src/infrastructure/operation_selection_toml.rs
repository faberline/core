use anyhow::{Context, Result};
use serde::Deserialize;

use crate::domain::OperationSelection;

impl OperationSelection {
    /// Parse the `[operations]` table of a project `codegen.toml`.
    ///
    /// ```toml
    /// [operations]
    /// include = ["getTask", "listTasks"]
    /// ```
    ///
    /// Returns `Ok(None)` when the file has no `[operations]` table. Other
    /// tables, such as `[targets]`, are ignored.
    pub fn from_toml(source: &str) -> Result<Option<Self>> {
        let raw: RawSelectionFile =
            toml::from_str(source).context("parse codegen operation selection")?;
        match raw.operations {
            None => Ok(None),
            Some(table) => Ok(Some(
                Self::new(table.include).context("operations.include")?,
            )),
        }
    }
}

#[derive(Deserialize)]
struct RawSelectionFile {
    operations: Option<RawOperations>,
}

#[derive(Deserialize)]
struct RawOperations {
    include: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    const TARGETS: &str = "[targets]\ntypescript = \"typescript-5.0\"\npython = \"python-3.14\"\nrust = \"rust-2024\"\n";

    #[test]
    fn include_list_parses() {
        let sel = OperationSelection::from_toml("[operations]\ninclude = [\"getTask\"]\n")
            .unwrap()
            .expect("operations table present");
        assert_eq!(sel.include(), ["getTask"]);
    }

    #[test]
    fn no_operations_table_is_none() {
        assert!(OperationSelection::from_toml(TARGETS).unwrap().is_none());
    }

    #[test]
    fn empty_include_is_an_error() {
        assert!(OperationSelection::from_toml("[operations]\ninclude = []\n").is_err());
    }

    #[test]
    fn targets_and_operations_can_share_a_file() {
        let source = format!("{TARGETS}[operations]\ninclude = [\"a\", \"b\"]\n");
        let sel = OperationSelection::from_toml(&source)
            .unwrap()
            .expect("operations table present");
        assert_eq!(sel.include(), ["a", "b"]);
    }
}
