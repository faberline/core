//! REST API IR (from OpenAPI).

use super::data_model::{DataModelSpec, ParamDef};
use crate::type_inference::Type;

// ============================================================================
// REST API Specification (from OpenAPI)
// ============================================================================

/// REST API specification
#[derive(Debug, Clone, Default)]
pub struct RestApiSpec {
    /// API title
    pub title: String,
    /// API version
    pub version: String,
    /// API description
    pub description: Option<String>,
    /// Base URL/server
    pub servers: Vec<ServerDef>,
    /// API endpoints
    pub endpoints: Vec<EndpointDef>,
    /// Shared schemas (referenced by endpoints)
    pub schemas: DataModelSpec,
    /// Security schemes
    pub security_schemes: Vec<SecurityScheme>,
}

/// Server definition
#[derive(Debug, Clone)]
pub struct ServerDef {
    pub url: String,
    pub description: Option<String>,
}

/// API endpoint definition
#[derive(Debug, Clone)]
pub struct EndpointDef {
    /// HTTP path (e.g., "/users/{id}")
    pub path: String,
    /// HTTP method
    pub method: HttpMethod,
    /// Operation ID (for function naming)
    pub operation_id: Option<String>,
    /// Summary
    pub summary: Option<String>,
    /// Description
    pub description: Option<String>,
    /// Tags for grouping
    pub tags: Vec<String>,
    /// Path parameters
    pub path_params: Vec<ParamDef>,
    /// Query parameters
    pub query_params: Vec<QueryParam>,
    /// Request body
    pub request_body: Option<RequestBody>,
    /// Responses
    pub responses: Vec<ResponseDef>,
    /// Security requirements
    pub security: Vec<String>,
    /// Whether endpoint is deprecated
    pub deprecated: bool,
}

/// HTTP methods
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Patch,
    Delete,
    Head,
    Options,
}

impl std::fmt::Display for HttpMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HttpMethod::Get => write!(f, "GET"),
            HttpMethod::Post => write!(f, "POST"),
            HttpMethod::Put => write!(f, "PUT"),
            HttpMethod::Patch => write!(f, "PATCH"),
            HttpMethod::Delete => write!(f, "DELETE"),
            HttpMethod::Head => write!(f, "HEAD"),
            HttpMethod::Options => write!(f, "OPTIONS"),
        }
    }
}

/// Query parameter
#[derive(Debug, Clone)]
pub struct QueryParam {
    pub name: String,
    pub ty: Type,
    pub required: bool,
    pub description: Option<String>,
    pub default: Option<String>,
}

/// Request body definition
#[derive(Debug, Clone)]
pub struct RequestBody {
    /// Content type (e.g., "application/json")
    pub content_type: String,
    /// Body schema (model name or inline type)
    pub schema: Type,
    /// Whether body is required
    pub required: bool,
    /// Description
    pub description: Option<String>,
}

/// Response definition
#[derive(Debug, Clone)]
pub struct ResponseDef {
    /// HTTP status code
    pub status_code: u16,
    /// Description
    pub description: String,
    /// Response body schema
    pub schema: Option<Type>,
    /// Content type
    pub content_type: Option<String>,
}

/// Security scheme
#[derive(Debug, Clone)]
pub struct SecurityScheme {
    pub name: String,
    pub scheme_type: SecuritySchemeType,
    pub description: Option<String>,
}

/// Security scheme types
#[derive(Debug, Clone)]
pub enum SecuritySchemeType {
    ApiKey {
        in_header: bool,
        key_name: String,
    },
    Http {
        scheme: String,
        bearer_format: Option<String>,
    },
    OAuth2 {
        flows: Vec<String>,
    },
    OpenIdConnect {
        url: String,
    },
}

impl RestApiSpec {
    pub fn new(title: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            version: version.into(),
            ..Default::default()
        }
    }
}
