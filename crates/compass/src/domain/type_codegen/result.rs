use std::path::PathBuf;

// ============================================================================
// Code Generation Result
// ============================================================================

/// Result of code generation.
#[derive(Debug, Clone)]
pub struct CodeGenResult {
    /// Generated code
    pub code: String,
    /// File to insert into
    pub target_file: PathBuf,
    /// Position to insert (line number)
    pub insert_line: usize,
    /// Imports needed
    pub imports: Vec<String>,
}

impl CodeGenResult {
    /// Create a new result.
    pub fn new(code: impl Into<String>, target_file: PathBuf) -> Self {
        Self {
            code: code.into(),
            target_file,
            insert_line: 0,
            imports: Vec::new(),
        }
    }

    /// Set insertion line.
    pub fn at_line(mut self, line: usize) -> Self {
        self.insert_line = line;
        self
    }

    /// Add required import.
    pub fn with_import(mut self, import: impl Into<String>) -> Self {
        self.imports.push(import.into());
        self
    }
}
