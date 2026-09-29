use super::literal::esc;
use crate::domain::emit::py::pymap;
use crate::domain::ir::names::{to_snake, NameRegistry};
use crate::domain::ir::openapi::RefOr;
use crate::domain::ir::operations::{OperationIR, ParamIR};
use crate::domain::ir::typemap::TypeMap;
use crate::domain::PythonTarget;

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

/// `(python_param, type, required)` for a parameter.
fn param_ann(p: &ParamIR, tm: &TypeMap, target: Option<PythonTarget>) -> (String, String, bool) {
    let ty = p
        .schema
        .as_ref()
        .map(|s| pymap::type_expr(s, tm, target))
        .unwrap_or_else(|| "str".to_string());
    (to_snake(&p.name), ty, p.required)
}

pub(super) fn emit_method(
    ir: &OperationIR,
    tm: &TypeMap,
    reg: &mut NameRegistry,
    is_async: bool,
    target: Option<PythonTarget>,
    file_bearer_auth: bool,
) -> String {
    let name = reg.unique(&fn_name(ir));

    // Signature parameters in a stable order: path, query, header, body.
    let mut sig: Vec<String> = Vec::new();
    let path_anns: Vec<(String, String, bool)> = ir
        .path_params
        .iter()
        .map(|p| param_ann(p, tm, target))
        .collect();
    let query_anns: Vec<(String, String, bool)> = ir
        .query_params
        .iter()
        .map(|p| param_ann(p, tm, target))
        .collect();
    let header_anns: Vec<(String, String, bool)> = ir
        .header_params
        .iter()
        .map(|p| param_ann(p, tm, target))
        .collect();

    for (n, t, _) in &path_anns {
        sig.push(format!("{n}: {t}")); // path params are always required
    }
    for (n, t, req) in query_anns.iter().chain(header_anns.iter()) {
        if *req {
            sig.push(format!("{n}: {t}"));
        } else {
            sig.push(format!("{n}: {} = None", pymap::optional(t, target)));
        }
    }
    let body_ty = ir
        .body
        .as_ref()
        .map(|b| pymap::type_expr(&b.schema, tm, target));
    if let Some(ty) = &body_ty {
        let required = ir.body.as_ref().map(|b| b.required).unwrap_or(false);
        if required {
            sig.push(format!("body: {ty}"));
        } else {
            sig.push(format!("body: {} = None", pymap::optional(ty, target)));
        }
    }

    let (ret_ann, ret_stmt) = response(ir, tm, target);
    let sig_str = if sig.is_empty() {
        String::new()
    } else {
        format!(", *, {}", sig.join(", "))
    };

    let async_prefix = if is_async { "async " } else { "" };
    let mut m = format!("    {async_prefix}def {name}(self{sig_str}) -> {ret_ann}:\n");

    // Path with python param names substituted into the f-string.
    let mut path = ir.path.clone();
    for (p, (snake, ..)) in ir.path_params.iter().zip(&path_anns) {
        path = path.replace(&format!("{{{}}}", p.name), &format!("{{{snake}}}"));
    }
    m.push_str(&format!("        _path = f\"{path}\"\n"));

    // Epic #1296 POST-twin fallback: a `QUERY` operation always carries a
    // twin path (`x-post-twin` override, else its own path); at call time,
    // `self._use_post_fallback` picks POST + twin path over HTTP QUERY.
    let twin = ir.post_twin_path.as_ref().map(|twin_path| {
        let mut twin_path = twin_path.clone();
        for (p, (snake, ..)) in ir.path_params.iter().zip(&path_anns) {
            twin_path = twin_path.replace(&format!("{{{}}}", p.name), &format!("{{{snake}}}"));
        }
        twin_path
    });

    m.push_str("        _params: dict[str, Any] = {}\n");
    for (p, (snake, _, req)) in ir.query_params.iter().zip(&query_anns) {
        if *req {
            m.push_str(&format!(
                "        _params[\"{}\"] = {snake}\n",
                esc(&p.name)
            ));
        } else {
            m.push_str(&format!(
                "        if {snake} is not None:\n            _params[\"{}\"] = {snake}\n",
                esc(&p.name)
            ));
        }
    }
    m.push_str("        _headers: dict[str, Any] = dict(self._default_headers)\n");
    for (p, (snake, _, req)) in ir.header_params.iter().zip(&header_anns) {
        if *req {
            m.push_str(&format!(
                "        _headers[\"{}\"] = {snake}\n",
                esc(&p.name)
            ));
        } else {
            m.push_str(&format!(
                "        if {snake} is not None:\n            _headers[\"{}\"] = {snake}\n",
                esc(&p.name)
            ));
        }
    }
    if file_bearer_auth {
        m.push_str("        _headers = _file_bearer_headers(self._base_url, _headers)\n");
    }

    let json_arg = if body_ty.is_some() {
        m.push_str("        _json = body.model_dump(by_alias=True, exclude_none=True) if isinstance(body, BaseModel) else body\n");
        ", json=_json"
    } else {
        ""
    };

    let method_expr = match &twin {
        Some(twin_path) => {
            m.push_str("        if self._use_post_fallback:\n");
            m.push_str("            _method = \"POST\"\n");
            m.push_str(&format!("            _path = f\"{twin_path}\"\n"));
            m.push_str("        else:\n");
            m.push_str(&format!("            _method = \"{}\"\n", ir.http_method));
            "_method".to_string()
        }
        None => format!("\"{}\"", ir.http_method),
    };

    if is_async {
        m.push_str(&format!(
            "        _resp = await self._client.request({method_expr}, self._base_url + _path, params=_params, headers=_headers{json_arg})\n"
        ));
    } else {
        m.push_str(&format!(
            "        _resp = self._client.request({method_expr}, self._base_url + _path, params=_params, headers=_headers{json_arg})\n"
        ));
    }
    m.push_str("        _resp.raise_for_status()\n");
    m.push_str(&format!("        {ret_stmt}\n"));
    m
}

/// `(return annotation, return statement)` for the operation's response.
fn response(ir: &OperationIR, tm: &TypeMap, target: Option<PythonTarget>) -> (String, String) {
    match &ir.response {
        None => ("None".to_string(), "return None".to_string()),
        Some(RefOr::Ref(r)) => match tm.resolve_ref(&r.reference) {
            Some(model) => (
                model.clone(),
                format!("return {model}.model_validate(_resp.json())"),
            ),
            None => ("Any".to_string(), "return _resp.json()".to_string()),
        },
        Some(node @ RefOr::Item(_)) => (
            pymap::type_expr(node, tm, target),
            "return _resp.json()".to_string(),
        ),
    }
}
