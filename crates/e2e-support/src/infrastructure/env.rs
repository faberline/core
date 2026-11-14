//! Environment variables that the e2e runs need.

/// The value of the environment variable `name`.
///
/// # Panics
///
/// Panics when the variable is missing or empty. It never skips.
pub fn required(name: &str) -> String {
    required_from(name, std::env::var(name).ok())
}

/// The namespace of the run: `E2E_NAMESPACE`, else `NAMESPACE`.
///
/// # Panics
///
/// Panics when both variables are missing or empty.
pub fn namespace() -> String {
    namespace_from(
        std::env::var("E2E_NAMESPACE").ok(),
        std::env::var("NAMESPACE").ok(),
    )
}

fn required_from(name: &str, value: Option<String>) -> String {
    match value {
        Some(v) if !v.is_empty() => v,
        _ => panic!("required environment variable {name} is not set"),
    }
}

fn namespace_from(primary: Option<String>, fallback: Option<String>) -> String {
    primary
        .filter(|v| !v.is_empty())
        .or(fallback.filter(|v| !v.is_empty()))
        .unwrap_or_else(|| panic!("neither E2E_NAMESPACE nor NAMESPACE is set"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn required_from_returns_the_value() {
        assert_eq!(required_from("X", Some("v".into())), "v");
    }

    #[test]
    #[should_panic(expected = "required environment variable X is not set")]
    fn required_from_panics_when_missing() {
        required_from("X", None);
    }

    #[test]
    #[should_panic(expected = "required environment variable X is not set")]
    fn required_from_panics_when_empty() {
        required_from("X", Some(String::new()));
    }

    #[test]
    #[should_panic(expected = "E2E_SUPPORT_TEST_NO_SUCH_VARIABLE")]
    fn required_panics_when_the_variable_is_missing() {
        required("E2E_SUPPORT_TEST_NO_SUCH_VARIABLE");
    }

    #[test]
    fn namespace_prefers_e2e_namespace() {
        assert_eq!(namespace_from(Some("a".into()), Some("b".into())), "a");
    }

    #[test]
    fn namespace_falls_back_to_namespace() {
        assert_eq!(namespace_from(None, Some("b".into())), "b");
        assert_eq!(namespace_from(Some(String::new()), Some("b".into())), "b");
    }

    #[test]
    #[should_panic(expected = "neither E2E_NAMESPACE nor NAMESPACE is set")]
    fn namespace_panics_when_both_are_missing() {
        namespace_from(None, None);
    }
}
