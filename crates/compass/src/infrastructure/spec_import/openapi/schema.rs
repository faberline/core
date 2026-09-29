//! Component schemas, type mapping and constraints.

use super::{OpenApiError, OpenApiParser};
use crate::domain::spec::ir::{DataModelSpec, FieldConstraints, FieldDef, ModelDef, StringFormat};
use crate::type_inference::Type;
use serde_json::Value;

impl OpenApiParser {
    /// Parse component schemas into DataModelSpec
    pub(super) fn parse_component_schemas(&self) -> Result<DataModelSpec, OpenApiError> {
        let mut spec = DataModelSpec::new();

        for (ref_path, schema) in &self.components {
            // Extract name from ref path
            let name = ref_path
                .strip_prefix("#/components/schemas/")
                .unwrap_or(ref_path);

            if let Some(model) = self.parse_schema_as_model(name, schema)? {
                spec.add_model(model);
            }
        }

        Ok(spec)
    }

    /// Parse a schema as a model definition
    fn parse_schema_as_model(
        &self,
        name: &str,
        schema: &Value,
    ) -> Result<Option<ModelDef>, OpenApiError> {
        let schema_type = schema.get("type").and_then(|v| v.as_str());

        // Only parse object types as models
        if schema_type != Some("object") {
            return Ok(None);
        }

        let mut model = ModelDef::new(name);
        model.description = schema
            .get("description")
            .and_then(|v| v.as_str())
            .map(String::from);

        // Parse properties
        let required_fields: Vec<String> = schema
            .get("required")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        if let Some(props) = schema.get("properties").and_then(|v| v.as_object()) {
            for (prop_name, prop_schema) in props {
                let field = self.parse_property(prop_name, prop_schema, &required_fields);
                model.add_field(field);
            }
        }

        Ok(Some(model))
    }

    /// Parse a property into FieldDef
    fn parse_property(&self, name: &str, schema: &Value, required: &[String]) -> FieldDef {
        let ty = self.schema_to_type(schema);
        let is_required = required.contains(&name.to_string());

        let mut field = FieldDef::new(to_snake_case(name), ty);
        field.required = is_required;
        field.description = schema
            .get("description")
            .and_then(|v| v.as_str())
            .map(String::from);

        if let Some(default) = schema.get("default") {
            field.default = Some(default.to_string());
        }

        // Parse constraints
        field.constraints = self.parse_constraints(schema);

        field
    }

    /// Convert OpenAPI schema to Type
    pub(super) fn schema_to_type(&self, schema: &Value) -> Type {
        // Handle $ref
        if let Some(ref_str) = schema.get("$ref").and_then(|v| v.as_str()) {
            return self.resolve_ref(ref_str);
        }

        // Handle allOf
        if let Some(all_of) = schema.get("allOf").and_then(|v| v.as_array()) {
            if let Some(first) = all_of.first() {
                return self.schema_to_type(first);
            }
        }

        // Handle oneOf/anyOf
        if let Some(one_of) = schema.get("oneOf").and_then(|v| v.as_array()) {
            let types: Vec<Type> = one_of.iter().map(|s| self.schema_to_type(s)).collect();
            return Type::Union(types);
        }
        if let Some(any_of) = schema.get("anyOf").and_then(|v| v.as_array()) {
            let types: Vec<Type> = any_of.iter().map(|s| self.schema_to_type(s)).collect();
            return Type::Union(types);
        }

        // Get type
        let type_str = schema.get("type").and_then(|v| v.as_str()).unwrap_or("any");
        let format = schema.get("format").and_then(|v| v.as_str());
        let nullable = schema
            .get("nullable")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let base_type = match type_str {
            "string" => match format {
                Some("date-time") => Type::Str, // Could use special DateTime type
                Some("date") => Type::Str,
                Some("time") => Type::Str,
                Some("email") => Type::Str,
                Some("uri") | Some("url") => Type::Str,
                Some("uuid") => Type::Str,
                Some("binary") => Type::Bytes,
                Some("byte") => Type::Bytes,
                _ => Type::Str,
            },
            "integer" => Type::Int,
            "number" => Type::Float,
            "boolean" => Type::Bool,
            "array" => {
                let items = schema.get("items").unwrap_or(&Value::Null);
                let item_type = self.schema_to_type(items);
                Type::List(Box::new(item_type))
            }
            "object" => {
                // Check for additionalProperties (dict)
                if let Some(add_props) = schema.get("additionalProperties") {
                    if add_props.is_boolean() && add_props.as_bool() == Some(true) {
                        return Type::Dict(Box::new(Type::Str), Box::new(Type::Any));
                    }
                    if add_props.is_object() {
                        let value_type = self.schema_to_type(add_props);
                        return Type::Dict(Box::new(Type::Str), Box::new(value_type));
                    }
                }

                // Check for title (named object)
                if let Some(title) = schema.get("title").and_then(|v| v.as_str()) {
                    return Type::Instance {
                        name: to_pascal_case(title),
                        module: None,
                        type_args: vec![],
                    };
                }

                Type::Dict(Box::new(Type::Str), Box::new(Type::Any))
            }
            "null" => Type::None,
            _ => Type::Any,
        };

        if nullable {
            Type::Optional(Box::new(base_type))
        } else {
            base_type
        }
    }

    /// Resolve a $ref to a Type
    fn resolve_ref(&self, ref_str: &str) -> Type {
        // Extract name from ref
        let name = ref_str
            .strip_prefix("#/components/schemas/")
            .unwrap_or(ref_str);

        Type::Instance {
            name: to_pascal_case(name),
            module: None,
            type_args: vec![],
        }
    }

    /// Parse constraints from schema
    fn parse_constraints(&self, schema: &Value) -> FieldConstraints {
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
            constraints.format = parse_format(format);
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

/// Parse format string to StringFormat
fn parse_format(format: &str) -> Option<StringFormat> {
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
        other => Some(StringFormat::Custom(other.to_string())),
    }
}

/// Convert to snake_case
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

/// Convert to PascalCase
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
