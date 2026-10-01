use std::fmt;

use super::current::{CURRENT_FILE_NAME, CURRENT_TEMP_FILE_NAME};

const MAX_GENERATION_NAME_BYTES: usize = 128;

/// One safe direct-child generation directory name.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct GenerationName(String);

impl GenerationName {
    /// Parse a generation name that cannot escape the store root.
    pub fn parse(value: impl Into<String>) -> Result<Self, GenerationNameError> {
        let value = value.into();
        let bytes = value.as_bytes();
        let kind = if bytes.is_empty() {
            Some(GenerationNameErrorKind::Empty)
        } else if bytes.len() > MAX_GENERATION_NAME_BYTES {
            Some(GenerationNameErrorKind::TooLong)
        } else if value == CURRENT_FILE_NAME || value == CURRENT_TEMP_FILE_NAME {
            Some(GenerationNameErrorKind::Reserved)
        } else if !bytes[0].is_ascii_alphanumeric()
            || !bytes[1..]
                .iter()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(*byte, b'.' | b'_' | b'-'))
        {
            Some(GenerationNameErrorKind::InvalidCharacter)
        } else {
            None
        };

        match kind {
            Some(kind) => Err(GenerationNameError { kind }),
            None => Ok(Self(value)),
        }
    }

    /// Return the validated name.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for GenerationName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// Why a generation name is unsafe.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GenerationNameErrorKind {
    Empty,
    TooLong,
    InvalidCharacter,
    Reserved,
}

/// A rejected generation name.
#[derive(Debug)]
pub struct GenerationNameError {
    pub kind: GenerationNameErrorKind,
}

impl fmt::Display for GenerationNameError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid generation name: {:?}", self.kind)
    }
}

impl std::error::Error for GenerationNameError {}
