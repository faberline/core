//! Router emitter for the Rust server skeleton (#325): one axum route per
//! operation plus the `service-http` standard probe routes.

use super::param_ann;
use crate::domain::emit::rust::rsmap;
use crate::domain::ir::names::to_pascal;
use crate::domain::ir::operations::{MediaKind, OperationIR};
use crate::domain::ir::typemap::TypeMap;

/// HTTP methods that have an axum route. `QUERY` and other methods do not.
const ROUTABLE_METHODS: &[&str] = &[
    "GET", "POST", "PUT", "PATCH", "DELETE", "HEAD", "OPTIONS", "TRACE",
];

/// Text of the query structs, the route functions and `router`, plus warnings.
/// `routes` holds the index in `ops` and the unique name of each non-NDJSON
/// operation, in `ops` order.
pub(super) fn emit_router(
    ops: &[OperationIR],
    tm: &TypeMap,
    routes: &[(usize, String)],
    trait_name: &str,
) -> (String, Vec<String>) {
    let mut warnings = Vec::new();
    let mut routed: Vec<(&OperationIR, &str)> = Vec::new();
    for (index, name) in routes {
        let ir = &ops[*index];
        match unroutable_reason(ir, name) {
            Some(reason) => warnings.push(reason),
            None => routed.push((ir, name.as_str())),
        }
    }

    let mut out = String::new();
    for (ir, name) in &routed {
        if !ir.query_params.is_empty() {
            out.push_str(&query_struct(ir, name, tm));
        }
    }
    for (ir, name) in &routed {
        out.push_str(&route_fn(ir, name, tm, trait_name));
    }
    out.push_str(&router_fn(&routed, trait_name));
    (out, warnings)
}

/// Why an operation has no route, or `None` when it has one.
fn unroutable_reason(ir: &OperationIR, name: &str) -> Option<String> {
    if !ROUTABLE_METHODS.contains(&ir.http_method.as_str()) {
        return Some(format!(
            "server: operation {name} uses method {} and has no route",
            ir.http_method
        ));
    }
    if !path_params_match(ir) {
        return Some(format!(
            "server: operation {name} has path parameters that do not match its path and has no route"
        ));
    }
    None
}

/// The names between the braces of a path template, in template order.
fn template_names(path: &str) -> Vec<String> {
    path.split('{')
        .skip(1)
        .filter_map(|rest| rest.split_once('}').map(|(name, _)| name.to_string()))
        .collect()
}

/// True when every `{name}` of the path is a path parameter and every path
/// parameter appears in the path.
fn path_params_match(ir: &OperationIR) -> bool {
    let names = template_names(&ir.path);
    names
        .iter()
        .all(|n| ir.path_params.iter().any(|p| &p.name == n))
        && ir.path_params.iter().all(|p| names.contains(&p.name))
}

/// `(snake, type)` of each path parameter, in the order of the template.
fn template_bindings(ir: &OperationIR, tm: &TypeMap) -> Vec<(String, String)> {
    template_names(&ir.path)
        .iter()
        .filter_map(|n| ir.path_params.iter().find(|p| &p.name == n))
        .map(|p| {
            let (snake, ty, _) = param_ann(p, tm);
            (snake, ty)
        })
        .collect()
}

/// A tuple of one or more items, with the trailing comma that a one-item
/// tuple needs.
fn tuple(items: &[String]) -> String {
    match items {
        [one] => format!("({one},)"),
        _ => format!("({})", items.join(", ")),
    }
}

/// The query struct of a routed operation, with a blank line after it.
fn query_struct(ir: &OperationIR, name: &str, tm: &TypeMap) -> String {
    let mut out = format!(
        "#[derive(serde::Deserialize)]\nstruct {}Query {{\n",
        to_pascal(name)
    );
    for p in &ir.query_params {
        let (snake, ty, required) = param_ann(p, tm);
        if required {
            out.push_str(&format!(
                "    #[serde(rename = \"{}\")]\n    {snake}: {ty},\n",
                p.name
            ));
        } else {
            out.push_str(&format!(
                "    #[serde(rename = \"{}\", default)]\n    {snake}: {},\n",
                p.name,
                rsmap::optional(&ty)
            ));
        }
    }
    out.push_str("}\n\n");
    out
}

