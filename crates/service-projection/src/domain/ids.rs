use std::fmt;

use serde::{Deserialize, Serialize};

macro_rules! string_id {
    ($name:ident, $doc:literal) => {
        #[doc = $doc]
        #[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            pub fn new(value: impl Into<String>) -> Self {
                Self(value.into())
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }
    };
}

macro_rules! number_id {
    ($name:ident, $doc:literal) => {
        #[doc = $doc]
        #[derive(
            Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
        )]
        #[serde(transparent)]
        pub struct $name(u64);

        impl $name {
            pub const fn new(value: u64) -> Self {
                Self(value)
            }

            pub const fn get(self) -> u64 {
                self.0
            }
        }
    };
}

string_id!(
    ProjectionName,
    "The name of one projection. It encodes as a string."
);
string_id!(
    ProjectionEventId,
    "The product's source event identity. It encodes as a string."
);
number_id!(
    ProjectionCursor,
    "A position in a projection's source. It encodes as a number."
);
number_id!(
    SourceGeneration,
    "The identity of a retained source set. It encodes as a number."
);

impl ProjectionCursor {
    /// The number of events advanced since `earlier`, stopping at zero.
    ///
    /// ```compile_fail
    /// use service_projection::{ProjectionCursor, SourceGeneration};
    /// ProjectionCursor::new(10).distance_since(SourceGeneration::new(3));
    /// ```
    pub const fn distance_since(self, earlier: Self) -> u64 {
        self.0.saturating_sub(earlier.0)
    }
}

macro_rules! bare_format {
    ($($name:ident),*) => {$(
        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                fmt::Debug::fmt(&self.0, f)
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                fmt::Display::fmt(&self.0, f)
            }
        }
    )*};
}

bare_format!(
    ProjectionName,
    ProjectionEventId,
    ProjectionCursor,
    SourceGeneration
);
