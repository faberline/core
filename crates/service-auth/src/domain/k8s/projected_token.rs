use std::fmt;

/// A bearer token that will not print itself.
///
/// `Debug` is the one that matters: `?err`, `{:?}` on a struct that holds a
/// token, and `#[derive(Debug)]` on anything upstream all reach it without
/// anyone deciding to log a credential.
#[derive(Clone)]
pub struct ProjectedToken(pub(crate) String);

impl ProjectedToken {
    /// Wrap material that is already a token.
    ///
    /// Crate-internal on purpose. There are exactly two things in this crate
    /// that hold one — the file this module reads and the TokenRequest
    /// [`super::token_request`] makes — and a public constructor would make
    /// this type a general-purpose string wrapper whose redaction is
    /// decorative rather than a property of where tokens come from.
    pub(crate) fn new(material: String) -> Self {
        Self(material)
    }

    /// The material itself, for the one place that puts it on the wire.
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for ProjectedToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ProjectedToken(<redacted>)")
    }
}

impl fmt::Display for ProjectedToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<redacted service account token>")
    }
}
