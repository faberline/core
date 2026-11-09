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
