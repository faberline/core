//! The errors of the ports a service implements: [`StateMachineError`] for
//! [`RaftStateMachine`](crate::RaftStateMachine), [`SnapshotPreparation`](crate::SnapshotPreparation)
//! and [`PreparedSnapshot`](crate::PreparedSnapshot), and [`MembershipError`]
//! for [`MembershipPolicy`](crate::MembershipPolicy).
//!
//! Both keep an implementor's own error whole. Converting an
//! [`anyhow::Error`] with `?` (the `From` impl) stores that `anyhow::Error`
//! itself, and [`into_anyhow`](StateMachineError::into_anyhow) hands the same
//! value back, so `downcast_ref::<TheirError>()` on what the host returns
//! finds exactly what it found before the ports were typed.

use std::error::Error;
use std::fmt;

use raft_core::Index;

/// A boxed error from an implementor.
type BoxedError = Box<dyn Error + Send + Sync>;

/// An implementor's `anyhow::Error`, kept whole inside an `Other` variant.
/// `Display`, `Debug` and `source` are the anyhow error's own.
struct AnyhowError(anyhow::Error);

impl fmt::Display for AnyhowError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

impl fmt::Debug for AnyhowError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.0, f)
    }
}

impl Error for AnyhowError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.0.source()
    }
}

fn box_anyhow(error: anyhow::Error) -> BoxedError {
    Box::new(AnyhowError(error))
}

/// The `anyhow::Error` for a boxed implementor error: the original
/// `anyhow::Error` if that is what was boxed, else `wrap(boxed)`.
fn unbox_anyhow(
    boxed: BoxedError,
    wrap: impl FnOnce(BoxedError) -> anyhow::Error,
) -> anyhow::Error {
    match boxed.downcast::<AnyhowError>() {
        Ok(original) => original.0,
        Err(boxed) => wrap(boxed),
    }
}

/// The first error of type `T` in `boxed` or its sources. An `anyhow::Error`
/// kept whole is searched with its own `downcast_ref`, as before.
fn find_in<T: Error + Send + Sync + 'static>(boxed: &BoxedError) -> Option<&T> {
    if let Some(original) = boxed.downcast_ref::<AnyhowError>() {
        return original.0.downcast_ref::<T>();
    }
    let first: &(dyn Error + 'static) = boxed.as_ref();
    let mut cause = Some(first);
    while let Some(error) = cause {
        if let Some(found) = error.downcast_ref::<T>() {
            return Some(found);
        }
        cause = error.source();
    }
    None
}

/// Why a [`RaftStateMachine`](crate::RaftStateMachine),
/// [`SnapshotPreparation`](crate::SnapshotPreparation) or
/// [`PreparedSnapshot`](crate::PreparedSnapshot) call failed.
#[derive(Debug, thiserror::Error)]
pub enum StateMachineError {
    /// The default `snapshot_at` refuses a prefix other than the applied head.
    #[error(
        "state machine cannot snapshot Raft prefix {index}; current applied index is {applied}"
    )]
    PrefixUnavailable { index: Index, applied: Index },
    /// The implementor's own error. Build it with [`StateMachineError::other`],
    /// or with `?` from an `anyhow::Error`.
    #[error(transparent)]
    Other(BoxedError),
}

impl StateMachineError {
    /// Wrap an implementor's error.
    pub fn other(error: impl Into<Box<dyn Error + Send + Sync>>) -> Self {
        Self::Other(error.into())
    }

    /// The `anyhow::Error` the host returns for this error. An
    /// `anyhow::Error` converted in with `?` comes back unchanged; any other
    /// error comes back as this `StateMachineError`.
    pub fn into_anyhow(self) -> anyhow::Error {
        match self {
            Self::Other(boxed) => {
                unbox_anyhow(boxed, |boxed| anyhow::Error::new(Self::Other(boxed)))
            }
            error => anyhow::Error::new(error),
        }
    }

    /// The first error of type `T` this error carries, searching an
    /// implementor's error and its sources.
    pub(crate) fn find<T: Error + Send + Sync + 'static>(&self) -> Option<&T> {
        match self {
            Self::Other(boxed) => find_in(boxed),
            Self::PrefixUnavailable { .. } => None,
        }
    }
}

impl From<anyhow::Error> for StateMachineError {
    fn from(error: anyhow::Error) -> Self {
        Self::Other(box_anyhow(error))
    }
}

/// Why a [`MembershipPolicy`](crate::MembershipPolicy) refused a topology.
#[derive(Debug, thiserror::Error)]
pub enum MembershipError {
    /// The policy's own refusal. Build it with [`MembershipError::other`], or
    /// with `?` from an `anyhow::Error`.
    #[error(transparent)]
    Other(BoxedError),
}

impl MembershipError {
    /// Wrap a policy's error.
    pub fn other(error: impl Into<Box<dyn Error + Send + Sync>>) -> Self {
        Self::Other(error.into())
    }

    /// The `anyhow::Error` the builder returns for this error. An
    /// `anyhow::Error` converted in with `?` comes back unchanged; any other
    /// error comes back as this `MembershipError`.
    pub fn into_anyhow(self) -> anyhow::Error {
        match self {
            Self::Other(boxed) => {
                unbox_anyhow(boxed, |boxed| anyhow::Error::new(Self::Other(boxed)))
            }
        }
    }
}

impl From<anyhow::Error> for MembershipError {
    fn from(error: anyhow::Error) -> Self {
        Self::Other(box_anyhow(error))
    }
}
