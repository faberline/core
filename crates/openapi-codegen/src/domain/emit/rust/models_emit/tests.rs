use super::*;
use crate::build_type_map;
use crate::ir::openapi::Spec;
use crate::RustTarget;

fn render(spec_json: &str) -> String {
    let s: Spec = serde_json::from_str(spec_json).unwrap();
    let tm = build_type_map(&s);
    emit(&s, &tm, RustTarget::Rust2021)
}

fn holder() -> String {
    render(
        r##"{"paths":{},"components":{"schemas":{
          "Label":{"type":"string"},
          "Holder":{"type":"object","required":["name"],"properties":{
            "name":{"type":"string"},
            "payload":{},
            "label":{"$ref":"#/components/schemas/Label"},
            "note":{"type":"string"}}}}}}"##,
    )
}

#[test]
fn optional_untyped_field_skips_when_null() {
    let out = holder();
    assert!(
        out.contains(
            "    #[serde(default, skip_serializing_if = \"serde_json::Value::is_null\")]\n    pub payload: serde_json::Value,\n"
        ),
        "optional free-form field must default and skip when null:\n{out}"
    );
    assert!(
        !out.contains("pub payload: Option<"),
        "free-form field must not be wrapped in Option:\n{out}"
    );
}

#[test]
fn optional_untyped_field_never_pairs_value_with_option_is_none() {
    let out = holder();
    assert!(
        !out.contains("#[serde(skip_serializing_if = \"Option::is_none\")]\n    pub payload"),
        "serde_json::Value must never use Option::is_none:\n{out}"
    );
}

#[test]
fn other_optional_fields_keep_option_is_none() {
    let out = holder();
    assert!(
        out.contains(
            "    #[serde(skip_serializing_if = \"Option::is_none\")]\n    pub note: Option<String>,\n"
        ),
        "{out}"
    );
    assert!(
        out.contains(
            "    #[serde(skip_serializing_if = \"Option::is_none\")]\n    pub label: Option<Label>,\n"
        ),
        "{out}"
    );
}

#[test]
fn required_field_has_no_skip_attribute() {
    let out = holder();
    assert!(
        out.contains("    pub label: Option<Label>,\n    pub name: String,\n"),
        "required field `name` must carry no serde attribute:\n{out}"
    );
}

fn parse_spec(json: &str) -> Spec {
    serde_json::from_str(json).unwrap()
}

#[test]
fn declared_order() {
    let spec = parse_spec(
        r##"{
        "openapi": "3.0.0",
        "info": { "title": "Test", "version": "1.0.0" },
        "paths": {},
        "components": {
            "schemas": {
                "Status": {
                    "type": "string",
                    "enum": ["active", "pending", "inactive"]
                }
            }
        }
    }"##,
    );
    let tm = build_type_map(&spec);
    let out = emit(&spec, &tm, RustTarget::Rust2021);
    assert!(out.contains(
        "#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]\npub enum Status {"
    ));
    let pos_active = out.find("Active,").unwrap();
    let pos_pending = out.find("Pending,").unwrap();
    let pos_inactive = out.find("Inactive,").unwrap();
    assert!(pos_active < pos_pending);
    assert!(pos_pending < pos_inactive);
}

#[test]
fn rename_non_identifier_values() {
    let spec = parse_spec(
        r##"{
        "openapi": "3.0.0",
        "info": { "title": "Test", "version": "1.0.0" },
        "paths": {},
        "components": {
            "schemas": {
                "Special": {
                    "type": "string",
                    "enum": ["", "123", "kebab-case", "with space"]
                }
            }
        }
    }"##,
    );
    let tm = build_type_map(&spec);
    let out = emit(&spec, &tm, RustTarget::Rust2021);
    assert!(out.contains("    #[serde(rename = \"\")]\n    Anonymous,"));
    assert!(out.contains("    #[serde(rename = \"123\")]\n    _123,"));
    assert!(out.contains("    #[serde(rename = \"kebab-case\")]\n    KebabCase,"));
    assert!(out.contains("    #[serde(rename = \"with space\")]\n    WithSpace,"));
}

