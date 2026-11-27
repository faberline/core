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

const UNIONS: &str = r##"{
    "openapi": "3.1.0",
    "info": { "title": "Test", "version": "1.0.0" },
    "paths": {},
    "components": { "schemas": {
        "Cat": { "type": "object", "properties": { "name": { "type": "string" } }, "required": ["name"] },
        "Dog": { "type": "object", "properties": { "bark": { "type": "boolean" } }, "required": ["bark"] },
        "Pet": {
            "oneOf": [ { "$ref": "#/components/schemas/Cat" }, { "$ref": "#/components/schemas/Dog" } ],
            "discriminator": { "propertyName": "kind", "mapping": { "cat": "#/components/schemas/Cat" } }
        },
        "Shape": { "oneOf": [ { "$ref": "#/components/schemas/Cat" }, { "$ref": "#/components/schemas/Dog" } ] },
        "Id": { "anyOf": [ { "type": "string" }, { "type": "integer" } ] },
        "Blob": { "oneOf": [ { "type": "object" }, { "type": "object" } ] },
        "Twin": { "oneOf": [ { "type": "string" }, { "type": "string" } ] },
        "Owner": {
            "type": "object",
            "properties": { "pet": { "$ref": "#/components/schemas/Pet" } },
            "required": ["pet"]
        }
    } }
}"##;

#[test]
fn tagged_union_emits_internally_tagged_enum() {
    let spec = parse_spec(UNIONS);
    let tm = build_type_map(&spec);
    let (out, warnings) = emit_with_warnings(&spec, &tm, RustTarget::Rust2021);
    assert!(out.contains("#[serde(tag = \"kind\")]\npub enum Pet {"));
    assert!(out.contains("    #[serde(rename = \"cat\")]\n    Cat(Cat),"));
    assert!(out.contains("    #[serde(rename = \"Dog\")]\n    Dog(Dog),"));
    assert!(warnings.iter().all(|w| !w.contains("`Pet`")));
}

#[test]
fn untagged_union_of_refs_and_primitives() {
    let spec = parse_spec(UNIONS);
    let tm = build_type_map(&spec);
    let (out, _) = emit_with_warnings(&spec, &tm, RustTarget::Rust2021);
    assert!(out.contains("#[serde(untagged)]\npub enum Shape {"));
    assert!(out.contains("    Cat(Cat),\n    Dog(Dog),\n}"));
    assert!(out.contains("#[serde(untagged)]\npub enum Id {"));
    assert!(out.contains("    String(String),\n    Integer(i64),\n}"));
}

#[test]
fn ambiguous_union_stays_value_with_warning() {
    let spec = parse_spec(UNIONS);
    let tm = build_type_map(&spec);
    let (out, warnings) = emit_with_warnings(&spec, &tm, RustTarget::Rust2021);
    assert!(out.contains("pub type Blob = serde_json::Value;"));
    assert!(out.contains("pub type Twin = serde_json::Value;"));
    assert_eq!(warnings.len(), 2);
    assert!(warnings.iter().any(|w| w.contains("`Blob`")));
    assert!(warnings.iter().any(|w| w.contains("`Twin`")));
}

#[test]
fn tagged_union_with_tag_field_in_variant_is_ambiguous() {
    let spec = parse_spec(
        r##"{
        "openapi": "3.1.0",
        "info": { "title": "Test", "version": "1.0.0" },
        "paths": {},
        "components": { "schemas": {
            "Cat": { "type": "object", "properties": { "kind": { "type": "string" } } },
            "Dog": { "type": "object", "properties": { "bark": { "type": "boolean" } } },
            "Pet": {
                "oneOf": [ { "$ref": "#/components/schemas/Cat" }, { "$ref": "#/components/schemas/Dog" } ],
                "discriminator": { "propertyName": "kind" }
            }
        } }
    }"##,
    );
    let tm = build_type_map(&spec);
    let (out, warnings) = emit_with_warnings(&spec, &tm, RustTarget::Rust2021);
    assert!(out.contains("pub type Pet = serde_json::Value;"));
    assert!(warnings.iter().any(|w| w.contains("`Pet`")));
}

#[test]
fn union_property_ref_uses_enum_name() {
    let spec = parse_spec(UNIONS);
    let tm = build_type_map(&spec);
    let out = emit(&spec, &tm, RustTarget::Rust2021);
    assert!(out.contains("    pub pet: Pet,"));
}

