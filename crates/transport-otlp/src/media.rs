use crate::error::{Result, TransportError};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OtlpMediaType {
    Json,
    Protobuf,
}

impl OtlpMediaType {
    pub fn parse(value: Option<&str>) -> Result<Self> {
        let value = value.unwrap_or("application/json");
        let media = value.split(';').next().unwrap_or(value).trim();
        match media {
            "application/json" => Ok(Self::Json),
            "application/x-protobuf" | "application/protobuf" => Ok(Self::Protobuf),
            other => Err(TransportError::UnsupportedMediaType {
                media_type: other.to_string(),
            }),
        }
    }

    pub fn content_type(self) -> &'static str {
        match self {
            Self::Json => "application/json",
            Self::Protobuf => "application/x-protobuf",
        }
    }
}
