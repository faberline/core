/// The operations a generation run keeps, named by their `operationId`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OperationSelection {
    include: Vec<String>,
}

impl OperationSelection {
    /// Build a selection from operation ids. Ids are kept as given, without
    /// duplicates, in first-seen order. An empty list, or an id that is blank
    /// after trimming, is rejected.
    pub fn new(include: Vec<String>) -> Result<Self, OperationSelectionError> {
        if include.is_empty() {
            return Err(OperationSelectionError::Empty);
        }
        let mut kept: Vec<String> = Vec::with_capacity(include.len());
        for id in include {
            if id.trim().is_empty() {
                return Err(OperationSelectionError::Blank);
            }
            if !kept.contains(&id) {
                kept.push(id);
            }
        }
        Ok(Self { include: kept })
    }

    /// The selected operation ids, in first-seen order.
    pub fn include(&self) -> &[String] {
        &self.include
    }
}

/// Why an [`OperationSelection`] or its use on a spec was rejected.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum OperationSelectionError {
    /// The selection names no operation.
    #[error("operation selection is empty: name at least one operationId")]
    Empty,
    /// The selection names an operation id that is blank.
    #[error("operation selection contains a blank operationId")]
    Blank,
    /// The selected id is not an operation of the spec.
    #[error("unknown operationId `{0}`: the spec has no operation with that id")]
    Unknown(String),
}

#[cfg(test)]
mod tests {
    use super::{OperationSelection, OperationSelectionError};

    #[test]
    fn empty_selection_is_rejected() {
        assert_eq!(
            OperationSelection::new(vec![]),
            Err(OperationSelectionError::Empty)
        );
    }

    #[test]
    fn blank_operation_id_is_rejected() {
        assert_eq!(
            OperationSelection::new(vec!["  ".into()]),
            Err(OperationSelectionError::Blank)
        );
    }

    #[test]
    fn duplicate_ids_are_kept_once_in_first_seen_order() {
        let sel = OperationSelection::new(vec!["a".into(), "b".into(), "a".into()]).unwrap();
        assert_eq!(sel.include(), ["a".to_string(), "b".to_string()]);
    }
}