#[test]
fn forcing_value_would_break_round_trip_guard() {
    let spec = parse_spec(UNIONS);
    let tm = build_type_map(&spec);
    let out = emit(&spec, &tm, RustTarget::Rust2021);
    assert!(!out.contains("pub type Pet = serde_json::Value;"));
}

fn render_with_warnings(spec_json: &str) -> (String, Vec<String>) {
    let spec = parse_spec(spec_json);
    let tm = build_type_map(&spec);
    emit_with_warnings(&spec, &tm, RustTarget::Rust2021)
}

#[test]
fn inline_union_with_distinct_required_is_typed_enum() {
    let (out, _) = render_with_warnings(
        r##"{
        "openapi": "3.1.0", "info": { "title": "Test", "version": "1.0.0" }, "paths": {},
        "components": { "schemas": { "Outcome": { "oneOf": [
            { "type": "object", "required": ["rows"], "properties": { "rows": { "type": "array", "items": { "type": "string" } } } },
            { "type": "object", "required": ["total"], "properties": { "total": { "type": "integer" } } }
        ] } } }
    }"##,
    );
    assert!(out.contains("pub struct OutcomeRows {"), "{out}");
    assert!(out.contains("    pub rows: Vec<String>,\n"), "{out}");
    assert!(out.contains("pub struct OutcomeTotal {"), "{out}");
    assert!(out.contains("    pub total: i64,\n"), "{out}");
    assert!(
        out.contains(
            "#[serde(untagged)]\npub enum Outcome {\n    OutcomeRows(OutcomeRows),\n    OutcomeTotal(OutcomeTotal),\n}\n"
        ),
        "{out}"
    );
    assert!(out.find("pub struct OutcomeRows").unwrap() < out.find("pub enum Outcome").unwrap());
    assert!(
        !out.contains("pub type Outcome = serde_json::Value;"),
        "{out}"
    );
}

#[test]
fn inline_union_names_use_variant_index_when_not_one_required() {
    let (out, _) = render_with_warnings(
        r##"{
        "openapi": "3.1.0", "info": { "title": "Test", "version": "1.0.0" }, "paths": {},
        "components": { "schemas": { "Pair": { "oneOf": [
            { "type": "object", "required": ["a", "b"], "properties": { "a": { "type": "string" }, "b": { "type": "string" } } },
            { "type": "object", "required": ["c", "d"], "properties": { "c": { "type": "string" }, "d": { "type": "string" } } }
        ] } } }
    }"##,
    );
    assert!(out.contains("pub struct PairVariant1 {"), "{out}");
    assert!(out.contains("pub struct PairVariant2 {"), "{out}");
}

#[test]
fn inline_union_same_required_stays_value() {
    let (out, warnings) = render_with_warnings(
        r##"{
        "openapi": "3.1.0", "info": { "title": "Test", "version": "1.0.0" }, "paths": {},
        "components": { "schemas": { "Clash": { "oneOf": [
            { "type": "object", "required": ["id"], "properties": { "id": { "type": "string" } } },
            { "type": "object", "required": ["id"], "properties": { "id": { "type": "string" } } }
        ] } } }
    }"##,
    );
    assert!(out.contains("pub type Clash = serde_json::Value;"), "{out}");
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    assert!(warnings[0].contains("`Clash`"), "{warnings:?}");
}

#[test]
fn inline_union_with_ref_and_primitive_branches_is_typed() {
    let (out, _) = render_with_warnings(
        r##"{
        "openapi": "3.1.0", "info": { "title": "Test", "version": "1.0.0" }, "paths": {},
        "components": { "schemas": {
            "Cat": { "type": "object", "required": ["name"], "properties": { "name": { "type": "string" } } },
            "Mixed": { "oneOf": [
                { "$ref": "#/components/schemas/Cat" },
                { "type": "object", "required": ["bark"], "properties": { "bark": { "type": "boolean" } } },
                { "type": "string" }
            ] }
        } }
    }"##,
    );
    assert!(out.contains("pub struct MixedBark {"), "{out}");
    assert!(out.contains("pub enum Mixed {"), "{out}");
    assert!(out.contains("    Cat(Cat),\n"), "{out}");
    assert!(out.contains("    MixedBark(MixedBark),\n"), "{out}");
    assert!(out.contains("    String(String),\n"), "{out}");
}

