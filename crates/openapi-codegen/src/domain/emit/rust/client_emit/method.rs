use super::literal::esc;
use crate::domain::emit::rust::rsmap;
use crate::domain::ir::names::{to_snake, NameRegistry};
use crate::domain::ir::openapi::RefOr;
use crate::domain::ir::operations::{OperationIR, ParamIR};
use crate::domain::ir::typemap::TypeMap;

fn fn_name(ir: &OperationIR) -> String {
    match &ir.operation_id {
        Some(id) if !id.trim().is_empty() => to_snake(id),
        _ => to_snake(&format!(
            "{}_{}",
            ir.method,
            ir.path.replace(['/', '{', '}'], "_")
        )),
    }
}

fn param_ann(p: &ParamIR, tm: &TypeMap) -> (String, String, bool) {
    let ty = p
        .schema
        .as_ref()
        .map(|s| rsmap::type_expr(s, tm))
        .unwrap_or_else(|| "String".to_string());
    (to_snake(&p.name), ty, p.required)
}

pub(super) fn emit_method(
    ir: &OperationIR,
    tm: &TypeMap,
    reg: &mut NameRegistry,
    file_bearer_auth: bool,
) -> String {
    let name = reg.unique(&fn_name(ir));

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
    let body_ty = ir.body.as_ref().map(|b| rsmap::type_expr(&b.schema, tm));
    if let Some(t) = &body_ty {
        let req = ir.body.as_ref().map(|b| b.required).unwrap_or(false);
        sig.push(format!(
            "body: {}",
            if req { t.clone() } else { rsmap::optional(t) }
        ));
    }

    let (ret, ret_stmt) = response(ir, tm);
    let mut m = format!(
        "    pub fn {name}(&self{}) -> ClientResult<{ret}> {{\n",
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

    if file_bearer_auth {
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
        m.push_str("        attach_file_bearer(&self.base_url, &mut request_headers)?;\n");
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
    if file_bearer_auth {
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

    m.push_str("        let _slot = self.acquire_slot()?;\n");
    m.push_str("        let resp = req.send()?.error_for_status()?;\n");
    m.push_str(&format!("        {ret_stmt}\n"));
    m.push_str("    }\n");
    m
}

/// `(return type, return statement)` for the operation's response.
fn response(ir: &OperationIR, tm: &TypeMap) -> (String, String) {
    match &ir.response {
        None => ("()".to_string(), "let _ = resp; Ok(())".to_string()),
        Some(RefOr::Ref(r)) => {
            let ty = tm
                .resolve_ref(&r.reference)
                .unwrap_or_else(|| "serde_json::Value".to_string());
            (ty, "Ok(resp.json()?)".to_string())
        }
        Some(node @ RefOr::Item(_)) => (rsmap::type_expr(node, tm), "Ok(resp.json()?)".to_string()),
    }
}
