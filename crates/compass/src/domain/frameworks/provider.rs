use crate::domain::cross_file::context::TypeContext;
use crate::type_inference::Type;

// ============================================================================
// Framework-Specific Type Providers
// ============================================================================

/// Provides framework-specific type information.
pub trait FrameworkTypeProvider {
    /// Get types for a symbol.
    fn get_type(&self, symbol: &str, context: &TypeContext) -> Option<Type>;

    /// Get attribute types for an object.
    fn get_attribute_type(&self, base_type: &Type, attr: &str) -> Option<Type>;

    /// Get method signatures.
    fn get_method_signature(&self, base_type: &Type, method: &str) -> Option<MethodType>;

    /// Framework name.
    fn framework_name(&self) -> &str;
}

/// Method type with parameters and return.
#[derive(Debug, Clone)]
pub struct MethodType {
    /// Parameter types
    pub params: Vec<(String, Type)>,
    /// Return type
    pub return_type: Type,
    /// Is async
    pub is_async: bool,
}