#[test]
fn inline_union_branch_without_required_stays_value() {
    let (out, warnings) = render_with_warnings(
        r##"{
        "openapi": "3.1.0", "info": { "title": "Test", "version": "1.0.0" }, "paths": {},
        "components": { "schemas": { "Gap": { "oneOf": [
            { "type": "object", "required": ["a"], "properties": { "a": { "type": "string" } } },
            { "type": "object", "properties": { "b": { "type": "string" } } }
        ] } } }
    }"##,
    );
    assert!(out.contains("pub type Gap = serde_json::Value;"), "{out}");
    assert!(warnings.iter().any(|w| w.contains("`Gap`")), "{warnings:?}");
}

#[test]
fn union_without_inline_object_is_unchanged() {
    let (out, _) = render_with_warnings(UNIONS);
    assert!(
        out.contains("#[serde(untagged)]\npub enum Shape {\n    Cat(Cat),\n    Dog(Dog),\n}"),
        "{out}"
    );
    assert!(!out.contains("Variant1"), "{out}");
}

const INLINE_A: &str = r##"{"type":"object","required":["id","x"],"properties":{"id":{"type":"string"},"x":{"type":"string"}}}"##;

/// Each body must leave the union as `serde_json::Value` with one warning.
fn assert_inline_union_stays_value(body: &str) {
    let json = r##"{
        "openapi": "3.1.0", "info": { "title": "Test", "version": "1.0.0" }, "paths": {},
        "components": { "schemas": {
            "Label": { "type": "string", "enum": ["x"] },
            "Cat": { "type": "object", "required": ["name"], "properties": { "name": { "type": "string" } } },
            "Odd": { "oneOf": [BODY] }
        } }
    }"##
    .replace("BODY", body);
    let (out, warnings) = render_with_warnings(&json);
    assert!(out.contains("pub type Odd = serde_json::Value;"), "{out}");
    assert!(warnings.iter().any(|w| w.contains("`Odd`")), "{warnings:?}");
}

#[test]
fn inline_union_repeated_primitive_stays_value() {
    assert_inline_union_stays_value(&format!(
        r#"{INLINE_A}, {{"type":"string"}}, {{"type":"string"}}"#
    ));
}

#[test]
fn inline_union_integer_and_number_stay_value() {
    assert_inline_union_stays_value(&format!(
        r#"{INLINE_A}, {{"type":"integer"}}, {{"type":"number"}}"#
    ));
}

#[test]
fn inline_union_non_struct_ref_stays_value() {
    assert_inline_union_stays_value(&format!(
        r##"{INLINE_A}, {{"$ref":"#/components/schemas/Label"}}"##
    ));
}

#[test]
fn inline_union_repeated_ref_stays_value() {
    assert_inline_union_stays_value(&format!(
        r##"{INLINE_A}, {{"$ref":"#/components/schemas/Cat"}}, {{"$ref":"#/components/schemas/Cat"}}"##
    ));
}

#[test]
fn inline_union_bare_object_branch_stays_value() {
    assert_inline_union_stays_value(&format!(r#"{INLINE_A}, {{"type":"object"}}"#));
}

#[test]
fn inline_union_subset_required_stays_value() {
    assert_inline_union_stays_value(&format!(
        r#"{{"type":"object","required":["id"],"properties":{{"id":{{"type":"string"}},"y":{{"type":"string"}}}}}}, {INLINE_A}"#
    ));
}

const TAG_ROWS: &str = r##"{"type":"object","required":["kind","rows"],"properties":{"kind":{"type":"string","enum":["rows"]},"rows":{"type":"array","items":{"type":"string"}}}}"##;
const TAG_TOTAL: &str = r##"{"type":"object","required":["kind","total"],"properties":{"kind":{"type":"string","enum":["total"]},"total":{"type":"integer"}}}"##;

fn tagged_spec(branches: &str) -> String {
    format!(
        r##"{{
        "openapi": "3.1.0", "info": {{ "title": "Test", "version": "1.0.0" }}, "paths": {{}},
        "components": {{ "schemas": {{
            "Cat": {{ "type": "object", "required": ["name"], "properties": {{ "name": {{ "type": "string" }} }} }},
            "Node": {{ "oneOf": [{branches}], "discriminator": {{ "propertyName": "kind" }} }}
        }} }}
    }}"##
    )
}

