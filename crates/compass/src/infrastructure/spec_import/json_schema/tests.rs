use super::*;
use crate::domain::spec::ir::StringFormat;
use crate::type_inference::Type;

#[test]
fn test_parse_simple_object() {
    let schema = r#"{
            "type": "object",
            "title": "User",
            "properties": {
                "id": { "type": "integer" },
                "name": { "type": "string" },
                "email": { "type": "string", "format": "email" }
            },
            "required": ["id", "name"]
        }"#;

    let mut parser = JsonSchemaParser::new();
    let spec = parser.parse_str(schema).unwrap();

    assert_eq!(spec.models.len(), 1);
    let model = &spec.models[0];
    assert_eq!(model.name, "User");
    assert_eq!(model.fields.len(), 3);

    let id_field = model.fields.iter().find(|f| f.name == "id").unwrap();
    assert!(id_field.required);
    assert_eq!(id_field.ty, Type::Int);

    let email_field = model.fields.iter().find(|f| f.name == "email").unwrap();
    assert!(!email_field.required);
    assert_eq!(email_field.constraints.format, Some(StringFormat::Email));
}

#[test]
fn test_parse_with_definitions() {
    let schema = r##"{
            "definitions": {
                "Address": {
                    "type": "object",
                    "properties": {
                        "street": { "type": "string" },
                        "city": { "type": "string" }
                    }
                }
            },
            "type": "object",
            "title": "Person",
            "properties": {
                "name": { "type": "string" },
                "address": { "$ref": "#/definitions/Address" }
            }
        }"##;

    let mut parser = JsonSchemaParser::new();
    let spec = parser.parse_str(schema).unwrap();

    assert_eq!(spec.models.len(), 2);

    let person = spec.get_model("Person").unwrap();
    let addr_field = person.fields.iter().find(|f| f.name == "address").unwrap();
    assert!(matches!(&addr_field.ty, Type::Instance { name, .. } if name == "Address"));
}

#[test]
fn test_parse_nullable_type() {
    let schema = r#"{
            "type": "object",
            "title": "Test",
            "properties": {
                "maybe_name": { "type": ["string", "null"] }
            }
        }"#;

    let mut parser = JsonSchemaParser::new();
    let spec = parser.parse_str(schema).unwrap();

    let model = &spec.models[0];
    let field = &model.fields[0];
    assert!(matches!(&field.ty, Type::Optional(inner) if **inner == Type::Str));
}

#[test]
fn test_parse_array_type() {
    let schema = r#"{
            "type": "object",
            "title": "Container",
            "properties": {
                "items": {
                    "type": "array",
                    "items": { "type": "string" },
                    "minItems": 1,
                    "maxItems": 10
                }
            }
        }"#;

    let mut parser = JsonSchemaParser::new();
    let spec = parser.parse_str(schema).unwrap();

    let model = &spec.models[0];
    let field = &model.fields[0];
    assert!(matches!(&field.ty, Type::List(inner) if **inner == Type::Str));
    assert_eq!(field.constraints.min_items, Some(1));
    assert_eq!(field.constraints.max_items, Some(10));
}

#[test]
fn test_parse_constraints() {
    let schema = r#"{
            "type": "object",
            "title": "Validated",
            "properties": {
                "age": {
                    "type": "integer",
                    "minimum": 0,
                    "maximum": 150
                },
                "username": {
                    "type": "string",
                    "minLength": 3,
                    "maxLength": 50,
                    "pattern": "^[a-z]+$"
                }
            }
        }"#;

    let mut parser = JsonSchemaParser::new();
    let spec = parser.parse_str(schema).unwrap();

    let model = &spec.models[0];

    let age = model.fields.iter().find(|f| f.name == "age").unwrap();
    assert_eq!(age.constraints.minimum, Some(0.0));
    assert_eq!(age.constraints.maximum, Some(150.0));

    let username = model.fields.iter().find(|f| f.name == "username").unwrap();
    assert_eq!(username.constraints.min_length, Some(3));
    assert_eq!(username.constraints.max_length, Some(50));
    assert_eq!(username.constraints.pattern, Some("^[a-z]+$".to_string()));
}

#[test]
fn test_case_conversion() {
    assert_eq!(to_pascal_case("user_name"), "UserName");
    assert_eq!(to_pascal_case("user-name"), "UserName");
    assert_eq!(to_pascal_case("userName"), "UserName");

    assert_eq!(to_snake_case("UserName"), "user_name");
    assert_eq!(to_snake_case("userName"), "user_name");
    assert_eq!(to_snake_case("user-name"), "user_name");
}
