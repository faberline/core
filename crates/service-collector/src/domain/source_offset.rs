use std::fmt;

use serde::{Deserialize, Serialize};

/// A byte position in a collector source. Record counts are separate values.
#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SourceOffset(u64);

impl SourceOffset {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

impl fmt::Debug for SourceOffset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.0, f)
    }
}

impl fmt::Display for SourceOffset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

#[cfg(test)]
mod tests {
    use super::SourceOffset;

    #[test]
    fn source_offsets_keep_bare_numbers_in_checkpoints() {
        for value in [0, 42, u64::MAX] {
            let offset = SourceOffset::new(value);
            let json = value.to_string();
            assert_eq!(serde_json::to_string(&offset).unwrap(), json);
            assert_eq!(serde_json::from_str::<SourceOffset>(&json).unwrap(), offset);
            assert_eq!(format!("{offset:?}"), json);
        }
    }
}