#[test]
fn inline_tagged_union_is_internally_tagged_enum() {
    let (out, warnings) = render_with_warnings(&tagged_spec(&format!("{TAG_ROWS}, {TAG_TOTAL}")));
    assert!(out.contains("pub struct NodeVariant1 {"), "{out}");
    assert!(out.contains("    pub rows: Vec<String>,\n"), "{out}");
    assert!(out.contains("pub struct NodeVariant2 {"), "{out}");
    assert!(out.contains("    pub total: i64,\n"), "{out}");
    assert!(!out.contains("pub kind:"), "{out}");
    assert!(
        out.contains(
            "#[serde(tag = \"kind\")]\npub enum Node {\n    #[serde(rename = \"rows\")]\n    NodeVariant1(NodeVariant1),\n    #[serde(rename = \"total\")]\n    NodeVariant2(NodeVariant2),\n}\n"
        ),
        "{out}"
    );
    assert!(!out.contains("pub type Node = serde_json::Value;"), "{out}");
    assert!(out.find("pub struct NodeVariant1").unwrap() < out.find("pub enum Node").unwrap());
    assert!(warnings.is_empty(), "{warnings:?}");
}

#[test]
fn inline_tagged_union_with_one_required_uses_property_name() {
    let rows = r##"{"type":"object","required":["rows"],"properties":{"kind":{"type":"string","enum":["rows"]},"rows":{"type":"array","items":{"type":"string"}}}}"##;
    let total = r##"{"type":"object","required":["total"],"properties":{"kind":{"type":"string","enum":["total"]},"total":{"type":"integer"}}}"##;
    let (out, _) = render_with_warnings(&tagged_spec(&format!("{rows}, {total}")));
    assert!(out.contains("pub struct NodeRows {"), "{out}");
    assert!(out.contains("pub struct NodeTotal {"), "{out}");
    assert!(
        out.contains("    #[serde(rename = \"rows\")]\n    NodeRows(NodeRows),"),
        "{out}"
    );
    assert!(!out.contains("pub kind:"), "{out}");
}

#[test]
fn inline_tagged_union_mixes_inline_and_ref_branches() {
    let (out, _) = render_with_warnings(&tagged_spec(&format!(
        r##"{{"$ref":"#/components/schemas/Cat"}}, {TAG_TOTAL}"##
    )));
    assert!(
        out.contains("#[serde(tag = \"kind\")]\npub enum Node {"),
        "{out}"
    );
    assert!(
        out.contains("    #[serde(rename = \"Cat\")]\n    Cat(Cat),"),
        "{out}"
    );
    assert!(
        out.contains("    #[serde(rename = \"total\")]\n    NodeVariant2(NodeVariant2),"),
        "{out}"
    );
    assert!(!out.contains("pub kind:"), "{out}");
}

/// Each body must leave the tagged union as `serde_json::Value` with one warning.
fn assert_tagged_inline_stays_value(body: &str) {
    let json = r##"{
        "openapi": "3.1.0", "info": { "title": "Test", "version": "1.0.0" }, "paths": {},
        "components": { "schemas": {
            "Label": { "type": "string", "enum": ["x"] },
            "Cat": { "type": "object", "required": ["name"], "properties": { "name": { "type": "string" } } },
            "Tagged": { "type": "object", "required": ["kind"], "properties": { "kind": { "type": "string" } } },
            "Odd": { "oneOf": [BODY], "discriminator": { "propertyName": "kind" } }
        } }
    }"##
    .replace("BODY", body);
    let (out, warnings) = render_with_warnings(&json);
    assert!(out.contains("pub type Odd = serde_json::Value;"), "{out}");
    assert!(warnings.iter().any(|w| w.contains("`Odd`")), "{warnings:?}");
}

const TAG_A1: &str = r##"{"type":"object","required":["rows"],"properties":{"kind":{"type":"string","enum":["rows"]},"rows":{"type":"string"}}}"##;

#[test]
fn inline_tagged_union_without_tag_enum_stays_value() {
    assert_tagged_inline_stays_value(&format!(
        r#"{TAG_A1}, {{"type":"object","required":["total"],"properties":{{"total":{{"type":"integer"}}}}}}"#
    ));
}

#[test]
fn inline_tagged_union_with_two_value_enum_stays_value() {
    assert_tagged_inline_stays_value(&format!(
        r#"{TAG_A1}, {{"type":"object","required":["total"],"properties":{{"kind":{{"type":"string","enum":["a","b"]}},"total":{{"type":"integer"}}}}}}"#
    ));
}

