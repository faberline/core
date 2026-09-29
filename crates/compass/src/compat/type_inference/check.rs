//! Type checking - verifies type compatibility and generates diagnostics

pub use crate::domain::semantic_model::builder::{build_semantic_model, SemanticModelBuilder};
pub use crate::domain::type_checking::checker::TypeChecker;
pub use crate::domain::type_checking::type_error::TypeError;
