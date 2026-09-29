//! Object definitions and fields.

use super::{to_pascal_case, to_snake_case, JsonSchemaError, JsonSchemaParser};
use crate::domain::spec::ir::{FieldDef, ModelDef};
use serde_json::Value;

impl JsonSchemaParser {
    /// Parse a definition into a model
    pub(super) fn parse_definition(
        &self,
        name: &str,
        value: &Value,
    ) -> Result<Option<ModelDef>, JsonSchemaError> {
        let type_val = value.get("type").and_then(|v| v.as_str());

        // Handle enum type
        if value.get("enum").is_some() {
            // Enums are handled separately
            return Ok(None);
        }

        match type_val {
            Some("object") => self.parse_object_schema(name, value),
            _ => Ok(None),
        }
    }

    /// Parse object schema into ModelDef
    pub(super) fn parse_object_schema(
        &self,
        name: &str,
        value: &Value,
    ) -> Result<Option<ModelDef>, JsonSchemaError> {
        let mut model = ModelDef::new(to_pascal_case(name));

        // Description
        if let Some(desc) = value.get("description").and_then(|v| v.as_str()) {
            model.description = Some(desc.to_string());
        }

        // Required fields
        let required_fields: Vec<String> = value
            .get("required")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        // Properties
        if let Some(props) = value.get("properties").and_then(|v| v.as_object()) {
            for (prop_name, prop_schema) in props {
                let field = self.parse_field(prop_name, prop_schema, &required_fields)?;
                model.fields.push(field);
            }
        }

        Ok(Some(model))
    }

    /// Parse a property into FieldDef
    fn parse_field(
        &self,
        name: &str,
        schema: &Value,
        required_fields: &[String],
    ) -> Result<FieldDef, JsonSchemaError> {
        let ty = self.parse_type(schema)?;
        let is_required = required_fields.contains(&name.to_string());

        let mut field = FieldDef::new(to_snake_case(name), ty);
        field.required = is_required;

        // Description
        if let Some(desc) = schema.get("description").and_then(|v| v.as_str()) {
            field.description = Some(desc.to_string());
        }

        // Default value
        if let Some(default) = schema.get("default") {
            field.default = Some(default.to_string());
            field.required = false;
        }

        // Constraints
        field.constraints = self.parse_constraints(schema);

        Ok(field)
    }
}
