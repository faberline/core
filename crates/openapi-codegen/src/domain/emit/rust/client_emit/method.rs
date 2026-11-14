use super::literal::esc;
use crate::domain::emit::rust::rsmap;
use crate::domain::ir::names::{to_pascal, to_snake, NameRegistry};
use crate::domain::ir::openapi::RefOr;
use crate::domain::ir::operations::{MediaKind, OperationIR, ParamIR};
use crate::domain::ir::typemap::TypeMap;

pub(in crate::domain::emit::rust) fn fn_name(ir: &OperationIR) -> String {
    match &ir.operation_id {
        Some(id) if !id.trim().is_empty() => to_snake(id),
        _ => to_snake(&format!(
            "{}_{}",
            ir.method,
            ir.path.replace(['/', '{', '}'], "_")
        )),
    }
}

pub(in crate::domain::emit::rust) fn param_ann(
    p: &ParamIR,
    tm: &TypeMap,
) -> (String, String, bool) {
    let ty = p
        .schema
        .as_ref()
        .map(|s| rsmap::type_expr(s, tm))
        .unwrap_or_else(|| "String".to_string());
    (to_snake(&p.name), ty, p.required)
}

/// One generated method, plus the error enum it needs (empty when none).
pub(super) struct MethodOut {
    pub method: String,
    pub error_type: String,
}

/// One declared non-success response, as a variant of the operation's error enum.
struct ErrorVariant {
    name: String,
    pattern: String,
    payload: String,
    is_text: bool,
}

/// True when the operation sends or receives NDJSON.
pub(in crate::domain::emit::rust) fn is_ndjson(ir: &OperationIR) -> bool {
    ir.body
        .as_ref()
        .is_some_and(|b| b.media_kind == MediaKind::Ndjson)
        || ir
            .response
            .as_ref()
            .is_some_and(|r| r.media_kind == MediaKind::Ndjson)
}

/// Variants for the declared error responses this emitter can name.
/// Keys that are neither a status code, `4XX`/`5XX` nor `default` get none.
/// An NDJSON operation has none: it returns `ClientResult` and `error_for_status`.
fn error_variants(ir: &OperationIR, tm: &TypeMap) -> Vec<ErrorVariant> {
    if is_ndjson(ir) {
        return Vec::new();
    }
    let mut out = Vec::new();
    for e in &ir.error_responses {
        let key = e.status.as_str();
        let (name, pattern) = if key.eq_ignore_ascii_case("default") {
            ("Default".to_string(), "_".to_string())
        } else if let Ok(code) = key.parse::<u16>() {
            (format!("Status{code}"), code.to_string())
        } else if key.eq_ignore_ascii_case("4XX") {
            ("Status4XX".to_string(), "400..=499".to_string())
        } else if key.eq_ignore_ascii_case("5XX") {
            ("Status5XX".to_string(), "500..=599".to_string())
        } else {
            continue;
        };
        let (payload, is_text) = match &e.schema {
            None => ("String".to_string(), true),
            Some(RefOr::Ref(r)) => (
                tm.resolve_ref(&r.reference)
                    .unwrap_or_else(|| "serde_json::Value".to_string()),
                false,
            ),
            Some(node @ RefOr::Item(_)) => (rsmap::type_expr(node, tm), false),
        };
        out.push(ErrorVariant {
            name,
            pattern,
            payload,
            is_text,
        });
    }
    out
}

/// True when the operation gets a typed error enum (see `error_variants`).
pub(super) fn has_declared_errors(ir: &OperationIR, tm: &TypeMap) -> bool {
    !error_variants(ir, tm).is_empty()
}

fn emit_error_enum(enum_name: &str, variants: &[ErrorVariant]) -> String {
    let mut out = format!("#[derive(Debug)]\npub enum {enum_name} {{\n");
    for v in variants {
        out.push_str(&format!("    {}({}),\n", v.name, v.payload));
    }
    out.push_str("}\n");
    out
}

