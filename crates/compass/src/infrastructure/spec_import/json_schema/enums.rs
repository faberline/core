//! Enum extraction.

use super::{to_pascal_case, JsonSchemaError, JsonSchemaParser};
use crate::domain::spec::ir::{EnumDef, EnumValue, EnumVariant};
use serde_json::Value;

impl JsonSchemaParser {
    /// Parse enums from schema
    pub fn parse_enums(&self, value: &Value) -> Result<Vec<EnumDef>, JsonSchemaError> {
        let mut enums = Vec::new();

        // Check definitions
        let defs_key = if value.get("$defs").is_some() {
            "$defs"
        } else {
            "definitions"
        };

        if let Some(defs) = value.get(defs_key).and_then(|v| v.as_object()) {
            for (name, def) in defs {
                if let Some(enum_vals) = def.get("enum").and_then(|v| v.as_array()) {
                    let enum_def = parse_enum_def(name, def, enum_vals);
                    enums.push(enum_def);
                }
            }
        }

        Ok(enums)
    }
}

/// Parse enum definition
fn parse_enum_def(name: &str, schema: &Value, values: &[Value]) -> EnumDef {
    let description = schema
        .get("description")
        .and_then(|v| v.as_str())
        .map(String::from);

    let variants: Vec<EnumVariant> = values
        .iter()
        .filter_map(|v| {
            let (variant_name, value) = match v {
                Value::String(s) => (to_pascal_case(s), Some(EnumValue::String(s.clone()))),
                Value::Number(n) => {
                    if let Some(i) = n.as_i64() {
                        (format!("Value{}", i), Some(EnumValue::Int(i)))
                    } else {
                        return None;
                    }
                }
                _ => return None,
            };

            Some(EnumVariant {
                name: variant_name,
                value,
                description: None,
            })
        })
        .collect();

    EnumDef {
        name: to_pascal_case(name),
        description,
        variants,
    }
}
