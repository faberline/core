//! Rust server skeleton emitter. #323 adds the handler trait, #324 the error types, #325 the router.

use super::client_emit::{fn_name, is_ndjson, param_ann, response};
use super::models_emit::HEADER;
use super::rsmap;
use crate::domain::ir::names::{to_pascal, NameRegistry};
use crate::domain::ir::operations::{MediaKind, OperationIR};
use crate::domain::ir::typemap::TypeMap;

/// Contents of the generated `server.rs`: the generated header, then one
/// handler trait with one async method per operation.
pub fn emit(ops: &[OperationIR], tm: &TypeMap, client_name: &str) -> (String, Vec<String>) {
    let mut out = String::from(HEADER);
    let mut warnings = Vec::new();
    let mut reg = NameRegistry::new();
    let mut stubs = String::new();
    let mut methods = String::new();

    out.push_str("\n#[allow(unused_imports)]\nuse crate::models::*;\n\n");

    for ir in ops {
        let name = reg.unique(&fn_name(ir));
        if is_ndjson(ir) {
            warnings.push(format!(
                "server: operation {name} uses NDJSON and is skipped"
            ));
            continue;
        }
        let op = to_pascal(&name);
        let error = format!("{op}Error");
        stubs.push_str(&emit_error_type(&error, &error_variants(ir)));

        let mut args = String::new();
        for p in &ir.path_params {
            let (snake, ty, _) = param_ann(p, tm);
            args.push_str(&format!(", {snake}: {ty}"));
        }
        for p in &ir.query_params {
            let (snake, ty, required) = param_ann(p, tm);
            if required {
                args.push_str(&format!(", {snake}: {ty}"));
            } else {
                args.push_str(&format!(", {snake}: {}", rsmap::optional(&ty)));
            }
        }
        if let Some(b) = &ir.body {
            if b.media_kind == MediaKind::Json {
                let ty = rsmap::type_expr(&b.schema, tm);
                if b.required {
                    args.push_str(&format!(", body: {ty}"));
                } else {
                    args.push_str(&format!(", body: {}", rsmap::optional(&ty)));
                }
            }
        }

        let (resp_ty, _) = response(ir, tm);
        methods.push_str(&format!("    /// {} {}\n", ir.http_method, ir.path));
        methods.push_str(&format!(
            "    fn {name}(&self{args}) -> impl std::future::Future<Output = Result<{resp_ty}, {error}>> + Send;\n"
        ));
    }

    out.push_str(&stubs);
    out.push_str(&format!(
        "pub trait {}Handler: Send + Sync + 'static {{\n",
        to_pascal(client_name)
    ));
    out.push_str(&methods);
    out.push_str("}\n");
    (out, warnings)
}

/// One variant of a generated per-operation error enum.
struct ErrorVariant {
    name: String,
    kind: VariantKind,
}

/// How a variant maps its declared response key to an HTTP status.
enum VariantKind {
    /// A numeric key such as `404`: the status is fixed.
    Exact(u16),
    /// `4XX` or `5XX`: the status is carried in the value.
    Range,
    /// `default`: the status is carried in the value.
    Default,
}

/// The variants of an operation's error enum, in `error_responses` order.
/// Keys that are not a status, a range or `default` are skipped, and a
/// repeated name (`4XX` and `4xx`) gives one variant.
fn error_variants(ir: &OperationIR) -> Vec<ErrorVariant> {
    let mut out: Vec<ErrorVariant> = Vec::new();
    for e in &ir.error_responses {
        let key = e.status.as_str();
        let (name, kind) = if key.eq_ignore_ascii_case("default") {
            ("Default".to_string(), VariantKind::Default)
        } else if let Ok(code) = key.parse::<u16>() {
            (format!("Status{code}"), VariantKind::Exact(code))
        } else if key.eq_ignore_ascii_case("4XX") {
            ("Status4XX".to_string(), VariantKind::Range)
        } else if key.eq_ignore_ascii_case("5XX") {
            ("Status5XX".to_string(), VariantKind::Range)
        } else {
            continue;
        };
        if out.iter().all(|v| v.name != name) {
            out.push(ErrorVariant { name, kind });
        }
    }
    out
}

