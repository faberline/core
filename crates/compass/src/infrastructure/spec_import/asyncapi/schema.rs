//! Message schemas, type mapping and constraints.

use super::{AsyncApiError, AsyncApiParser};
use crate::domain::spec::ir::{DataModelSpec, FieldConstraints, FieldDef, ModelDef, StringFormat};
use crate::type_inference::Type;
use serde_json::Value;

impl AsyncApiParser {
    /// Parse message schemas into DataModelSpec
    pub(super) fn parse_message_schemas(
        &self,
        value: &Value,
    ) -> Result<DataModelSpec, AsyncApiError> {
        let mut spec = DataModelSpec::new();

        // Parse schemas from components
        if let Some(components) = value.get("components") {
            // Schemas
            if let Some(schemas) = components.get("schemas").and_then(|s| s.as_object()) {
                for (name, schema) in schemas {
                    if let Some(model) = self.parse_schema_as_model(name, schema)? {
                        spec.add_model(model);
                    }
                }
            }

            // Messages (extract their payloads as models)
            if let Some(messages) = components.get("messages").and_then(|m| m.as_object()) {
                for (name, message) in messages {
                    if let Some(payload) = message.get("payload") {
                        let model_name = format!("{}Message", name);
                        if let Some(model) = self.parse_schema_as_model(&model_name, payload)? {
                            spec.add_model(model);
                        }
                    }
                }
            }
        }

        Ok(spec)
    }

    /// Parse a schema as a model
    fn parse_schema_as_model(
        &self,
        name: &str,
        schema: &Value,
    ) -> Result<Option<ModelDef>, AsyncApiError> {
        let schema_type = schema.get("type").and_then(|t| t.as_str());

        // Only create models for object types
        if schema_type != Some("object") {
            return Ok(None);
        }

        let mut model = ModelDef {
            name: name.to_string(),
            description: schema
                .get("description")
                .and_then(|d| d.as_str())
                .map(String::from),
            fields: Vec::new(),
            methods: Vec::new(),
            extends: Vec::new(),
            type_params: Vec::new(),
            is_abstract: false,
            table_name: None,
            collection_name: None,
        };

        // Parse properties
        if let Some(properties) = schema.get("properties").and_then(|p| p.as_object()) {
            let required_fields: Vec<&str> = schema
                .get("required")
                .and_then(|r| r.as_array())
                .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect())
                .unwrap_or_default();

            for (field_name, field_schema) in properties {
                let ty = self.schema_to_type(field_schema)?;
                let required = required_fields.contains(&field_name.as_str());

                let field_type = if required {
                    ty
                } else {
                    Type::Optional(Box::new(ty))
                };

                model.fields.push(FieldDef {
                    name: field_name.clone(),
                    ty: field_type,
                    default: field_schema.get("default").map(|v| v.to_string()),
                    description: field_schema
                        .get("description")
                        .and_then(|d| d.as_str())
                        .map(String::from),
                    required,
                    constraints: self.parse_constraints(field_schema),
                    column_name: None,
                    primary_key: false,
                    unique: false,
                    indexed: false,
                    foreign_key: None,
                    alias: None,
                });
            }
        }

