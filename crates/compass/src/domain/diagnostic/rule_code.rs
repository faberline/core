//! The code that names the rule behind a diagnostic.

use std::fmt;

use serde::{Deserialize, Serialize};

/// The code of the rule that produced a diagnostic, such as `"PY001"` or
/// `"CUSTOM_NO_PRINT"`.
///
/// It serializes as a bare string, so the JSON and bincode formats of
/// [`Diagnostic`](crate::Diagnostic) are the same as when the code was a
/// `String`.
#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RuleCode(String);

impl RuleCode {
    /// Wrap a rule code.
    pub fn new(code: impl Into<String>) -> Self {
        Self(code.into())
    }

    /// The code as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Unwrap the code into its string.
    pub fn into_string(self) -> String {
        self.0
    }
}

impl fmt::Display for RuleCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for RuleCode {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl PartialEq<str> for RuleCode {
    fn eq(&self, other: &str) -> bool {
        self.0 == other
    }
}

impl PartialEq<&str> for RuleCode {
    fn eq(&self, other: &&str) -> bool {
        self.0 == *other
    }
}

impl From<&str> for RuleCode {
    fn from(code: &str) -> Self {
        Self(code.to_owned())
    }
}

impl From<String> for RuleCode {
    fn from(code: String) -> Self {
        Self(code)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn displays_and_compares_as_its_string() {
        let code = RuleCode::from("PY001");
        assert_eq!(code.to_string(), "PY001");
        assert_eq!(code.as_str(), "PY001");
        assert!(code == "PY001");
        assert!(code == *"PY001");
        assert_eq!(RuleCode::from(String::from("PY001")), code);
    }

    #[test]
    fn serializes_as_a_bare_string() {
        let code = RuleCode::from("CUSTOM_X");
        assert_eq!(serde_json::to_string(&code).unwrap(), "\"CUSTOM_X\"");
        let back: RuleCode = serde_json::from_str("\"CUSTOM_X\"").unwrap();
        assert_eq!(back, code);
    }
}