/// The axum route function of a routed operation, with a blank line after it.
fn route_fn(ir: &OperationIR, name: &str, tm: &TypeMap, trait_name: &str) -> String {
    let (params, args) = route_params_and_args(ir, name, tm);
    let ok = if ir.response.is_some() {
        "|v| axum::response::IntoResponse::into_response(axum::Json(v))"
    } else {
        "|()| axum::response::IntoResponse::into_response(axum::http::StatusCode::NO_CONTENT)"
    };
    let error = format!("{}Error", to_pascal(name));
    format!(
        "async fn {name}_route<H: {trait_name}>(\n{params}) -> Result<axum::response::Response, {error}> {{\n    handler.{name}({args}).await.map({ok})\n}}\n\n"
    )
}

/// The extractor lines (each ending in a newline, indented by 4) and the
/// call arguments of a route function.
fn route_params_and_args(ir: &OperationIR, name: &str, tm: &TypeMap) -> (String, String) {
    let mut params = String::from(
        "    axum::extract::State(handler): axum::extract::State<std::sync::Arc<H>>,\n",
    );
    let mut args: Vec<String> = Vec::new();

    let bindings = template_bindings(ir, tm);
    if !bindings.is_empty() {
        let snakes: Vec<String> = bindings.iter().map(|(s, _)| s.clone()).collect();
        let types: Vec<String> = bindings.iter().map(|(_, t)| t.clone()).collect();
        params.push_str(&format!(
            "    axum::extract::Path({}): axum::extract::Path<{}>,\n",
            tuple(&snakes),
            tuple(&types)
        ));
    }
    for p in &ir.path_params {
        args.push(param_ann(p, tm).0);
    }

    if !ir.query_params.is_empty() {
        params.push_str(&format!(
            "    axum::extract::Query(query): axum::extract::Query<{}Query>,\n",
            to_pascal(name)
        ));
        for p in &ir.query_params {
            args.push(format!("query.{}", param_ann(p, tm).0));
        }
    }

    if let Some(body) = ir.body.as_ref().filter(|b| b.media_kind == MediaKind::Json) {
        let ty = rsmap::type_expr(&body.schema, tm);
        if body.required {
            params.push_str(&format!("    axum::Json(body): axum::Json<{ty}>,\n"));
            args.push("body".to_string());
        } else {
            params.push_str(&format!("    body: Option<axum::Json<{ty}>>,\n"));
            args.push("body.map(|b| b.0)".to_string());
        }
    }
    (params, args.join(", "))
}

/// The `router` function: one `.route` per routed operation, the state, and
/// the `service-http` standard probe routes. Ends with a newline.
fn router_fn(routed: &[(&OperationIR, &str)], trait_name: &str) -> String {
    let mut out = format!(
        "/// Routes for every operation plus the standard probe routes of `service-http`.\n\
         pub fn router<H: {trait_name}, R: service_http::ReadinessHook + 'static>(\n\
         \x20   handler: std::sync::Arc<H>,\n\
         \x20   readiness: std::sync::Arc<R>,\n\
         \x20   openapi_json: fn() -> String,\n\
         ) -> axum::Router {{\n\
         \x20   axum::Router::<std::sync::Arc<H>>::new()\n"
    );
    for (ir, name) in routed {
        out.push_str(&format!(
            "        .route(\"{}\", axum::routing::{}({name}_route::<H>))\n",
            ir.path,
            ir.http_method.to_lowercase()
        ));
    }
    out.push_str(
        "        .with_state(handler)\n        .merge(service_http::standard_probe_routes_canonical_json(readiness, None, openapi_json))\n}\n",
    );
    out
}

#[cfg(test)]
mod tests {
    use crate::domain::emit::rust::server_emit::emit as server_text;
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