/// The error enum `type_name` with its `From<ApiErr>` and `IntoResponse`
/// impls. The `Other` catch-all is always last. No `#[derive]`:
/// `service_http::ApiErr` does not implement `Debug`.
fn emit_error_type(type_name: &str, variants: &[ErrorVariant]) -> String {
    let mut out = format!("pub enum {type_name} {{\n");
    for v in variants {
        match v.kind {
            VariantKind::Exact(code) => {
                out.push_str(&format!("    Status{code}(service_http::ErrorEnvelope),\n"));
            }
            VariantKind::Range | VariantKind::Default => {
                let name = &v.name;
                out.push_str(&format!(
                    "    {name}(axum::http::StatusCode, service_http::ErrorEnvelope),\n"
                ));
            }
        }
    }
    out.push_str("    Other(service_http::ApiErr),\n}\n\n");

    out.push_str(&format!(
        "impl From<service_http::ApiErr> for {type_name} {{\n    fn from(e: service_http::ApiErr) -> Self {{\n        Self::Other(e)\n    }}\n}}\n\n"
    ));

    out.push_str(&format!(
        "impl axum::response::IntoResponse for {type_name} {{\n    fn into_response(self) -> axum::response::Response {{\n        match self {{\n"
    ));
    for v in variants {
        let name = &v.name;
        match v.kind {
            VariantKind::Exact(code) => out.push_str(&format!(
                "            Self::{name}(e) => axum::response::IntoResponse::into_response((axum::http::StatusCode::from_u16({code}).unwrap_or(axum::http::StatusCode::INTERNAL_SERVER_ERROR), axum::Json(e))),\n"
            )),
            VariantKind::Range | VariantKind::Default => out.push_str(&format!(
                "            Self::{name}(s, e) => axum::response::IntoResponse::into_response((s, axum::Json(e))),\n"
            )),
        }
    }
    out.push_str(
        "            Self::Other(e) => axum::response::IntoResponse::into_response(e),\n        }\n    }\n}\n\n",
    );
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::ir::build_type_map;
    use crate::domain::ir::openapi::Spec;
    use crate::domain::ir::operations;

    const SPEC: &str = r##"{
      "openapi": "3.0.3",
      "info": { "title": "Pets", "version": "1.0.0" },
      "paths": {
        "/pets": {
          "get": {
            "operationId": "listPets",
            "parameters": [{ "name": "limit", "in": "query", "required": false, "schema": { "type": "integer" } }],
            "responses": {
              "200": { "description": "ok", "content": { "application/json": { "schema": { "type": "array", "items": { "$ref": "#/components/schemas/Pet" } } } } }
            }
          },
          "post": {
            "operationId": "createPet",
            "requestBody": { "required": true, "content": { "application/json": { "schema": { "$ref": "#/components/schemas/Pet" } } } },
            "responses": {
              "201": { "description": "created", "content": { "application/json": { "schema": { "$ref": "#/components/schemas/Pet" } } } },
              "404": { "description": "missing", "content": { "application/json": { "schema": { "$ref": "#/components/schemas/ErrorEnvelope" } } } }
            }
          }
        },
        "/pets/{petId}": {
          "get": {
            "operationId": "getPetById",
            "parameters": [{ "name": "petId", "in": "path", "required": true, "schema": { "type": "integer" } }],
            "responses": {
              "200": { "description": "ok", "content": { "application/json": { "schema": { "$ref": "#/components/schemas/Pet" } } } },
              "404": { "description": "missing", "content": { "application/json": { "schema": { "$ref": "#/components/schemas/ErrorEnvelope" } } } }
            }
          },
          "delete": {
            "operationId": "deletePet",
            "parameters": [{ "name": "petId", "in": "path", "required": true, "schema": { "type": "integer" } }],
            "responses": { "204": { "description": "deleted" } }
          }
        }
      },
      "components": {
        "schemas": {
          "Pet": { "type": "object", "properties": { "id": { "type": "integer" }, "name": { "type": "string" } }, "required": ["id", "name"] },
          "ErrorEnvelope": { "type": "object", "properties": { "error": { "type": "string" }, "message": { "type": "string" } }, "required": ["error", "message"] }
        }
      }
    }"##;

    fn run(spec_json: &str) -> (String, Vec<String>) {
        let spec: Spec = serde_json::from_str(spec_json).expect("parse spec");
        let tm = build_type_map(&spec);
        let ops = operations::build(&spec);
        emit(&ops, &tm, "createClient")
    }

    #[test]
    fn trait_has_one_method_per_operation() {
        let (text, _) = run(SPEC);
        for line in [
            "pub trait CreateClientHandler: Send + Sync + 'static {",
            "    fn list_pets(&self, limit: Option<i64>) -> impl std::future::Future<Output = Result<Vec<Pet>, ListPetsError>> + Send;",
            "    fn create_pet(&self, body: Pet) -> impl std::future::Future<Output = Result<Pet, CreatePetError>> + Send;",
            "    fn get_pet_by_id(&self, pet_id: i64) -> impl std::future::Future<Output = Result<Pet, GetPetByIdError>> + Send;",
            "    fn delete_pet(&self, pet_id: i64) -> impl std::future::Future<Output = Result<(), DeletePetError>> + Send;",
        ] {
            assert!(text.contains(line), "missing line: {line}\n{text}");
        }
        assert_eq!(text.matches("    fn ").count(), 4);
    }

    #[test]
    fn each_operation_has_an_error_type() {
        let (text, _) = run(SPEC);
        for ty in [
            "pub enum ListPetsError {",
            "pub enum CreatePetError {",
            "pub enum GetPetByIdError {",
            "pub enum DeletePetError {",
        ] {
            assert!(text.contains(ty), "missing error type: {ty}\n{text}");
        }
        assert!(!text.contains("pub struct ListPetsError;"), "{text}");
    }

    #[test]
    fn declared_404_gets_a_variant_and_its_status() {
        let (text, _) = run(SPEC);
        assert!(
            text.contains("    Status404(service_http::ErrorEnvelope),"),
            "{text}"
        );
        assert!(
            text.contains("Self::Status404(e) => axum::response::IntoResponse::into_response((axum::http::StatusCode::from_u16(404).unwrap_or(axum::http::StatusCode::INTERNAL_SERVER_ERROR), axum::Json(e))),"),
            "{text}"
        );
    }

    #[test]
    fn operation_without_declared_errors_has_only_other() {
        let (text, _) = run(SPEC);
        let start = text
            .find("pub enum DeletePetError {")
            .expect("DeletePetError enum");
        let rest = &text[start..];
        let end = rest.find('}').expect("enum closing brace") + 1;
        assert_eq!(
            &rest[..end],
            "pub enum DeletePetError {\n    Other(service_http::ApiErr),\n}"
        );
    }

    fn get_thing_spec(responses: &str) -> String {
        format!(
            r##"{{
  "openapi": "3.0.3",
  "info": {{ "title": "Things", "version": "1.0.0" }},
  "paths": {{
    "/things/{{id}}": {{
      "get": {{
        "operationId": "getThing",
        "parameters": [{{ "name": "id", "in": "path", "required": true, "schema": {{ "type": "integer" }} }}],
        "responses": {{ {responses} }}
      }}
    }}
  }},
  "components": {{
    "schemas": {{
      "Thing": {{ "type": "object", "properties": {{ "id": {{ "type": "integer" }} }}, "required": ["id"] }},
      "ErrorEnvelope": {{ "type": "object", "properties": {{ "error": {{ "type": "string" }}, "message": {{ "type": "string" }} }}, "required": ["error", "message"] }}
    }}
  }}
}}"##
        )
    }

    const ERROR_BODY: &str = r##"{ "description": "e", "content": { "application/json": { "schema": { "$ref": "#/components/schemas/ErrorEnvelope" } } } }"##;

    #[test]
    fn status_mapping_uses_the_declared_code() {
        // Negative control: the code must come from the declared response, not a fixed 404.
        let spec = get_thing_spec(&format!(
            r#""200": {{ "description": "ok" }}, "409": {ERROR_BODY}"#
        ));
        let (text, _) = run(&spec);
        assert!(text.contains("from_u16(409)"), "{text}");
        assert!(!text.contains("from_u16(404)"), "{text}");
    }

    #[test]
    fn range_and_default_variants_carry_the_status_code() {
        let spec = get_thing_spec(&format!(
            r#""200": {{ "description": "ok" }}, "4XX": {ERROR_BODY}, "5XX": {ERROR_BODY}, "default": {ERROR_BODY}"#
        ));
        let (text, _) = run(&spec);
        for line in [
            "    Status4XX(axum::http::StatusCode, service_http::ErrorEnvelope),",
            "    Status5XX(axum::http::StatusCode, service_http::ErrorEnvelope),",
            "    Default(axum::http::StatusCode, service_http::ErrorEnvelope),",
        ] {
            assert!(text.contains(line), "missing line: {line}\n{text}");
        }
        let four = text.find("    Status4XX(").expect("Status4XX");
        let five = text.find("    Status5XX(").expect("Status5XX");
        let default = text.find("    Default(").expect("Default");
        assert!(four < five && five < default, "{text}");
    }

    #[test]
    fn duplicate_range_keys_give_one_variant() {
        let spec = get_thing_spec(&format!(
            r#""200": {{ "description": "ok" }}, "4XX": {ERROR_BODY}, "4xx": {ERROR_BODY}"#
        ));
        let (text, _) = run(&spec);
        assert_eq!(text.matches("    Status4XX(").count(), 1, "{text}");
    }

    #[test]
    fn removing_an_operation_removes_its_method() {
        // Negative control: the same spec without `deletePet`.
        let without_delete = SPEC.replace(
            r#""delete": {
            "operationId": "deletePet",
            "parameters": [{ "name": "petId", "in": "path", "required": true, "schema": { "type": "integer" } }],
            "responses": { "204": { "description": "deleted" } }
          }"#,
            "\"x-removed\": true",
        );
        assert_ne!(without_delete, SPEC, "fixture edit must apply");
        let (text, _) = run(&without_delete);
        assert!(!text.contains("fn delete_pet"), "{text}");
        assert!(!text.contains("DeletePetError"), "{text}");
        assert!(text.contains("fn list_pets"), "{text}");
    }

    #[test]
    fn ndjson_operation_is_skipped_with_a_warning() {
        let spec = r##"{
          "openapi": "3.0.3",
          "info": { "title": "Stream", "version": "1.0.0" },
          "paths": {
            "/pets/stream": {
              "get": {
                "operationId": "streamPets",
                "responses": {
                  "200": { "description": "ok", "content": { "application/x-ndjson": { "schema": { "$ref": "#/components/schemas/Pet" } } } }
                }
              }
            }
          },
          "components": {
            "schemas": {
              "Pet": { "type": "object", "properties": { "id": { "type": "integer" } }, "required": ["id"] }
            }
          }
        }"##;
        let (text, warnings) = run(spec);
        assert!(!text.contains("fn stream_pets"), "{text}");
        assert!(!text.contains("StreamPetsError"), "{text}");
        assert_eq!(
            warnings,
            vec!["server: operation stream_pets uses NDJSON and is skipped".to_string()]
        );
    }
}
