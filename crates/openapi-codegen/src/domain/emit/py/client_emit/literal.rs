pub(super) fn esc(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

pub(super) fn python_literal(value: &str) -> String {
    serde_json::to_string(value).expect("serialize generated Python string literal")
}