    /// `SPEC` without `deletePet`.
    const SPEC_WITHOUT_DELETE: &str = r##"{
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
        server_text(&ops, &tm, "createClient")
    }

    #[test]
    fn router_has_one_route_per_operation() {
        let (text, _) = run(SPEC);
        for line in [
            r#"        .route("/pets", axum::routing::get(list_pets_route::<H>))"#,
            r#"        .route("/pets", axum::routing::post(create_pet_route::<H>))"#,
            r#"        .route("/pets/{petId}", axum::routing::get(get_pet_by_id_route::<H>))"#,
            r#"        .route("/pets/{petId}", axum::routing::delete(delete_pet_route::<H>))"#,
        ] {
            assert!(text.contains(line), "missing line: {line}\n{text}");
        }
        assert_eq!(text.matches("        .route(").count(), 4, "{text}");
    }

    #[test]
    fn dropping_an_operation_drops_its_route() {
        let (text, _) = run(SPEC_WITHOUT_DELETE);
        assert!(!text.contains("delete_pet_route"), "{text}");
        assert_eq!(text.matches("        .route(").count(), 3, "{text}");
    }

    #[test]
    fn route_function_extracts_path_query_and_body() {
        let (text, _) = run(SPEC);
        for line in [
            "async fn list_pets_route<H: CreateClientHandler>(",
            "    axum::extract::Query(query): axum::extract::Query<ListPetsQuery>,",
            "    axum::Json(body): axum::Json<Pet>,",
            "    axum::extract::Path((pet_id,)): axum::extract::Path<(i64,)>,",
            "    handler.list_pets(query.limit).await.map(|v| axum::response::IntoResponse::into_response(axum::Json(v)))",
            "    handler.delete_pet(pet_id).await.map(|()| axum::response::IntoResponse::into_response(axum::http::StatusCode::NO_CONTENT))",
        ] {
            assert!(text.contains(line), "missing line: {line}\n{text}");
        }
    }

    #[test]
    fn query_struct_marks_optional_params_with_default() {
        let (text, _) = run(SPEC);
        assert!(
            text.contains(
                "struct ListPetsQuery {\n    #[serde(rename = \"limit\", default)]\n    limit: Option<i64>,\n}"
            ),
            "{text}"
        );
    }

    #[test]
    fn path_extractor_follows_the_template_order() {
        let spec = r##"{
          "openapi": "3.0.3",
          "info": { "title": "AB", "version": "1.0.0" },
          "paths": {
            "/a/{x}/b/{y}": {
              "get": {
                "operationId": "getAB",
                "parameters": [
                  { "name": "y", "in": "path", "required": true, "schema": { "type": "string" } },
                  { "name": "x", "in": "path", "required": true, "schema": { "type": "integer" } }
                ],
                "responses": { "200": { "description": "ok" } }
              }
            }
          }
        }"##;
        let (text, _) = run(spec);
        assert!(
            text.contains("axum::extract::Path((x, y)): axum::extract::Path<(i64, String)>"),
            "{text}"
        );
        assert!(text.contains("handler.get_ab(y, x)"), "{text}");
    }

    #[test]
    fn unsupported_method_gets_a_warning_and_no_route() {
        let spec = r##"{
          "openapi": "3.2.0",
          "info": { "title": "Pets", "version": "1.0.0" },
          "paths": {
            "/pets/search": {
              "query": {
                "operationId": "searchPets",
                "responses": { "200": { "description": "ok" } }
              }
            }
          }
        }"##;
        let (text, warnings) = run(spec);
        assert!(
            warnings
                .iter()
                .any(|w| w == "server: operation search_pets uses method QUERY and has no route"),
            "{warnings:?}"
        );
        assert!(!text.contains("search_pets_route"), "{text}");
    }

    #[test]
    fn router_merges_the_probe_routes() {
        let (text, _) = run(SPEC);
        assert!(
            text.contains(
                ".merge(service_http::standard_probe_routes_canonical_json(readiness, None, openapi_json))"
            ),
            "{text}"
        );
    }
}
