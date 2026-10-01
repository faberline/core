use thiserror::Error;

#[derive(Debug, Error, Eq, PartialEq)]
pub enum HeaderError {
    #[error("Prometheus Remote Write 2.0 is not supported")]
    RemoteWriteTwo,
    #[error("Prometheus Remote Write 1.0 requires application/x-protobuf")]
    UnsupportedMediaType,
    #[error("Prometheus Remote Write 1.0 requires snappy block compression")]
    UnsupportedEncoding,
    #[error("unsupported Prometheus Remote Write version `{0}`")]
    UnsupportedVersion(String),
}

/// Validate all transport headers before any payload is decoded or written.
pub fn validate_headers(
    content_type: &str,
    content_encoding: &str,
    version: Option<&str>,
) -> Result<(), HeaderError> {
    let media = content_type.trim().to_ascii_lowercase();
    let version = version.map(str::trim).filter(|value| !value.is_empty());
    if media.contains("io.prometheus.write.v2.request")
        || version.is_some_and(|value| value.starts_with('2'))
    {
        return Err(HeaderError::RemoteWriteTwo);
    }
    if media.split(';').next().map(str::trim) != Some("application/x-protobuf") {
        return Err(HeaderError::UnsupportedMediaType);
    }
    if !content_encoding.trim().eq_ignore_ascii_case("snappy") {
        return Err(HeaderError::UnsupportedEncoding);
    }
    if let Some(version) = version {
        if version != "0.1.0" && version != "1.0" && version != "1.0.0" {
            return Err(HeaderError::UnsupportedVersion(version.to_owned()));
        }
    }
    Ok(())
}
