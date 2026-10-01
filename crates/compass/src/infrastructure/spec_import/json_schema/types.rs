//! JSON Schema type mapping.

use super::{to_pascal_case, JsonSchemaError, JsonSchemaParser};
use crate::type_inference::Type;
use serde_json::Value;

impl JsonSchemaParser {
    /// Parse JSON Schema type into Type IR
    pub(super) fn parse_type(&self, schema: &Value) -> Result<Type, JsonSchemaError> {
        // Handle $ref
        if let Some(ref_str) = schema.get("$ref").and_then(|v| v.as_str()) {
            return self.resolve_ref(ref_str);
        }

        // Handle allOf (intersection/extends)
        if let Some(all_of) = schema.get("allOf").and_then(|v| v.as_array()) {
            return self.parse_all_of(all_of);
        }

        // Handle oneOf/anyOf (union)
        if let Some(one_of) = schema.get("oneOf").and_then(|v| v.as_array()) {
            return self.parse_union(one_of);
        }
        if let Some(any_of) = schema.get("anyOf").and_then(|v| v.as_array()) {
            return self.parse_union(any_of);
        }

        // Handle enum
        if let Some(enum_vals) = schema.get("enum").and_then(|v| v.as_array()) {
            return self.parse_enum_type(enum_vals);
        }

        // Handle const
        if let Some(const_val) = schema.get("const") {
            return self.parse_literal(const_val);
        }

        // Handle type array (e.g., ["string", "null"])
        if let Some(types) = schema.get("type").and_then(|v| v.as_array()) {
            return self.parse_type_array(schema, types);
        }

        // Handle single type
        let type_str = schema.get("type").and_then(|v| v.as_str()).unwrap_or("any");

        self.parse_single_type(schema, type_str)
    }

    /// Parse a single type string
    fn parse_single_type(&self, schema: &Value, type_str: &str) -> Result<Type, JsonSchemaError> {
        match type_str {
            "string" => {
                // Check for format
                if let Some(format) = schema.get("format").and_then(|v| v.as_str()) {
                    return Ok(self.format_to_type(format));
                }
                Ok(Type::Str)
            }
            "integer" | "number" => {
                if type_str == "integer" {
                    Ok(Type::Int)
                } else {
                    Ok(Type::Float)
                }
            }
            "boolean" => Ok(Type::Bool),
            "null" => Ok(Type::None),
            "array" => {
                let item_type = if let Some(items) = schema.get("items") {
                    self.parse_type(items)?
                } else {
                    Type::Any
                };
                Ok(Type::List(Box::new(item_type)))
            }
            "object" => {
                // Check for additionalProperties (dict type)
                if let Some(add_props) = schema.get("additionalProperties") {
                    if add_props.is_boolean() && add_props.as_bool() == Some(true) {
                        return Ok(Type::Dict(Box::new(Type::Str), Box::new(Type::Any)));
                    }
                    if add_props.is_object() {
                        let value_type = self.parse_type(add_props)?;
                        return Ok(Type::Dict(Box::new(Type::Str), Box::new(value_type)));
                    }
                }
                // Named object reference
                if let Some(title) = schema.get("title").and_then(|v| v.as_str()) {
                    return Ok(Type::Instance {
                        name: to_pascal_case(title),
                        module: None,
                        type_args: vec![],
                    });
                }
                Ok(Type::Dict(Box::new(Type::Str), Box::new(Type::Any)))
            }
            _ => Err(JsonSchemaError::InvalidType(type_str.to_string())),
        }
    }

    /// Parse type array (union with null)
    fn parse_type_array(&self, schema: &Value, types: &[Value]) -> Result<Type, JsonSchemaError> {
        let mut parsed_types = Vec::new();

        for type_val in types {
            if let Some(type_str) = type_val.as_str() {
                let ty = self.parse_single_type(schema, type_str)?;
                parsed_types.push(ty);
            }
        }

        // Special case: ["type", "null"] -> Optional<type>
        if parsed_types.len() == 2 && parsed_types.contains(&Type::None) {
            let non_null = parsed_types
                .into_iter()
                .find(|t| t != &Type::None)
                .unwrap_or(Type::Any);
            return Ok(Type::Optional(Box::new(non_null)));
        }

        if parsed_types.len() == 1 {
            return Ok(parsed_types.remove(0));
        }

        Ok(Type::Union(parsed_types))
    }