#[test]
fn collision_disambiguation() {
    let spec = parse_spec(
        r##"{
        "openapi": "3.0.0",
        "info": { "title": "Test", "version": "1.0.0" },
        "paths": {},
        "components": {
            "schemas": {
                "Collision": {
                    "type": "string",
                    "enum": ["foo_bar", "foo-bar", "FooBar"]
                }
            }
        }
    }"##,
    );
    let tm = build_type_map(&spec);
    let out = emit(&spec, &tm, RustTarget::Rust2021);
    assert!(out.contains("    #[serde(rename = \"foo_bar\")]\n    FooBar,"));
    assert!(out.contains("    #[serde(rename = \"foo-bar\")]\n    FooBar_2,"));
    assert!(out.contains("    #[serde(rename = \"FooBar\")]\n    FooBar_3,"));
}

#[test]
fn nullable_enum_emits_same_enum() {
    let spec = parse_spec(
        r##"{
        "openapi": "3.0.0",
        "info": { "title": "Test", "version": "1.0.0" },
        "paths": {},
        "components": {
            "schemas": {
                "NullableStatus": {
                    "type": "string",
                    "enum": ["active", "inactive"],
                    "nullable": true
                }
            }
        }
    }"##,
    );
    let tm = build_type_map(&spec);
    let out = emit(&spec, &tm, RustTarget::Rust2021);
    assert!(out.contains(
        "#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]\npub enum NullableStatus {"
    ));
    assert!(out.contains("    #[serde(rename = \"active\")]\n    Active,"));
    assert!(out.contains("    #[serde(rename = \"inactive\")]\n    Inactive,"));
}

#[test]
fn struct_property_refs_enum() {
    let spec = parse_spec(
        r##"{
        "openapi": "3.0.0",
        "info": { "title": "Test", "version": "1.0.0" },
        "paths": {},
        "components": {
            "schemas": {
                "Status": {
                    "type": "string",
                    "enum": ["active", "inactive"]
                },
                "User": {
                    "type": "object",
                    "properties": {
                        "status": { "$ref": "#/components/schemas/Status" },
                        "opt_status": { "$ref": "#/components/schemas/Status" },
                        "statuses": {
                            "type": "array",
                            "items": { "$ref": "#/components/schemas/Status" }
                        }
                    },
                    "required": ["status"]
                }
            }
        }
    }"##,
    );
    let tm = build_type_map(&spec);
    let out = emit(&spec, &tm, RustTarget::Rust2021);
    assert!(out.contains("pub enum Status {"));
    assert!(out.contains("pub struct User {"));
    assert!(out.contains("    pub status: Status,"));
    assert!(out.contains("    pub opt_status: Option<Status>,"));
    assert!(out.contains("    pub statuses: Option<Vec<Status>>,"));
}

#[test]
fn non_string_enum_left_as_string() {
    let spec = parse_spec(
        r##"{
        "openapi": "3.0.0",
        "info": { "title": "Test", "version": "1.0.0" },
        "paths": {},
        "components": {
            "schemas": {
                "IntEnum": {
                    "type": "integer",
                    "enum": [1, 2]
                },
                "UntypedEnum": {
                    "enum": ["a", "b"]
                },
                "MixedEnum": {
                    "type": "string",
                    "enum": ["a", 1]
                }
            }
        }
    }"##,
    );
    let tm = build_type_map(&spec);
    let out = emit(&spec, &tm, RustTarget::Rust2021);
    assert!(out.contains("pub type IntEnum = String;"));
    assert!(out.contains("pub type UntypedEnum = String;"));
    assert!(out.contains("pub type MixedEnum = String;"));
}

#[test]
fn serde_round_trip_rejects_undeclared_value() {
    // The emitted enum has no catch-all variant, so serde rejects any value
    // that is not declared. Assert on the emitter's own output.
    let spec = parse_spec(
        r##"{
        "openapi": "3.0.0",
        "info": { "title": "Test", "version": "1.0.0" },
        "paths": {},
        "components": {
            "schemas": {
                "Status": {
                    "type": "string",
                    "enum": ["active", "inactive"]
                }
            }
        }
    }"##,
    );
    let tm = build_type_map(&spec);
    let out = emit(&spec, &tm, RustTarget::Rust2021);
    let block = out
        .split("pub enum Status {")
        .nth(1)
        .and_then(|rest| rest.split("\n}\n").next())
        .expect("emitted Status enum");
    assert_eq!(block.matches("#[serde(rename = ").count(), 2);
    assert!(block.contains("    #[serde(rename = \"active\")]\n    Active,\n"));
    assert!(block.ends_with("    #[serde(rename = \"inactive\")]\n    Inactive,"));
    assert!(!block.contains("serde(other"));
    assert!(!block.contains("untagged"));
    assert!(!block.contains("(String)"));
}
