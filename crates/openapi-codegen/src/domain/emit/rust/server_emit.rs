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
        stubs.push_str(&format!("#[derive(Debug)]\npub struct {error};\n\n"));

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
    fn each_operation_has_a_stub_error() {
        let (text, _) = run(SPEC);
        for stub in [
            "pub struct ListPetsError;",
            "pub struct CreatePetError;",
            "pub struct GetPetByIdError;",
            "pub struct DeletePetError;",
        ] {
            assert!(text.contains(stub), "missing stub: {stub}\n{text}");
        }
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
