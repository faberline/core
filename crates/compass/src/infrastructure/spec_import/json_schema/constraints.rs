//! Field constraints and string formats.

use super::JsonSchemaParser;
use crate::domain::spec::ir::{FieldConstraints, StringFormat};
use serde_json::Value;

impl JsonSchemaParser {
    /// Parse constraints from schema
    pub(super) fn parse_constraints(&self, schema: &Value) -> FieldConstraints {
        let mut constraints = FieldConstraints::default();

        // String constraints
        if let Some(min) = schema.get("minLength").and_then(|v| v.as_u64()) {
            constraints.min_length = Some(min as usize);
        }
        if let Some(max) = schema.get("maxLength").and_then(|v| v.as_u64()) {
            constraints.max_length = Some(max as usize);
        }
        if let Some(pattern) = schema.get("pattern").and_then(|v| v.as_str()) {
            constraints.pattern = Some(pattern.to_string());
        }
        if let Some(format) = schema.get("format").and_then(|v| v.as_str()) {
            constraints.format = parse_string_format(format);
        }

        // Numeric constraints
        if let Some(min) = schema.get("minimum").and_then(|v| v.as_f64()) {
            constraints.minimum = Some(min);
        }
        if let Some(max) = schema.get("maximum").and_then(|v| v.as_f64()) {
            constraints.maximum = Some(max);
        }
        if let Some(min) = schema.get("exclusiveMinimum").and_then(|v| v.as_f64()) {
            constraints.exclusive_minimum = Some(min);
        }
        if let Some(max) = schema.get("exclusiveMaximum").and_then(|v| v.as_f64()) {
            constraints.exclusive_maximum = Some(max);
        }
        if let Some(mult) = schema.get("multipleOf").and_then(|v| v.as_f64()) {
            constraints.multiple_of = Some(mult);
        }

        // Array constraints
        if let Some(min) = schema.get("minItems").and_then(|v| v.as_u64()) {
            constraints.min_items = Some(min as usize);
        }
        if let Some(max) = schema.get("maxItems").and_then(|v| v.as_u64()) {
            constraints.max_items = Some(max as usize);
        }
        if let Some(unique) = schema.get("uniqueItems").and_then(|v| v.as_bool()) {
            constraints.unique_items = unique;
        }

        constraints
    }
}

/// Parse string format
fn parse_string_format(format: &str) -> Option<StringFormat> {
    match format {
        "email" => Some(StringFormat::Email),
        "uri" | "url" => Some(StringFormat::Url),
        "uuid" => Some(StringFormat::Uuid),
        "date-time" | "datetime" => Some(StringFormat::DateTime),
        "date" => Some(StringFormat::Date),
        "time" => Some(StringFormat::Time),
        "duration" => Some(StringFormat::Duration),
        "hostname" => Some(StringFormat::Hostname),
        "ipv4" => Some(StringFormat::Ipv4),
        "ipv6" => Some(StringFormat::Ipv6),
        "regex" => Some(StringFormat::Regex),
        "json-pointer" => Some(StringFormat::JsonPointer),
        other => Some(StringFormat::Custom(other.to_string())),
    }
}