#[test]
fn inline_tagged_union_with_repeated_tag_stays_value() {
    assert_tagged_inline_stays_value(&format!(r#"{TAG_A1}, {TAG_A1}"#));
}

#[test]
fn inline_tagged_union_with_primitive_branch_stays_value() {
    assert_tagged_inline_stays_value(&format!(r#"{TAG_A1}, {{"type":"string"}}"#));
}

#[test]
fn inline_tagged_union_ref_declaring_tag_stays_value() {
    assert_tagged_inline_stays_value(&format!(
        r##"{TAG_A1}, {{"$ref":"#/components/schemas/Tagged"}}"##
    ));
}

const TRACK_TOTAL_MODE: &str = r##""TrackTotalMode":{"type":"string","enum":["exact","bounded"]}"##;
const TRACK_TOTAL: &str = r##""TrackTotal":{"oneOf":[
  {"type":"object","required":["mode"],"properties":{"mode":{"$ref":"#/components/schemas/TrackTotalMode"}}},
  {"type":"object","required":["limit"],"properties":{"limit":{"type":"integer"}}}]}"##;
const QUERY_NODE: &str = r##""QueryNode":{"oneOf":[
  {"type":"object","required":["not"],"properties":{"not":{"$ref":"#/components/schemas/QueryNode"}}},
  {"type":"object","required":["term"],"properties":{"term":{"type":"string"}}}]}"##;

fn components(schemas: &[&str]) -> String {
    format!(
        r#"{{"paths":{{}},"components":{{"schemas":{{{}}}}}}}"#,
        schemas.join(",")
    )
}

#[test]
fn inline_branch_name_avoids_component_name() {
    let out = render(&components(&[TRACK_TOTAL_MODE, TRACK_TOTAL]));
    assert_eq!(out.matches("pub enum TrackTotalMode {").count(), 1);
    assert!(!out.contains("pub struct TrackTotalMode {"));
    assert!(out.contains("pub struct TrackTotalModeInline {"));
    assert!(out.contains("TrackTotalModeInline(TrackTotalModeInline),"));
}

#[test]
fn inline_branch_name_avoids_inline_suffix_collision() {
    let out = render(&components(&[
        TRACK_TOTAL_MODE,
        r##""TrackTotalModeInline":{"type":"string","enum":["a"]}"##,
        TRACK_TOTAL,
    ]));
    assert!(out.contains("pub struct TrackTotalModeInline2 {"));
    assert!(out.contains("TrackTotalModeInline2(TrackTotalModeInline2),"));
}

#[test]
fn inline_branch_without_collision_keeps_name() {
    let out = render(&components(&[r##""Outcome":{"oneOf":[
  {"type":"object","required":["rows"],"properties":{"rows":{"type":"array","items":{"type":"string"}}}},
  {"type":"object","required":["total"],"properties":{"total":{"type":"integer"}}}]}"##]));
    assert!(out.contains("pub struct OutcomeRows {"));
    assert!(out.contains("pub struct OutcomeTotal {"));
}

#[test]
fn recursive_inline_branch_field_is_boxed() {
    let out = render(&components(&[QUERY_NODE]));
    assert!(out.contains("    pub not: Box<QueryNode>,\n"));
    assert!(out.contains("    pub term: String,\n"));
}

#[test]
fn recursive_struct_fields_are_boxed_but_vec_is_not() {
    let out = render(&components(&[
        r##""Tree":{"type":"object","required":["name"],"properties":{
  "name":{"type":"string"},
  "next":{"$ref":"#/components/schemas/Tree"},
  "kids":{"type":"array","items":{"$ref":"#/components/schemas/Tree"}}}}"##,
    ]));
    assert!(out.contains("pub next: Option<Box<Tree>>,"));
    assert!(out.contains("pub kids: Option<Vec<Tree>>,"));
}

#[test]
fn mutual_recursion_is_boxed_on_both_sides() {
    let out = render(&components(&[
        r##""A":{"type":"object","required":["b"],"properties":{"b":{"$ref":"#/components/schemas/B"}}}"##,
        r##""B":{"type":"object","required":["a"],"properties":{"a":{"$ref":"#/components/schemas/A"}}}"##,
    ]));
    assert!(out.contains("pub b: Box<B>,"));
    assert!(out.contains("pub a: Box<A>,"));
}
