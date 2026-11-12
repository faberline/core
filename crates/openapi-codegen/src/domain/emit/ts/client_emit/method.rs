use crate::domain::emit::ts::plan::OperationPlan;
use crate::domain::ir::names::{self, is_ident};

pub(super) fn emit_method(p: &OperationPlan) -> String {
    let ndjson_body = p.body.as_ref().is_some_and(|b| b.ndjson);
    // NDJSON methods take an optional `options` (the abort signal) last.
    let options = ndjson_body || p.response_stream;
    let mut params: Vec<String> = Vec::new();
    if let Some(name) = &p.data_type_name {
        params.push(format!("data: {name}"));
    }
    if options {
        params.push("options?: NdjsonOptions".to_string());
    }
    let sig = format!("({})", params.join(", "));

    let mut args = match &p.post_twin_path {
        // OpenAPI 3.2 `QUERY` operation with a POST-twin fallback target:
        // route through `config.usePostFallback` at call time so a single
        // generated client can serve either transport (epic #1296 policy).
        Some(twin) if p.http_method == "QUERY" => vec![
            format!(
                "method: config.usePostFallback ? \"POST\" : \"{}\"",
                p.http_method
            ),
            format!(
                "path: config.usePostFallback ? {} : {}",
                path_template_for(twin),
                path_template_for(&p.path_raw)
            ),
        ],
        _ => vec![
            format!("method: \"{}\"", p.http_method),
            format!("path: {}", path_template_for(&p.path_raw)),
        ],
    };
    if !p.query_params.is_empty() {
        let entries = p
            .query_params
            .iter()
            .map(|f| {
                format!(
                    "{}: {}",
                    names::prop_key(&f.name),
                    access("data.query", !p.query_required(), &f.name)
                )
            })
            .collect::<Vec<_>>()
            .join(", ");
        args.push(format!("query: {{ {entries} }}"));
    }
    if !p.header_params.is_empty() {
        let entries = p
            .header_params
            .iter()
            .map(|f| {
                format!(
                    "{}: String({})",
                    names::prop_key(&f.name),
                    access("data.headers", !p.headers_required(), &f.name)
                )
            })
            .collect::<Vec<_>>()
            .join(", ");
        args.push(format!("headers: {{ {entries} }}"));
    }
    if ndjson_body {
        args.push("ndjsonBody: data.body".to_string());
    } else if p.body.is_some() {
        args.push("body: data.body".to_string());
    }
    if p.response_type == "void" {
        args.push("expectBody: false".to_string());
    }

    let resp = &p.response_type_name;
    let args = args.join(", ");
    let (ret, call) = if p.response_stream {
        (
            resp.to_string(),
            format!("requestNdjsonStream<NdjsonItem<{resp}>>(config, {{ {args} }}, options)"),
        )
    } else if ndjson_body {
        (
            format!("Promise<{resp}>"),
            format!("requestNdjsonUpload<{resp}>(config, {{ {args} }}, options)"),
        )
    } else {
        (
            format!("Promise<{resp}>"),
            format!("request<{resp}>(config, {{ {args} }})"),
        )
    };
    format!(
        "    {name}{sig}: {ret} {{\n      return {call};\n    }},\n",
        name = p.fn_name,
    )
}

/// `/pets/{petId}` → `` `/pets/${data.path.petId}` ``. Takes a raw path
/// template rather than an [`OperationPlan`] so it can also render a
/// POST-twin fallback path that shares the same `{param}` names.
fn path_template_for(raw: &str) -> String {
    let mut out = String::from("`");
    let mut chars = raw.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '{' {
            let mut name = String::new();
            for c in chars.by_ref() {
                if c == '}' {
                    break;
                }
                name.push(c);
            }
            out.push_str("${");
            out.push_str(&access("data.path", false, &name));
            out.push('}');
        } else {
            out.push(c);
        }
    }
    out.push('`');
    out
}

/// Member access against a grouped sub-object, e.g. `data.query?.limit` or
/// `data.headers["X-Id"]`.
fn access(base: &str, optional: bool, name: &str) -> String {
    if is_ident(name) {
        if optional {
            format!("{base}?.{name}")
        } else {
            format!("{base}.{name}")
        }
    } else {
        let key = name.replace('\\', "\\\\").replace('"', "\\\"");
        if optional {
            format!("{base}?.[\"{key}\"]")
        } else {
            format!("{base}[\"{key}\"]")
        }
    }
}