    /// Parse allOf (intersection)
    fn parse_all_of(&self, items: &[Value]) -> Result<Type, JsonSchemaError> {
        // If single item, just parse it
        if items.len() == 1 {
            return self.parse_type(&items[0]);
        }

        // For multiple items, try to find a $ref and merge
        let mut base_ref = None;
        for item in items {
            if item.get("$ref").is_some() {
                base_ref = Some(self.parse_type(item)?);
                break;
            }
        }

        if let Some(base) = base_ref {
            // Return the base type - properties are merged in model parsing
            return Ok(base);
        }

        // Fall back to first item
        self.parse_type(&items[0])
    }

    /// Parse oneOf/anyOf (union)
    fn parse_union(&self, items: &[Value]) -> Result<Type, JsonSchemaError> {
        let types: Vec<Type> = items
            .iter()
            .map(|item| self.parse_type(item))
            .collect::<Result<Vec<_>, _>>()?;

        if types.len() == 1 {
            return Ok(types.into_iter().next().unwrap());
        }

        Ok(Type::Union(types))
    }

    /// Parse enum values as literal union
    fn parse_enum_type(&self, values: &[Value]) -> Result<Type, JsonSchemaError> {
        let literals: Vec<Type> = values
            .iter()
            .filter_map(|v| self.value_to_literal(v))
            .collect();

        if literals.len() == 1 {
            return Ok(literals.into_iter().next().unwrap());
        }

        Ok(Type::Union(literals))
    }

    /// Convert JSON value to Literal type
    fn value_to_literal(&self, value: &Value) -> Option<Type> {
        use crate::type_inference::LiteralValue;

        match value {
            Value::String(s) => Some(Type::Literal(LiteralValue::Str(s.clone()))),
            Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    Some(Type::Literal(LiteralValue::Int(i)))
                } else if let Some(f) = n.as_f64() {
                    Some(Type::Literal(LiteralValue::Float(f)))
                } else {
                    None
                }
            }
            Value::Bool(b) => Some(Type::Literal(LiteralValue::Bool(*b))),
            Value::Null => Some(Type::Literal(LiteralValue::None)),
            _ => None,
        }
    }

    /// Parse const value as literal
    fn parse_literal(&self, value: &Value) -> Result<Type, JsonSchemaError> {
        self.value_to_literal(value)
            .ok_or_else(|| JsonSchemaError::InvalidType("unsupported const value".to_string()))
    }

    /// Resolve $ref to a type
    fn resolve_ref(&self, ref_str: &str) -> Result<Type, JsonSchemaError> {
        // Handle local references: #/definitions/Name or #/$defs/Name
        if let Some(name) = ref_str.strip_prefix("#/definitions/") {
            return Ok(Type::Instance {
                name: to_pascal_case(name),
                module: None,
                type_args: vec![],
            });
        }
        if let Some(name) = ref_str.strip_prefix("#/$defs/") {
            return Ok(Type::Instance {
                name: to_pascal_case(name),
                module: None,
                type_args: vec![],
            });
        }

        Err(JsonSchemaError::InvalidRef(ref_str.to_string()))
    }

    /// Convert format string to Type
    fn format_to_type(&self, format: &str) -> Type {
        // For now, all formats map to Str but we record the format
        // The constraint will be used for validation
        match format {
            "date-time" | "datetime" => Type::Str, // Could be a special DateTime type
            "date" => Type::Str,
            "time" => Type::Str,
            "email" => Type::Str,
            "uri" | "url" => Type::Str,
            "uuid" => Type::Str,
            "hostname" => Type::Str,
            "ipv4" => Type::Str,
            "ipv6" => Type::Str,
            _ => Type::Str,
        }
    }
}
