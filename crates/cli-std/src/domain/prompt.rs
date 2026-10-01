//! The interactive confirmation the state-changing verbs ask for before they
//! act.

/// Asks the operator a yes-or-no question.
pub(crate) trait Confirm {
    /// Show `prompt` and return whether the operator agreed. A session that
    /// cannot ask agrees; callers check `--yes` before they ask.
    fn confirm(&self, prompt: &str) -> Result<bool, PromptError>;
}

/// A confirmation that could not be read.
#[derive(Debug, thiserror::Error)]
pub(crate) enum PromptError {
    /// The answer could not be read from the terminal.
    #[error("read confirmation")]
    Read(#[source] std::io::Error),
}
