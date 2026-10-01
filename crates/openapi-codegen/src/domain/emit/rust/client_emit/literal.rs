pub(super) fn esc(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

pub(super) fn rust_literal(value: &str) -> String {
    format!("{value:?}")
}
