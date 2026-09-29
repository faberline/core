//! OpenAPI 3.x to RestApiSpec parser
//!
//! Parses OpenAPI 3.0.x and 3.1.x specifications.

use crate::domain::spec::ir::{RestApiSpec, ServerDef};
use serde_json::Value;
use std::collections::HashMap;

mod operation;
mod schema;
mod security;

/// Error type for OpenAPI parsing
#[derive(Debug)]
pub enum OpenApiError {
    /// Invalid JSON/YAML
    ParseError(String),
    /// Missing required field
    MissingField(String),
    /// Unsupported OpenAPI version
    UnsupportedVersion(String),
    /// Invalid reference
    InvalidRef(String),
    /// Other error
    Other(String),
}

impl std::fmt::Display for OpenApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OpenApiError::ParseError(s) => write!(f, "parse error: {}", s),
            OpenApiError::MissingField(s) => write!(f, "missing field: {}", s),
            OpenApiError::UnsupportedVersion(s) => write!(f, "unsupported version: {}", s),
            OpenApiError::InvalidRef(s) => write!(f, "invalid reference: {}", s),
            OpenApiError::Other(s) => write!(f, "{}", s),
        }
    }
}

impl std::error::Error for OpenApiError {}

/// OpenAPI parser
pub struct OpenApiParser {
    /// Parsed components for reference resolution
    components: HashMap<String, Value>,
}

impl OpenApiParser {
    pub fn new() -> Self {
        Self {
            components: HashMap::new(),
        }
    }

    /// Parse OpenAPI from JSON string
    pub fn parse_json(&mut self, json: &str) -> Result<RestApiSpec, OpenApiError> {
        let value: Value =
            serde_json::from_str(json).map_err(|e| OpenApiError::ParseError(e.to_string()))?;
        self.parse_value(&value)
    }

    /// Parse OpenAPI from YAML string
    pub fn parse_yaml(&mut self, yaml: &str) -> Result<RestApiSpec, OpenApiError> {
        let value: Value =
            serde_yaml::from_str(yaml).map_err(|e| OpenApiError::ParseError(e.to_string()))?;
        self.parse_value(&value)
    }

    /// Parse OpenAPI from Value
    pub fn parse_value(&mut self, value: &Value) -> Result<RestApiSpec, OpenApiError> {
        // Check version
        let version = value
            .get("openapi")
            .and_then(|v| v.as_str())
            .ok_or_else(|| OpenApiError::MissingField("openapi".into()))?;

        if !version.starts_with("3.") {
            return Err(OpenApiError::UnsupportedVersion(version.to_string()));
        }

        // Extract components for reference resolution
        self.extract_components(value);

        // Parse info
        let info = value
            .get("info")
            .ok_or_else(|| OpenApiError::MissingField("info".into()))?;

        let title = info
            .get("title")
            .and_then(|v| v.as_str())
            .ok_or_else(|| OpenApiError::MissingField("info.title".into()))?;

        let api_version = info
            .get("version")
            .and_then(|v| v.as_str())
            .ok_or_else(|| OpenApiError::MissingField("info.version".into()))?;

        let mut spec = RestApiSpec::new(title, api_version);
        spec.description = info
            .get("description")
            .and_then(|v| v.as_str())
            .map(String::from);

        // Parse servers
        if let Some(servers) = value.get("servers").and_then(|v| v.as_array()) {
            for server in servers {
                if let Some(url) = server.get("url").and_then(|v| v.as_str()) {
                    spec.servers.push(ServerDef {
                        url: url.to_string(),
                        description: server
                            .get("description")
                            .and_then(|v| v.as_str())
                            .map(String::from),
                    });
                }
            }
        }

        // Parse paths (endpoints)
        if let Some(paths) = value.get("paths").and_then(|v| v.as_object()) {
            for (path, path_item) in paths {
                self.parse_path_item(path, path_item, &mut spec)?;
            }
        }

        // Parse component schemas
        spec.schemas = self.parse_component_schemas()?;

        // Parse security schemes
        if let Some(sec_schemes) = value
            .pointer("/components/securitySchemes")
            .and_then(|v| v.as_object())
        {
            for (name, scheme) in sec_schemes {
                if let Some(parsed) = self.parse_security_scheme(name, scheme) {
                    spec.security_schemes.push(parsed);
                }
            }
        }

        Ok(spec)
    }

    /// Extract components for reference resolution
    fn extract_components(&mut self, value: &Value) {
        if let Some(components) = value.get("components").and_then(|v| v.as_object()) {
            if let Some(schemas) = components.get("schemas").and_then(|v| v.as_object()) {
                for (name, schema) in schemas {
                    self.components
                        .insert(format!("#/components/schemas/{}", name), schema.clone());
                }
            }
        }
    }
}

impl Default for OpenApiParser {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests;
