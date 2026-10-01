use crate::diagnostic::Range;
use crate::domain::type_system::ty::Type;

/// Type error information
#[derive(Debug, Clone)]
pub struct TypeError {
    pub range: Range,
    pub expected: Type,
    pub got: Type,
    pub message: String,
}