        Ok(Some(model))
    }

    /// Convert JSON Schema to Type
    pub(super) fn schema_to_type(&self, schema: &Value) -> Result<Type, AsyncApiError> {
        // Handle $ref
        if let Some(ref_str) = schema.get("$ref").and_then(|r| r.as_str()) {
            let name = ref_str.rsplit('/').next().unwrap_or(ref_str);
            return Ok(Type::Instance {
                name: name.to_string(),
                module: None,
                type_args: vec![],
            });
        }

        let schema_type = schema.get("type").and_then(|t| t.as_str());

        match schema_type {
            Some("string") => {
                // Check for format
                if let Some(format) = schema.get("format").and_then(|f| f.as_str()) {
                    let sf = match format {
                        "date-time" => Some(StringFormat::DateTime),
                        "date" => Some(StringFormat::Date),
                        "time" => Some(StringFormat::Time),
                        "email" => Some(StringFormat::Email),
                        "uuid" => Some(StringFormat::Uuid),
                        "uri" | "url" => Some(StringFormat::Uri),
                        _ => None,
                    };
                    if let Some(string_format) = sf {
                        return Ok(Type::Instance {
                            name: format!("Formatted<{:?}>", string_format),
                            module: None,
                            type_args: vec![],
                        });
                    }
                }
                // Check for enum
                if let Some(enum_values) = schema.get("enum").and_then(|e| e.as_array()) {
                    let values: Vec<Type> = enum_values
                        .iter()
                        .filter_map(|v| v.as_str())
                        .map(|s| {
                            Type::Literal(crate::type_inference::LiteralValue::Str(s.to_string()))
                        })
                        .collect();
                    return Ok(Type::Union(values));
                }
                Ok(Type::Str)
            }
            Some("integer") => Ok(Type::Int),
            Some("number") => Ok(Type::Float),
            Some("boolean") => Ok(Type::Bool),
            Some("array") => {
                let items_type = if let Some(items) = schema.get("items") {
                    self.schema_to_type(items)?
                } else {
                    Type::Any
                };
                Ok(Type::List(Box::new(items_type)))
            }
            Some("object") => {
                // Check for additionalProperties (dict)
                if let Some(additional) = schema.get("additionalProperties") {
                    if additional.is_boolean() || additional.is_object() {
                        let value_type = if additional.is_object() {
                            self.schema_to_type(additional)?
                        } else {
                            Type::Any
                        };
                        return Ok(Type::Dict(Box::new(Type::Str), Box::new(value_type)));
                    }
                }
                // Check for inline object with properties
                if schema.get("properties").is_some() {
                    // This should be a named model, but for inline we use Any
                    return Ok(Type::Any);
                }
                Ok(Type::Dict(Box::new(Type::Str), Box::new(Type::Any)))
            }
            Some("null") => Ok(Type::None),
            None => {
                // Check for oneOf/anyOf/allOf
                if let Some(one_of) = schema.get("oneOf").and_then(|o| o.as_array()) {
                    let types: Vec<Type> = one_of
                        .iter()
                        .filter_map(|s| self.schema_to_type(s).ok())
                        .collect();
                    return Ok(Type::Union(types));
                }
                if let Some(any_of) = schema.get("anyOf").and_then(|o| o.as_array()) {
                    let types: Vec<Type> = any_of
                        .iter()
                        .filter_map(|s| self.schema_to_type(s).ok())
                        .collect();
                    return Ok(Type::Union(types));
                }
                Ok(Type::Any)
            }
            _ => Ok(Type::Any),
        }
    }

    /// Parse field constraints
    fn parse_constraints(&self, schema: &Value) -> FieldConstraints {
        FieldConstraints {
            min_length: schema
                .get("minLength")
                .and_then(|v| v.as_u64())
                .map(|v| v as usize),
            max_length: schema
                .get("maxLength")
                .and_then(|v| v.as_u64())
                .map(|v| v as usize),
            pattern: schema
                .get("pattern")
                .and_then(|v| v.as_str())
                .map(String::from),
            format: schema
                .get("format")
                .and_then(|v| v.as_str())
                .and_then(|f| match f {
                    "email" => Some(StringFormat::Email),
                    "uri" | "url" => Some(StringFormat::Uri),
                    "uuid" => Some(StringFormat::Uuid),
                    "date-time" => Some(StringFormat::DateTime),
                    "date" => Some(StringFormat::Date),
                    "time" => Some(StringFormat::Time),
                    _ => None,
                }),
            minimum: schema.get("minimum").and_then(|v| v.as_f64()),
            maximum: schema.get("maximum").and_then(|v| v.as_f64()),
            exclusive_minimum: schema.get("exclusiveMinimum").and_then(|v| v.as_f64()),
            exclusive_maximum: schema.get("exclusiveMaximum").and_then(|v| v.as_f64()),
            multiple_of: schema.get("multipleOf").and_then(|v| v.as_f64()),
            min_items: schema
                .get("minItems")
                .and_then(|v| v.as_u64())
                .map(|v| v as usize),
            max_items: schema
                .get("maxItems")
                .and_then(|v| v.as_u64())
                .map(|v| v as usize),
            unique_items: schema
                .get("uniqueItems")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
        }
    }
}
