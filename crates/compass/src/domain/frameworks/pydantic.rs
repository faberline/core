use std::collections::HashMap;

use crate::domain::cross_file::context::TypeContext;
use crate::domain::frameworks::provider::{FrameworkTypeProvider, MethodType};
use crate::type_inference::Type;

// ============================================================================
// Pydantic Support
// ============================================================================

/// Pydantic type provider.
pub struct PydanticTypeProvider {
    /// Registered models
    models: HashMap<String, PydanticModel>,
}

/// Pydantic model.
#[derive(Debug, Clone)]
pub struct PydanticModel {
    /// Model name
    pub name: String,
    /// Fields
    pub fields: HashMap<String, PydanticField>,
    /// Validators
    pub validators: Vec<String>,
    /// Config class
    pub config: Option<PydanticConfig>,
}

/// Pydantic field.
#[derive(Debug, Clone)]
pub struct PydanticField {
    /// Field name
    pub name: String,
    /// Field type
    pub ty: Type,
    /// Default value
    pub default: Option<String>,
    /// Field validators
    pub validators: Vec<String>,
    /// Alias
    pub alias: Option<String>,
}

/// Pydantic config.
#[derive(Debug, Clone)]
pub struct PydanticConfig {
    /// Allow extra fields
    pub extra: PydanticExtra,
    /// Validate assignment
    pub validate_assignment: bool,
    /// Use enum values
    pub use_enum_values: bool,
}

/// Pydantic extra handling.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PydanticExtra {
    Allow,
    Forbid,
    Ignore,
}

impl PydanticTypeProvider {
    /// Create a new provider.
    pub fn new() -> Self {
        Self {
            models: HashMap::new(),
        }
    }

    /// Register a model.
    pub fn register_model(&mut self, model: PydanticModel) {
        self.models.insert(model.name.clone(), model);
    }

    /// Get model info.
    pub fn get_model(&self, name: &str) -> Option<&PydanticModel> {
        self.models.get(name)
    }
}

impl Default for PydanticTypeProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl FrameworkTypeProvider for PydanticTypeProvider {
    fn get_type(&self, symbol: &str, _context: &TypeContext) -> Option<Type> {
        if self.models.contains_key(symbol) {
            Some(Type::Instance {
                name: symbol.to_string(),
                module: None,
                type_args: Vec::new(),
            })
        } else {
            None
        }
    }

    fn get_attribute_type(&self, base_type: &Type, attr: &str) -> Option<Type> {
        if let Type::Instance { name, .. } = base_type {
            if let Some(model) = self.models.get(name) {
                if let Some(field) = model.fields.get(attr) {
                    return Some(field.ty.clone());
                }
            }
        }
        None
    }

    fn get_method_signature(&self, base_type: &Type, method: &str) -> Option<MethodType> {
        if let Type::Instance { name, .. } = base_type {
            // Check if this is a known Pydantic model
            if let Some(_model) = self.models.get(name) {
                return match method {
                    // dict() - convert model to dictionary
                    // Usage: user.dict()
                    "dict" => Some(MethodType {
                        params: vec![],
                        return_type: Type::Dict(Box::new(Type::Str), Box::new(Type::Any)),
                        is_async: false,
                    }),

                    // json() - serialize to JSON string
                    // Usage: user.json()
                    "json" => Some(MethodType {
                        params: vec![],
                        return_type: Type::Str,
                        is_async: false,
                    }),

                    // copy() - create a copy of the model
                    // Usage: new_user = user.copy(update={"name": "New Name"})
                    "copy" => Some(MethodType {
                        params: vec![],
                        return_type: Type::Instance {
                            name: name.clone(),
                            module: None,
                            type_args: Vec::new(),
                        },
                        is_async: false,
                    }),

                    // parse_obj() - parse from dictionary (class method)
                    // Usage: User.parse_obj({"name": "Alice"})
                    "parse_obj" => Some(MethodType {
                        params: vec![(
                            "obj".to_string(),
                            Type::Dict(Box::new(Type::Str), Box::new(Type::Any)),
                        )],
                        return_type: Type::Instance {
                            name: name.clone(),
                            module: None,
                            type_args: Vec::new(),
                        },
                        is_async: false,
                    }),

                    // parse_raw() - parse from JSON string (class method)
                    // Usage: User.parse_raw('{"name": "Alice"}')
                    "parse_raw" => Some(MethodType {
                        params: vec![("b".to_string(), Type::Str)],
                        return_type: Type::Instance {
                            name: name.clone(),
                            module: None,
                            type_args: Vec::new(),
                        },
                        is_async: false,
                    }),

                    // parse_file() - parse from JSON file (class method)
                    // Usage: User.parse_file("user.json")
                    "parse_file" => Some(MethodType {
                        params: vec![("path".to_string(), Type::Str)],
                        return_type: Type::Instance {
                            name: name.clone(),
                            module: None,
                            type_args: Vec::new(),
                        },
                        is_async: false,
                    }),

                    // schema() - get JSON schema (class method)
                    // Usage: User.schema()
                    "schema" => Some(MethodType {
                        params: vec![],
                        return_type: Type::Dict(Box::new(Type::Str), Box::new(Type::Any)),
                        is_async: false,
                    }),

                    // schema_json() - get JSON schema as string (class method)
                    // Usage: User.schema_json()
                    "schema_json" => Some(MethodType {
                        params: vec![],
                        return_type: Type::Str,
                        is_async: false,
                    }),

                    // construct() - construct without validation (class method)
                    // Usage: User.construct(name="Alice")
                    "construct" => Some(MethodType {
                        params: vec![],
                        return_type: Type::Instance {
                            name: name.clone(),
                            module: None,
                            type_args: Vec::new(),
                        },
                        is_async: false,
                    }),

                    // from_orm() - create from ORM model (class method)
                    // Usage: UserResponse.from_orm(db_user)
                    "from_orm" => Some(MethodType {
                        params: vec![("obj".to_string(), Type::Any)],
                        return_type: Type::Instance {
                            name: name.clone(),
                            module: None,
                            type_args: Vec::new(),
                        },
                        is_async: false,
                    }),

                    // validate() - validate data (class method)
                    // Usage: User.validate(data)
                    "validate" => Some(MethodType {
                        params: vec![("value".to_string(), Type::Any)],
                        return_type: Type::Instance {
                            name: name.clone(),
                            module: None,
                            type_args: Vec::new(),
                        },
                        is_async: false,
                    }),

                    // update_forward_refs() - update forward references (class method)
                    // Usage: User.update_forward_refs()
                    "update_forward_refs" => Some(MethodType {
                        params: vec![],
                        return_type: Type::None,
                        is_async: false,
                    }),

                    _ => None,
                };
            }
        }
        None
    }

    fn framework_name(&self) -> &str {
        "Pydantic"
    }
}

#[cfg(test)]
mod tests;
