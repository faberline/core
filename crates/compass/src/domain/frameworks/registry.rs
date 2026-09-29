use crate::domain::cross_file::context::TypeContext;
use crate::domain::frameworks::provider::{FrameworkTypeProvider, MethodType};
use crate::type_inference::Type;

// ============================================================================
// Framework Registry
// ============================================================================

/// Registry of framework type providers.
pub struct FrameworkRegistry {
    /// Registered providers
    providers: Vec<Box<dyn FrameworkTypeProvider + Send + Sync>>,
}

impl FrameworkRegistry {
    /// Create a new registry.
    pub fn new() -> Self {
        Self {
            providers: Vec::new(),
        }
    }

    /// Register a provider.
    pub fn register(&mut self, provider: Box<dyn FrameworkTypeProvider + Send + Sync>) {
        self.providers.push(provider);
    }

    /// Get type from any provider.
    pub fn get_type(&self, symbol: &str, context: &TypeContext) -> Option<Type> {
        for provider in &self.providers {
            if let Some(ty) = provider.get_type(symbol, context) {
                return Some(ty);
            }
        }
        None
    }

    /// Get attribute type from any provider.
    pub fn get_attribute_type(&self, base_type: &Type, attr: &str) -> Option<Type> {
        for provider in &self.providers {
            if let Some(ty) = provider.get_attribute_type(base_type, attr) {
                return Some(ty);
            }
        }
        None
    }

    /// Get method signature from any provider.
    pub fn get_method_signature(&self, base_type: &Type, method: &str) -> Option<MethodType> {
        for provider in &self.providers {
            if let Some(sig) = provider.get_method_signature(base_type, method) {
                return Some(sig);
            }
        }
        None
    }
}

impl Default for FrameworkRegistry {
    fn default() -> Self {
        Self::new()
    }
}
