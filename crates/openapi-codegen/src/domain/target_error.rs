use crate::domain::{Lang, TargetProfile};

/// A target profile identifier that names no known profile, from
/// [`TargetProfile::from_id`] and `str::parse::<TargetProfile>`.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error(
    "unknown target profile {id:?}; expected one of: python-3.11, python-3.12, python-3.13, \
     python-3.14, typescript-5.0, rust-2021, rust-2024"
)]
pub struct UnknownTargetProfile {
    id: String,
}

impl UnknownTargetProfile {
    pub(crate) fn new(id: impl Into<String>) -> Self {
        Self { id: id.into() }
    }

    /// The identifier that was not recognized.
    pub fn id(&self) -> &str {
        &self.id
    }
}

/// Why [`TargetPolicy::resolve`](crate::TargetPolicy::resolve) could not pick
/// a target profile.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum TargetPolicyError {
    /// The explicit override is not a known profile identifier.
    #[error("parse explicit target profile {value:?}")]
    UnknownExplicit {
        /// The explicit override as given.
        value: String,
        /// The parse failure.
        #[source]
        source: UnknownTargetProfile,
    },
    /// The chosen profile belongs to another language.
    #[error(
        "target profile {} is for {:?}, not requested language {:?}",
        .target.id(),
        .target.lang(),
        .requested
    )]
    LanguageMismatch {
        /// The profile that was chosen.
        target: TargetProfile,
        /// The language the caller asked for.
        requested: Lang,
    },
}
