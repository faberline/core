//! AsyncAPI 2.x/3.x to EventApiSpec parser
//!
//! Parses AsyncAPI specifications for event-driven APIs.

use crate::domain::spec::ir::EventApiSpec;
use serde_json::Value;
use std::collections::HashMap;

mod channel;
mod schema;

/// Error type for AsyncAPI parsing
#[derive(Debug)]
pub enum AsyncApiError {
    /// Invalid JSON/YAML
    ParseError(String),
    /// Missing required field
    MissingField(String),
    /// Unsupported AsyncAPI version
    UnsupportedVersion(String),
    /// Invalid reference
    InvalidRef(String),
    /// Other error
    Other(String),
}

impl std::fmt::Display for AsyncApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AsyncApiError::ParseError(s) => write!(f, "parse error: {}", s),
            AsyncApiError::MissingField(s) => write!(f, "missing field: {}", s),
            AsyncApiError::UnsupportedVersion(s) => write!(f, "unsupported version: {}", s),
            AsyncApiError::InvalidRef(s) => write!(f, "invalid reference: {}", s),
            AsyncApiError::Other(s) => write!(f, "{}", s),
        }
    }
}

impl std::error::Error for AsyncApiError {}

/// AsyncAPI parser
pub struct AsyncApiParser {
    /// Parsed components for reference resolution
    components: HashMap<String, Value>,
    /// AsyncAPI version (2.x or 3.x)
    version_major: u8,
}

impl AsyncApiParser {
    pub fn new() -> Self {
        Self {
            components: HashMap::new(),
            version_major: 2,
        }
    }

    /// Parse AsyncAPI from JSON string
    pub fn parse_json(&mut self, json: &str) -> Result<EventApiSpec, AsyncApiError> {
        let value: Value =
            serde_json::from_str(json).map_err(|e| AsyncApiError::ParseError(e.to_string()))?;
        self.parse_value(&value)
    }

    /// Parse AsyncAPI from YAML string
    pub fn parse_yaml(&mut self, yaml: &str) -> Result<EventApiSpec, AsyncApiError> {
        let value: Value =
            serde_yaml::from_str(yaml).map_err(|e| AsyncApiError::ParseError(e.to_string()))?;
        self.parse_value(&value)
    }

    /// Parse AsyncAPI from Value
    pub fn parse_value(&mut self, value: &Value) -> Result<EventApiSpec, AsyncApiError> {
        // Check version
        let version = value
            .get("asyncapi")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AsyncApiError::MissingField("asyncapi".into()))?;

        self.version_major = if version.starts_with("3.") {
            3
        } else if version.starts_with("2.") {
            2
        } else {
            return Err(AsyncApiError::UnsupportedVersion(version.to_string()));
        };

        // Extract components for reference resolution
        self.extract_components(value);

        // Parse info
        let info = value
            .get("info")
            .ok_or_else(|| AsyncApiError::MissingField("info".into()))?;

        let title = info
            .get("title")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AsyncApiError::MissingField("info.title".into()))?;

        let api_version = info
            .get("version")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AsyncApiError::MissingField("info.version".into()))?;

        let description = info
            .get("description")
            .and_then(|v| v.as_str())
            .map(String::from);

        // Parse channels
        let channels = self.parse_channels(value)?;

        // Parse message schemas
        let messages = self.parse_message_schemas(value)?;

        Ok(EventApiSpec {
            title: title.to_string(),
            version: api_version.to_string(),
            description,
            channels,
            messages,
        })
    }

    /// Extract components for reference resolution
    fn extract_components(&mut self, value: &Value) {
        if let Some(components) = value.get("components") {
            // Schemas
            if let Some(schemas) = components.get("schemas").and_then(|s| s.as_object()) {
                for (name, schema) in schemas {
                    self.components
                        .insert(format!("#/components/schemas/{}", name), schema.clone());
                }
            }
            // Messages
            if let Some(messages) = components.get("messages").and_then(|m| m.as_object()) {
                for (name, message) in messages {
                    self.components
                        .insert(format!("#/components/messages/{}", name), message.clone());
                }
            }
        }
    }
}

impl Default for AsyncApiParser {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests;
