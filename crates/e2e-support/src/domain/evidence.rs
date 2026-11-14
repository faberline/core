pub const EVIDENCE_PREFIX: &str = "E2E_EVIDENCE";

/// `E2E_EVIDENCE {"suite":..,"step":..,"data":..}` (one line, no trailing newline).
pub fn evidence_line(suite: &str, step: &str, data: &serde_json::Value) -> String {
    let value = serde_json::json!({"suite": suite, "step": step, "data": data});
    format!("{EVIDENCE_PREFIX} {value}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn evidence_line_has_exact_shape() {
        let line = evidence_line("lumen", "restore", &json!({"n": 1}));
        assert_eq!(
            line,
            r#"E2E_EVIDENCE {"suite":"lumen","step":"restore","data":{"n":1}}"#
        );
    }

    #[test]
    fn evidence_line_has_no_ok_key_and_no_newline() {
        let line = evidence_line("lumen", "restore", &json!({"n": 1}));
        assert!(!line.contains("\"ok\""), "{line}");
        assert!(!line.contains('\n'), "{line}");
    }
}