pub(super) fn emit_method(
    ir: &OperationIR,
    tm: &TypeMap,
    reg: &mut NameRegistry,
    file_bearer_auth: bool,
    managed: bool,
) -> MethodOut {
    let name = reg.unique(&fn_name(ir));
    let variants = error_variants(ir, tm);
    let error_enum = if variants.is_empty() {
        None
    } else {
        Some(format!("{}Error", to_pascal(&name)))
    };

    let path_anns: Vec<_> = ir.path_params.iter().map(|p| param_ann(p, tm)).collect();
    let query_anns: Vec<_> = ir.query_params.iter().map(|p| param_ann(p, tm)).collect();
    let header_anns: Vec<_> = ir.header_params.iter().map(|p| param_ann(p, tm)).collect();

    // Signature: path (required), query, header, body — stable order.
    let mut sig: Vec<String> = Vec::new();
    for (n, t, _) in &path_anns {
        sig.push(format!("{n}: {t}"));
    }
    for (n, t, req) in query_anns.iter().chain(header_anns.iter()) {
        sig.push(format!(
            "{n}: {}",
            if *req { t.clone() } else { rsmap::optional(t) }
        ));
    }
    let body_ty = ir
        .body
        .as_ref()
        .filter(|b| b.media_kind == MediaKind::Json)
        .map(|b| rsmap::type_expr(&b.schema, tm));
    let ndjson_body = ir
        .body
        .as_ref()
        .filter(|b| b.media_kind == MediaKind::Ndjson)
        .map(|b| rsmap::type_expr(&b.schema, tm));
    if let Some(t) = &body_ty {
        let req = ir.body.as_ref().map(|b| b.required).unwrap_or(false);
        sig.push(format!(
            "body: {}",
            if req { t.clone() } else { rsmap::optional(t) }
        ));
    }
    // An NDJSON body is always taken as an iterator: its records are streamed
    // lazily, so an optional body has no meaning here.
    if let Some(item) = &ndjson_body {
        sig.push(format!(
            "body: impl Iterator<Item = {item}> + Send + 'static"
        ));
    }
    let ndjson_resp = ir
        .response
        .as_ref()
        .is_some_and(|r| r.media_kind == MediaKind::Ndjson);

    let (ret, ret_stmt) = response(ir, tm);
    let result_ty = match &error_enum {
        Some(enum_name) => format!("ApiResult<{ret}, {enum_name}>"),
        None => format!("ClientResult<{ret}>"),
    };
    let mut m = format!(
        "    pub fn {name}(&self{}) -> {result_ty} {{\n",
        if sig.is_empty() {
            String::new()
        } else {
            format!(", {}", sig.join(", "))
        }
    );

    // URL with path params substituted into a format string.
    let mut fmt = ir.path.clone();
    let mut args: Vec<String> = Vec::new();
    for (p, (snake, ..)) in ir.path_params.iter().zip(&path_anns) {
        fmt = fmt.replace(&format!("{{{}}}", p.name), "{}");
        args.push(snake.clone());
    }
    let arg_tail = if args.is_empty() {
        String::new()
    } else {
        format!(", {}", args.join(", "))
    };
    m.push_str(&format!(
        "        let url = format!(\"{{}}{fmt}\", self.base_url{arg_tail});\n"
    ));

    if file_bearer_auth || managed {
        m.push_str("        let mut request_headers = self.default_headers.clone();\n");
        for (p, (snake, _, req)) in ir.header_params.iter().zip(&header_anns) {
            let header_name = esc(&p.name);
            if *req {
                m.push_str(&format!(
                    "        insert_request_header(&mut request_headers, \"{header_name}\", &{snake}.to_string())?;\n"
                ));
            } else {
                m.push_str(&format!(
                    "        if let Some(v) = &{snake} {{ insert_request_header(&mut request_headers, \"{header_name}\", &v.to_string())?; }}\n"
                ));
            }
        }
        if managed {
            m.push_str(
                "        attach_managed(&self.base_url, &self.ca_path, &mut request_headers)?;\n",
            );
        } else {
            m.push_str("        attach_file_bearer(&self.base_url, &mut request_headers)?;\n");
        }
    }

    if ir.method == "query" {
        // OpenAPI 3.2 HTTP QUERY (RFC 10008): `reqwest::blocking::Client` has no
        // dedicated `.query()` verb method (that name is the querystring
        // builder), so build the method via `Method::from_bytes`. Epic #1296
        // POST-twin fallback: when `self.use_post_fallback` is set, send POST
        // against the documented twin path instead.
        match &ir.post_twin_path {
            Some(twin_path) => {
                let mut twin_fmt = twin_path.clone();
                for p in &ir.path_params {
                    twin_fmt = twin_fmt.replace(&format!("{{{}}}", p.name), "{}");
                }
                m.push_str(&format!(
                    "        let mut req = if self.use_post_fallback {{\n            let twin_url = format!(\"{{}}{twin_fmt}\", self.base_url{arg_tail});\n            self.http.post(twin_url)\n        }} else {{\n            self.http.request(reqwest::Method::from_bytes(b\"QUERY\").expect(\"valid HTTP method\"), url)\n        }};\n"
                ));
            }
            None => {
                m.push_str("        let mut req = self.http.request(reqwest::Method::from_bytes(b\"QUERY\").expect(\"valid HTTP method\"), url);\n");
            }
        }
    } else {
        let method = ir.method.as_str();
        m.push_str(&format!("        let mut req = self.http.{method}(url);\n"));
    }

    if !ir.query_params.is_empty() {
        m.push_str("        let mut q: Vec<(&str, String)> = Vec::new();\n");
        for (p, (snake, _, req)) in ir.query_params.iter().zip(&query_anns) {
            if *req {
                m.push_str(&format!(
                    "        q.push((\"{}\", {snake}.to_string()));\n",
                    esc(&p.name)
                ));
            } else {
                m.push_str(&format!(
                    "        if let Some(v) = &{snake} {{ q.push((\"{}\", v.to_string())); }}\n",
                    esc(&p.name)
                ));
            }
        }
        m.push_str("        req = req.query(&q);\n");
    }
    if file_bearer_auth || managed {
        m.push_str("        req = req.headers(request_headers);\n");
    } else {
        for (p, (snake, _, req)) in ir.header_params.iter().zip(&header_anns) {
            if *req {
                m.push_str(&format!(
                    "        req = req.header(\"{}\", {snake}.to_string());\n",
                    esc(&p.name)
                ));
            } else {
                m.push_str(&format!(
                    "        if let Some(v) = &{snake} {{ req = req.header(\"{}\", v.to_string()); }}\n",
                    esc(&p.name)
                ));
            }
        }
    }
    if body_ty.is_some() {
        m.push_str("        req = req.json(&body);\n");
    }
    if ndjson_body.is_some() {
        m.push_str("        req = req.header(\"Content-Type\", \"application/x-ndjson\").body(reqwest::blocking::Body::new(NdjsonBodyReader::new(body)));\n");
    }
    if ndjson_resp {
        m.push_str("        req = req.header(\"Accept\", \"application/x-ndjson\");\n");
    }

    // An NDJSON response keeps the admission slot for as long as its iterator lives.
    let slot = if ndjson_resp { "slot" } else { "_slot" };
    m.push_str(&format!("        let {slot} = self.acquire_slot()?;\n"));
    match &error_enum {
        None => m.push_str("        let resp = req.send()?.error_for_status()?;\n"),
        Some(enum_name) => {
            m.push_str("        let resp = req.send()?;\n");
            m.push_str(
                "        if resp.status().is_client_error() || resp.status().is_server_error() {\n",
            );
            m.push_str("            return match resp.status().as_u16() {\n");
            let has_default = variants.iter().any(|v| v.pattern == "_");
            for v in &variants {
                let decode = if v.is_text {
                    "resp.text()?"
                } else {
                    "resp.json()?"
                };
                m.push_str(&format!(
                    "                {} => Err(ApiError::Api({enum_name}::{}({decode}))),\n",
                    v.pattern, v.name
                ));
            }
            if !has_default {
                m.push_str(
                    "                _ => Err(resp.error_for_status().expect_err(\"4xx or 5xx status\").into()),\n",
                );
            }
            m.push_str("            };\n");
            m.push_str("        }\n");
        }
    }
    m.push_str(&format!("        {ret_stmt}\n"));
    m.push_str("    }\n");
    let error_type = match &error_enum {
        Some(enum_name) => emit_error_enum(enum_name, &variants),
        None => String::new(),
    };
    MethodOut {
        method: m,
        error_type,
    }
}

/// `(return type, return statement)` for the operation's response.
pub(in crate::domain::emit::rust) fn response(ir: &OperationIR, tm: &TypeMap) -> (String, String) {
    match &ir.response {
        Some(resp) if resp.media_kind == MediaKind::Ndjson => {
            let item = match &resp.schema {
                RefOr::Ref(r) => tm
                    .resolve_ref(&r.reference)
                    .unwrap_or_else(|| "serde_json::Value".to_string()),
                node @ RefOr::Item(_) => rsmap::type_expr(node, tm),
            };
            (
                format!("NdjsonRecords<{item}>"),
                "Ok(NdjsonRecords::new(resp, slot))".to_string(),
            )
        }
        Some(resp) if resp.media_kind == MediaKind::Json => match &resp.schema {
            RefOr::Ref(r) => {
                let ty = tm
                    .resolve_ref(&r.reference)
                    .unwrap_or_else(|| "serde_json::Value".to_string());
                (ty, "Ok(resp.json()?)".to_string())
            }
            node @ RefOr::Item(_) => (rsmap::type_expr(node, tm), "Ok(resp.json()?)".to_string()),
        },
        _ => ("()".to_string(), "let _ = resp; Ok(())".to_string()),
    }
}
