//! JSON Schema to SpecIR parser
//!
//! Converts JSON Schema definitions into DataModelSpec.

use crate::domain::spec::ir::{DataModelSpec, ModelDef};
use serde_json::Value;
use std::collections::HashMap;

mod constraints;
mod enums;
mod object;
mod types;

/// Error type for JSON Schema parsing
#[derive(Debug)]
pub enum JsonSchemaError {
    /// Invalid JSON
    InvalidJson(String),
    /// Missing required field
    MissingField(String),
    /// Unsupported schema feature
    UnsupportedFeature(String),
    /// Invalid type
    InvalidType(String),
    /// Reference error
    InvalidRef(String),
}

impl std::fmt::Display for JsonSchemaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            JsonSchemaError::InvalidJson(s) => write!(f, "invalid JSON: {}", s),
            JsonSchemaError::MissingField(s) => write!(f, "missing field: {}", s),
            JsonSchemaError::UnsupportedFeature(s) => write!(f, "unsupported feature: {}", s),
            JsonSchemaError::InvalidType(s) => write!(f, "invalid type: {}", s),
            JsonSchemaError::InvalidRef(s) => write!(f, "invalid reference: {}", s),
        }
    }
}

impl std::error::Error for JsonSchemaError {}

/// JSON Schema parser
pub struct JsonSchemaParser {
    /// Resolved definitions (for $ref handling)
    definitions: HashMap<String, Value>,
    /// Root schema
    root: Option<Value>,
}

impl JsonSchemaParser {
    pub fn new() -> Self {
        Self {
            definitions: HashMap::new(),
            root: None,
        }
    }

    /// Parse JSON Schema from string
    pub fn parse_str(&mut self, json: &str) -> Result<DataModelSpec, JsonSchemaError> {
        let value: Value =
            serde_json::from_str(json).map_err(|e| JsonSchemaError::InvalidJson(e.to_string()))?;
        self.parse_value(&value)
    }

    /// Parse JSON Schema from Value
    pub fn parse_value(&mut self, value: &Value) -> Result<DataModelSpec, JsonSchemaError> {
        self.root = Some(value.clone());

        // Extract definitions (draft-07: definitions, draft-2020-12: $defs)
        self.extract_definitions(value);

        let mut spec = DataModelSpec::new();

        // Parse root schema if it's an object type
        if let Some(root_model) = self.parse_root_object(value)? {
            spec.add_model(root_model);
        }

        // Parse all definitions as models
        let def_keys: Vec<String> = self.definitions.keys().cloned().collect();
        for name in def_keys {
            if let Some(def_value) = self.definitions.get(&name).cloned() {
                if let Some(model) = self.parse_definition(&name, &def_value)? {
                    spec.add_model(model);
                }
            }
        }

        Ok(spec)
    }

    /// Extract definitions from schema
    fn extract_definitions(&mut self, value: &Value) {
        // Draft-07 style: definitions
        if let Some(defs) = value.get("definitions").and_then(|v| v.as_object()) {
            for (name, def) in defs {
                self.definitions.insert(name.clone(), def.clone());
            }
        }

        // Draft-2020-12 style: $defs
        if let Some(defs) = value.get("$defs").and_then(|v| v.as_object()) {
            for (name, def) in defs {
                self.definitions.insert(name.clone(), def.clone());
            }
        }
    }

    /// Parse root object as a model
    fn parse_root_object(&self, value: &Value) -> Result<Option<ModelDef>, JsonSchemaError> {
        let type_val = value.get("type").and_then(|v| v.as_str());

        if type_val != Some("object") {
            return Ok(None);
        }

        // Get title or use "Root" as default
        let name = value
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or("Root")
            .to_string();

        self.parse_object_schema(&name, value)
    }
}

impl Default for JsonSchemaParser {
    fn default() -> Self {
        Self::new()
    }
}

/// Convert string to PascalCase
fn to_pascal_case(s: &str) -> String {
    let mut result = String::new();
    let mut capitalize_next = true;

    for c in s.chars() {
        if c == '_' || c == '-' || c == ' ' {
            capitalize_next = true;
        } else if capitalize_next {
            result.push(c.to_ascii_uppercase());
            capitalize_next = false;
        } else {
            result.push(c);
        }
    }

    result
}

/// Convert string to snake_case
fn to_snake_case(s: &str) -> String {
    let mut result = String::new();

    for (i, c) in s.chars().enumerate() {
        if c.is_uppercase() {
            if i > 0 {
                result.push('_');
            }
            result.push(c.to_ascii_lowercase());
        } else if c == '-' || c == ' ' {
            result.push('_');
        } else {
            result.push(c);
        }
    }

    result
}

#[cfg(test)]
mod tests;
