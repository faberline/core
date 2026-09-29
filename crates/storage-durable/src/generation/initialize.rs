use std::io;
use std::path::{Path, PathBuf};

use super::commit_error::{CommitError, CommitFailureClass};
use super::current::{
    current_read_as_io, read_current_from, CurrentReadErrorKind, CurrentTarget, CURRENT_FILE_NAME,
};
use super::failure_injection::{CommitStep, Mutation};
use super::name::GenerationName;
use super::store::GenerationStore;
use super::tree::validate_real_directory;

impl GenerationStore {
    /// Initialize an empty store. Missing `CURRENT` never implies this state.
    pub fn initialize_empty(&self) -> Result<(), CommitError> {
        let mut guard = self.lock_commit().map_err(|error| {
            self.commit_error(
                CommitFailureClass::PreCommit,
                CommitStep::ValidateCurrent,
                CurrentTarget::Empty,
                &self.inner.root,
                error,
            )
        })?;
        let target = CurrentTarget::Empty;
        let mut mutation = Mutation::new(self.inner.injector.as_ref());
        match read_current_from(&self.inner.root, |relative| {
            mutation.check(CommitStep::ValidateCurrent, relative)
        }) {
            Err(error) if error.kind == CurrentReadErrorKind::Missing => {}
            Ok(_) => {
                return Err(self.commit_error(
                    CommitFailureClass::PreCommit,
                    CommitStep::ValidateCurrent,
                    target,
                    self.inner.root.join(CURRENT_FILE_NAME),
                    io::Error::new(
                        io::ErrorKind::AlreadyExists,
                        "CURRENT is already initialized",
                    ),
                ));
            }
            Err(error) => {
                let path = error.path.clone();
                return Err(self.commit_error(
                    CommitFailureClass::PreCommit,
                    CommitStep::ValidateCurrent,
                    target,
                    path,
                    current_read_as_io(error),
                ));
            }
        }

        mutation
            .check(CommitStep::ValidateCurrent, Path::new("."))
            .map_err(|error| {
                self.commit_error(
                    CommitFailureClass::PreCommit,
                    CommitStep::ValidateCurrent,
                    target.clone(),
                    &self.inner.root,
                    error,
                )
            })?;
        let mut entries: Vec<PathBuf> = std::fs::read_dir(&self.inner.root)
            .map_err(|error| {
                self.commit_error(
                    CommitFailureClass::PreCommit,
                    CommitStep::ValidateCurrent,
                    target.clone(),
                    &self.inner.root,
                    error,
                )
            })?
            .map(|entry| entry.map(|entry| entry.path()))
            .collect::<io::Result<_>>()
            .map_err(|error| {
                self.commit_error(
                    CommitFailureClass::PreCommit,
                    CommitStep::ValidateCurrent,
                    target.clone(),
                    &self.inner.root,
                    error,
                )
            })?;
        entries.sort();
        for entry in entries {
            let relative = entry
                .file_name()
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("."));
            mutation
                .check(CommitStep::ValidateCurrent, &relative)
                .map_err(|error| {
                    self.commit_error(
                        CommitFailureClass::PreCommit,
                        CommitStep::ValidateCurrent,
                        target.clone(),
                        &entry,
                        error,
                    )
                })?;
            let metadata = std::fs::symlink_metadata(&entry).map_err(|error| {
                self.commit_error(
                    CommitFailureClass::PreCommit,
                    CommitStep::ValidateCurrent,
                    target.clone(),
                    &entry,
                    error,
                )
            })?;
            let valid_generation = entry
                .file_name()
                .and_then(|name| name.to_str())
                .and_then(|name| GenerationName::parse(name.to_owned()).ok())
                .is_some();
            if metadata.is_dir() && !metadata.file_type().is_symlink() && valid_generation {
                return Err(self.commit_error(
                    CommitFailureClass::PreCommit,
                    CommitStep::ValidateCurrent,
                    target,
                    &entry,
                    io::Error::new(
                        io::ErrorKind::AlreadyExists,
                        "legacy generation blocks empty initialization",
                    ),
                ));
            }
        }

        match self.commit_pointer(&CurrentTarget::Empty, &mut mutation) {
            Ok(()) => {
                guard.durable_current = None;
                Ok(())
            }
            Err(error) => {
                if error.class() == CommitFailureClass::CommitUncertain {
                    guard.durable_current = None;
                }
                Err(error)
            }
        }
    }

    /// Point an uninitialized store at one caller-validated legacy generation.
    pub fn adopt_legacy(&self, generation: GenerationName) -> Result<(), CommitError> {
        let mut guard = self.lock_commit().map_err(|error| {
            self.commit_error(
                CommitFailureClass::PreCommit,
                CommitStep::ValidateCurrent,
                CurrentTarget::Generation(generation.clone()),
                &self.inner.root,
                error,
            )
        })?;
        let target = CurrentTarget::Generation(generation.clone());
        let mut mutation = Mutation::new(self.inner.injector.as_ref());
        match read_current_from(&self.inner.root, |relative| {
            mutation.check(CommitStep::ValidateCurrent, relative)
        }) {
            Err(error) if error.kind == CurrentReadErrorKind::Missing => {}
            Ok(_) => {
                return Err(self.commit_error(
                    CommitFailureClass::PreCommit,
                    CommitStep::ValidateCurrent,
                    target,
                    self.inner.root.join(CURRENT_FILE_NAME),
                    io::Error::new(
                        io::ErrorKind::AlreadyExists,
                        "CURRENT is already initialized",
                    ),
                ));
            }
            Err(error) => {
                let path = error.path.clone();
                return Err(self.commit_error(
                    CommitFailureClass::PreCommit,
                    CommitStep::ValidateCurrent,
                    target,
                    path,
                    current_read_as_io(error),
                ));
            }
        }

        let generation_path = self.generation_path(&generation);
        self.checked(
            &mut mutation,
            CommitStep::ValidateStaging,
            Path::new(generation.as_str()),
            &target,
            &generation_path,
            || validate_real_directory(&generation_path),
        )?;
        self.sync_tree(&generation_path, &target, &mut mutation)?;
        self.sync_root(
            &mut mutation,
            CommitStep::SyncRootAfterGeneration,
            &target,
            CommitFailureClass::PreCommit,
        )?;
        match self.commit_pointer(&target, &mut mutation) {
            Ok(()) => {
                guard.durable_current = Some(generation);
                Ok(())
            }
            Err(error) => {
                if error.class() == CommitFailureClass::CommitUncertain {
                    guard.durable_current = None;
                }
                Err(error)
            }
        }
    }
}
