/// Assemble the issue body: message first (when non-empty), separator, then the
/// diagnostics block.
pub fn assemble_body(message: Option<&str>, diagnostics: &str) -> String {
    match message {
        Some(m) if !m.trim().is_empty() => format!("{}\n\n---\n{diagnostics}", m.trim()),
        _ => diagnostics.to_string(),
    }
}
