//! Issue numbers.

/// Why an issue number was rejected.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum IssueNumberError {
    /// Issue numbers start at 1.
    #[error("issue number must be positive")]
    Zero,
}

/// Accepts `number` when it can name an issue.
pub(crate) const fn check_issue_number(number: u64) -> Result<u64, IssueNumberError> {
    if number == 0 {
        Err(IssueNumberError::Zero)
    } else {
        Ok(number)
    }
}
