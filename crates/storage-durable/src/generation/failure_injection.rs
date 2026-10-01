use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};

/// One load-bearing operation in a generation mutation.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CommitStep {
    ValidateCurrent,
    ValidateStaging,
    SyncFile,
    SyncDirectory,
    RenameGeneration,
    SyncRootAfterGeneration,
    RemoveStaleCurrentTemp,
    SyncRootAfterTempCleanup,
    CreateCurrentTemp,
    WriteCurrentTemp,
    SyncCurrentTemp,
    RenameCurrent,
    SyncRootAfterCurrent,
}

/// One deterministic injection point.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FailurePoint {
    pub step: CommitStep,
    pub occurrence: usize,
    pub relative_path: PathBuf,
}

/// Test seam for failures immediately before load-bearing filesystem calls.
pub trait FailureInjector: Send + Sync {
    fn check(&self, point: &FailurePoint) -> io::Result<()>;
}

/// Production injector that never fails.
#[derive(Debug, Default)]
pub struct NoFailures;

impl FailureInjector for NoFailures {
    fn check(&self, _: &FailurePoint) -> io::Result<()> {
        Ok(())
    }
}

pub(super) struct Mutation<'a> {
    injector: &'a dyn FailureInjector,
    occurrences: HashMap<CommitStep, usize>,
}

impl<'a> Mutation<'a> {
    pub(super) fn new(injector: &'a dyn FailureInjector) -> Self {
        Self {
            injector,
            occurrences: HashMap::new(),
        }
    }

    pub(super) fn check(&mut self, step: CommitStep, relative_path: &Path) -> io::Result<()> {
        let occurrence = self.occurrences.entry(step).or_default();
        let point = FailurePoint {
            step,
            occurrence: *occurrence,
            relative_path: relative_path.to_path_buf(),
        };
        *occurrence += 1;
        self.injector.check(&point)
    }
}
