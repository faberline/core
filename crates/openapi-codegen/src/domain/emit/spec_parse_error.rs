/// Why an emitter could not read the OpenAPI document it was given.
///
/// The `serde_json` error is the [`source`](std::error::Error::source), so
/// the `{:#}` chain names the line and column.
#[derive(Debug, thiserror::Error)]
#[error("failed to parse OpenAPI spec")]
pub struct SpecParseError {
    #[source]
    source: serde_json::Error,
}

impl SpecParseError {
    pub(crate) fn new(source: serde_json::Error) -> Self {
        Self { source }
    }
}
