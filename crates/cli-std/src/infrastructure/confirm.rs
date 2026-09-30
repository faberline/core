use crate::domain::prompt::{Confirm, PromptError};

/// The [`Confirm`] port on the process's terminal.
pub(crate) struct TerminalPrompt;

impl Confirm for TerminalPrompt {
    /// Prompt on an interactive terminal; non-interactive sessions return
    /// `true` (callers gate on `--yes` first).
    fn confirm(&self, prompt: &str) -> Result<bool, PromptError> {
        use std::io::{IsTerminal, Write};
        if !std::io::stdin().is_terminal() {
            return Ok(true);
        }
        print!("{prompt} [y/N] ");
        std::io::stdout().flush().ok();
        let mut line = String::new();
        std::io::stdin()
            .read_line(&mut line)
            .map_err(PromptError::Read)?;
        Ok(matches!(line.trim(), "y" | "Y" | "yes" | "Yes"))
    }
}
