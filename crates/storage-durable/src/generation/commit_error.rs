use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

use super::current::CurrentTarget;
use super::failure_injection::{CommitStep, Mutation};
use super::store::GenerationStore;

/// Whether the activation commit point definitely ran.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommitFailureClass {
    PreCommit,
    CommitUncertain,
}

/// A durable generation mutation failure.
#[derive(Debug)]
pub struct CommitError {
    class: CommitFailureClass,
    step: CommitStep,
    target: CurrentTarget,
    path: PathBuf,
    source: io::Error,
}

impl CommitError {
    pub fn class(&self) -> CommitFailureClass {
        self.class
    }

    pub fn step(&self) -> CommitStep {
        self.step
    }

    pub fn target(&self) -> &CurrentTarget {
        &self.target
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn io_error(&self) -> &io::Error {
        &self.source
    }
}

impl fmt::Display for CommitError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "durable generation {:?} failure at {:?} for {}: {}",
            self.class,
            self.step,
            self.path.display(),
            self.source
        )
    }
}

impl std::error::Error for CommitError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.source)
    }
}

impl GenerationStore {
    pub(super) fn checked(
        &self,
        mutation: &mut Mutation<'_>,
        step: CommitStep,
        relative: &Path,
        target: &CurrentTarget,
        absolute: &Path,
        operation: impl FnOnce() -> io::Result<()>,
    ) -> Result<(), CommitError> {
        self.checked_value(mutation, step, relative, target, absolute, operation)
    }

    pub(super) fn checked_value<T>(
        &self,
        mutation: &mut Mutation<'_>,
        step: CommitStep,
        relative: &Path,
        target: &CurrentTarget,
        absolute: &Path,
        operation: impl FnOnce() -> io::Result<T>,
    ) -> Result<T, CommitError> {
        mutation
            .check(step, relative)
            .and_then(|_| operation())
            .map_err(|error| {
                self.commit_error(
                    CommitFailureClass::PreCommit,
                    step,
                    target.clone(),
                    absolute,
                    error,
                )
            })
    }

    pub(super) fn commit_error(
        &self,
        class: CommitFailureClass,
        step: CommitStep,
        target: CurrentTarget,
        path: impl AsRef<Path>,
        source: io::Error,
    ) -> CommitError {
        CommitError {
            class,
            step,
            target,
            path: path.as_ref().to_path_buf(),
            source,
        }
    }
}
