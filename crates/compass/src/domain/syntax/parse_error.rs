/// Information about a parse error
#[derive(Debug, Clone)]
pub struct ParseError {
    /// Start byte offset of the error
    pub start_byte: usize,
    /// End byte offset of the error
    pub end_byte: usize,
    /// Start position (line, column)
    pub start_position: (usize, usize),
    /// End position (line, column)
    pub end_position: (usize, usize),
    /// The error node kind (usually "ERROR")
    pub kind: String,
}
