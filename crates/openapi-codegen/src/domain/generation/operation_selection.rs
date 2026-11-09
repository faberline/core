/// The operations to generate, by `operationId`, in first-seen order.
///
/// A selection is always non-empty and has no blank ids. Build it with
/// [`OperationSelection::new`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OperationSelection {
    include: Vec<String>,
}

impl OperationSelection {
    /// Select the operations named in `include`. Duplicate ids are dropped
    /// and the first occurrence keeps its place.
    pub fn new(include: Vec<String>) -> Result<Self, OperationSelectionError> {
        if include.is_empty() {
            return Err(OperationSelectionError::Empty);
        }
        if include.iter().any(|id| id.trim().is_empty()) {
            return Err(OperationSelectionError::Blank);
        }
        let mut unique: Vec<String> = Vec::with_capacity(include.len());
        for id in include {
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

/// Why an operation selection was rejected.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum OperationSelectionError {
    /// The selection names no operation.
    #[error("operation selection is empty: name at least one operationId")]
    Empty,
    /// The selection names an empty or blank `operationId`.
    #[error("operation selection contains a blank operationId")]
    Blank,
    /// The spec has no operation with this `operationId`.
    #[error("unknown operationId `{0}`: the spec has no operation with that id")]
    Unknown(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_selection_is_an_error() {
        assert_eq!(
            OperationSelection::new(vec![]),
            Err(OperationSelectionError::Empty)
        );
    }

    #[test]
    fn blank_operation_id_is_an_error() {
        assert_eq!(
            OperationSelection::new(vec!["  ".into()]),
            Err(OperationSelectionError::Blank)
        );
    }

    #[test]
    fn duplicate_ids_are_dropped_and_first_seen_order_is_kept() {
        let selection = OperationSelection::new(vec!["b".into(), "a".into(), "b".into()]).unwrap();
        assert_eq!(selection.include(), ["b", "a"]);
    }
}
