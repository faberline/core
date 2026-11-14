/// The operations a caller asks to generate, named by `operationId`.
///
/// Build it with [`OperationSelection::new`]. The ids are kept as given,
/// de-duplicated, in first-seen order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OperationSelection {
    include: Vec<String>,
}

impl OperationSelection {
    /// Select the operations with these `operationId`s.
    ///
    /// Fails with [`OperationSelectionError::Empty`] for an empty list and
    /// with [`OperationSelectionError::Blank`] when an id is empty after
    /// trimming whitespace.
    pub fn new(include: Vec<String>) -> Result<Self, OperationSelectionError> {
        if include.is_empty() {
            return Err(OperationSelectionError::Empty);
        }
        let mut unique: Vec<String> = Vec::with_capacity(include.len());
        for id in include {
            if id.trim().is_empty() {
                return Err(OperationSelectionError::Blank);
            }
            if !unique.contains(&id) {
                unique.push(id);
            }
        }
        Ok(Self { include: unique })
    }

    /// The selected `operationId`s, in first-seen order.
    pub fn include(&self) -> &[String] {
        &self.include
    }
}

/// Why [`OperationSelection::new`] rejected its arguments, or why a spec does
/// not contain a selected operation.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum OperationSelectionError {
    /// The selection names no operation.
    #[error("operation selection is empty: name at least one operationId")]
    Empty,
    /// An `operationId` is empty or only whitespace.
    #[error("operation selection contains a blank operationId")]
    Blank,
    /// A selected `operationId` is not an operation of the spec.
    #[error("unknown operationId `{0}`: the spec has no operation with that id")]
    Unknown(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_selection_is_rejected() {
        assert_eq!(
            OperationSelection::new(Vec::new()),
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
        let sel = OperationSelection::new(vec!["a".into(), "a".into()]).unwrap();
        assert_eq!(sel.include(), ["a"]);

        let sel = OperationSelection::new(vec!["b".into(), "a".into(), "b".into()]).unwrap();
        assert_eq!(sel.include(), ["b", "a"]);
    }
}
