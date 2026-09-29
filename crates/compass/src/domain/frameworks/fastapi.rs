use std::collections::HashMap;

use crate::domain::cross_file::context::TypeContext;
use crate::domain::frameworks::provider::{FrameworkTypeProvider, MethodType};
use crate::type_inference::Type;

// ============================================================================
// FastAPI Support
// ============================================================================

/// FastAPI type provider.
pub struct FastAPITypeProvider {
    /// Registered endpoints
    endpoints: HashMap<String, FastAPIEndpoint>,
    /// Dependency types
    dependencies: HashMap<String, Type>,
}

/// FastAPI endpoint.
#[derive(Debug, Clone)]
pub struct FastAPIEndpoint {
    /// Path
    pub path: String,
    /// HTTP methods
    pub methods: Vec<String>,
    /// Request body type
    pub request_body: Option<Type>,
    /// Response type
    pub response_type: Type,
    /// Dependencies
    pub dependencies: Vec<String>,
}

impl FastAPITypeProvider {
    /// Create a new provider.
    pub fn new() -> Self {
        Self {
            endpoints: HashMap::new(),
            dependencies: HashMap::new(),
        }
    }

    /// Register an endpoint.
    pub fn register_endpoint(&mut self, name: String, endpoint: FastAPIEndpoint) {
        self.endpoints.insert(name, endpoint);
    }

    /// Register a dependency.
    pub fn register_dependency(&mut self, name: String, ty: Type) {
        self.dependencies.insert(name, ty);
    }
}

impl Default for FastAPITypeProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl FrameworkTypeProvider for FastAPITypeProvider {
    fn get_type(&self, symbol: &str, _context: &TypeContext) -> Option<Type> {
        self.dependencies.get(symbol).cloned()
    }

    fn get_attribute_type(&self, _base_type: &Type, _attr: &str) -> Option<Type> {
        None
    }

    fn get_method_signature(&self, base_type: &Type, method: &str) -> Option<MethodType> {
        // FastAPI parameter injection helpers
        match method {
            // Depends() - dependency injection
            // Usage: user: User = Depends(get_current_user)
            "Depends" => Some(MethodType {
                params: vec![(
                    "dependency".to_string(),
                    Type::Callable {
                        params: vec![],
                        ret: Box::new(Type::Any),
                    },
                )],
                return_type: Type::Any, // Will be inferred from the dependency function
                is_async: false,
            }),

            // Path() - path parameter
            // Usage: user_id: int = Path(..., gt=0)
            "Path" => Some(MethodType {
                params: vec![("default".to_string(), Type::Any)],
                return_type: Type::Any, // Returns the type specified in annotation
                is_async: false,
            }),

            // Query() - query parameter
            // Usage: skip: int = Query(0, ge=0)
            "Query" => Some(MethodType {
                params: vec![("default".to_string(), Type::Any)],
                return_type: Type::Any, // Returns the type specified in annotation
                is_async: false,
            }),

            // Body() - request body
            // Usage: user: UserCreate = Body(...)
            "Body" => Some(MethodType {
                params: vec![("default".to_string(), Type::Any)],
                return_type: Type::Any, // Returns the type specified in annotation
                is_async: false,
            }),

            // Header() - header parameter
            // Usage: token: str = Header(...)
            "Header" => Some(MethodType {
                params: vec![("default".to_string(), Type::Any)],
                return_type: Type::Any, // Returns the type specified in annotation
                is_async: false,
            }),

            // Cookie() - cookie parameter
            // Usage: session: str = Cookie(None)
            "Cookie" => Some(MethodType {
                params: vec![("default".to_string(), Type::Any)],
                return_type: Type::Any, // Returns the type specified in annotation
                is_async: false,
            }),

            // File() - file upload
            // Usage: file: bytes = File(...)
            "File" => Some(MethodType {
                params: vec![("default".to_string(), Type::Any)],
                return_type: Type::Instance {
                    name: "bytes".to_string(),
                    module: None,
                    type_args: Vec::new(),
                },
                is_async: false,
            }),

            // UploadFile() - uploaded file object
            // Usage: file: UploadFile = File(...)
            "UploadFile" => Some(MethodType {
                params: vec![],
                return_type: Type::Instance {
                    name: "UploadFile".to_string(),
                    module: Some("fastapi".to_string()),
                    type_args: Vec::new(),
                },
                is_async: false,
            }),

            // Form() - form data
            // Usage: username: str = Form(...)
            "Form" => Some(MethodType {
                params: vec![("default".to_string(), Type::Any)],
                return_type: Type::Any, // Returns the type specified in annotation
                is_async: false,
            }),

            // Response models
            "JSONResponse" => Some(MethodType {
                params: vec![("content".to_string(), Type::Any)],
                return_type: Type::Instance {
                    name: "JSONResponse".to_string(),
                    module: Some("fastapi.responses".to_string()),
                    type_args: Vec::new(),
                },
                is_async: false,
            }),

            "HTMLResponse" => Some(MethodType {
                params: vec![("content".to_string(), Type::Str)],
                return_type: Type::Instance {
                    name: "HTMLResponse".to_string(),
                    module: Some("fastapi.responses".to_string()),
                    type_args: Vec::new(),
                },
                is_async: false,
            }),

            "PlainTextResponse" => Some(MethodType {
                params: vec![("content".to_string(), Type::Str)],
                return_type: Type::Instance {
                    name: "PlainTextResponse".to_string(),
                    module: Some("fastapi.responses".to_string()),
                    type_args: Vec::new(),
                },
                is_async: false,
            }),

            "RedirectResponse" => Some(MethodType {
                params: vec![("url".to_string(), Type::Str)],
                return_type: Type::Instance {
                    name: "RedirectResponse".to_string(),
                    module: Some("fastapi.responses".to_string()),
                    type_args: Vec::new(),
                },
                is_async: false,
            }),

            "StreamingResponse" => Some(MethodType {
                params: vec![("content".to_string(), Type::Any)],
                return_type: Type::Instance {
                    name: "StreamingResponse".to_string(),
                    module: Some("fastapi.responses".to_string()),
                    type_args: Vec::new(),
                },
                is_async: false,
            }),

            // FastAPI() app instance methods
            "FastAPI" if matches!(base_type, Type::Instance { name, .. } if name == "FastAPI") => {
                Some(MethodType {
                    params: vec![],
                    return_type: Type::Instance {
                        name: "FastAPI".to_string(),
                        module: Some("fastapi".to_string()),
                        type_args: Vec::new(),
                    },
                    is_async: false,
                })
            }

            _ => None,
        }
    }

    fn framework_name(&self) -> &str {
        "FastAPI"
    }
}

#[cfg(test)]
mod tests;
