use std::collections::BTreeSet;

use super::method::emit_method;
use crate::domain::emit::ts::plan::OperationPlan;
use crate::domain::emit::ts::types_emit::HEADER;
use crate::domain::GenOptions;

/// Render `client.ts`.
///
pub fn emit_client(plans: &[OperationPlan], opts: &GenOptions) -> String {
    let mut out = String::from(HEADER);
    out.push_str("import type { ClientConfig } from \"./runtime\";\n");
    out.push_str("import { request } from \"./runtime\";\n");
    out.push_str(&type_import(plans));
    out.push('\n');

    let factory = &opts.client_name;
    out.push_str(&format!(
        "export function {factory}(config: ClientConfig) {{\n"
    ));
    out.push_str("  return {\n");
    for p in plans {
        out.push_str(&emit_method(p));
    }
    out.push_str("  };\n");
    out.push_str("}\n\n");
    out.push_str(&format!(
        "export type ApiClient = ReturnType<typeof {factory}>;\n"
    ));
    out
}

/// `import type { ... } from "./types";` for the per-operation type names.
pub fn type_import(plans: &[OperationPlan]) -> String {
    let mut names: BTreeSet<String> = BTreeSet::new();
    for p in plans {
        if let Some(d) = &p.data_type_name {
            names.insert(d.clone());
        }
        names.insert(p.response_type_name.clone());
    }
    if names.is_empty() {
        return String::new();
    }
    let list = names.into_iter().collect::<Vec<_>>().join(", ");
    format!("import type {{ {list} }} from \"./types\";\n")
}
