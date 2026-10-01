use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};

use super::commit_error::{CommitError, CommitFailureClass};
use super::current::{current_read_as_io, read_current_from, CurrentTarget, CURRENT_FILE_NAME};
use super::failure_injection::{CommitStep, Mutation};
use super::inherited::InheritedFile;
use super::name::GenerationName;
use super::staging::StagedGeneration;
use super::store::GenerationStore;
use super::tree::{path_exists, validate_real_directory};

impl GenerationStore {
    /// Durably activate a complete staged generation.
    ///
    /// Before this call, the caller must close its writers and stop all changes
    /// under [`StagedGeneration::path`]. The caller must also ensure that no
    /// other process mutates this store root.
    pub fn commit(&self, staged: StagedGeneration) -> Result<GenerationName, CommitError> {
        self.commit_with_publication_guard(staged, || Ok(()))
    }

    /// Acquire a domain guard for CURRENT publication. The returned guard must
    /// remain held until the pointer and its parent directory are durable.
    pub fn commit_with_publication_guard<G>(
        &self,
        staged: StagedGeneration,
        acquire: impl FnOnce() -> io::Result<G>,
    ) -> Result<GenerationName, CommitError> {
        self.commit_staged_with_publication_guard(staged, None, BTreeMap::new(), acquire)
    }

    pub(super) fn commit_staged_with_publication_guard<G>(
        &self,
        staged: StagedGeneration,
        predecessor: Option<GenerationName>,
        inherited: BTreeMap<PathBuf, InheritedFile>,
        acquire: impl FnOnce() -> io::Result<G>,
    ) -> Result<GenerationName, CommitError> {
        let mut guard = self.lock_commit().map_err(|error| {
            self.commit_error(
                CommitFailureClass::PreCommit,
                CommitStep::ValidateStaging,
                CurrentTarget::Generation(staged.generation.clone()),
                &staged.path,
                error,
            )
        })?;
        let target = CurrentTarget::Generation(staged.generation.clone());
        let mut mutation = Mutation::new(self.inner.injector.as_ref());
        let current = match read_current_from(&self.inner.root, |relative| {
            mutation.check(CommitStep::ValidateCurrent, relative)
        }) {
            Ok(current) => current,
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
        };
        if predecessor.as_ref().is_some_and(|predecessor| {
            current != CurrentTarget::Generation(predecessor.clone())
                || guard.durable_current.as_ref() != Some(predecessor)
        }) {
            return Err(self.commit_error(
                CommitFailureClass::PreCommit,
                CommitStep::ValidateCurrent,
                target,
                self.inner.root.join(CURRENT_FILE_NAME),
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "CURRENT changed or lost durable proof since inherited staging began",
                ),
            ));
        }

        self.validate_staged(&staged, &target, &mut mutation)?;
        let skipped = self.verify_inherited_files(
            &staged,
            predecessor.as_ref(),
            &inherited,
            &target,
            &mut mutation,
        )?;
        self.sync_tree_with_skips(&staged.path, &target, &mut mutation, &skipped)?;

        let final_path = self.generation_path(&staged.generation);
        self.checked(
            &mut mutation,
            CommitStep::RenameGeneration,
            Path::new(staged.generation.as_str()),
            &target,
            &final_path,
            || std::fs::rename(&staged.path, &final_path),
        )?;
        self.sync_root(
            &mut mutation,
            CommitStep::SyncRootAfterGeneration,
            &target,
            CommitFailureClass::PreCommit,
        )?;

        let _publication = acquire().map_err(|error| {
            self.commit_error(
                CommitFailureClass::PreCommit,
                CommitStep::ValidateCurrent,
                target.clone(),
                &final_path,
                error,
            )
        })?;
        match self.commit_pointer(&target, &mut mutation) {
            Ok(()) => {
                guard.durable_current = Some(staged.generation.clone());
                Ok(staged.generation)
            }
            Err(error) => {
                if error.class() == CommitFailureClass::CommitUncertain {
                    guard.durable_current = None;
                }
                Err(error)
            }
        }
    }

    fn validate_staged(
        &self,
        staged: &StagedGeneration,
        target: &CurrentTarget,
        mutation: &mut Mutation<'_>,
    ) -> Result<(), CommitError> {
        let expected = self.stage_path(&staged.generation);
        if staged.root != self.inner.root || staged.path != expected {
            return Err(self.commit_error(
                CommitFailureClass::PreCommit,
                CommitStep::ValidateStaging,
                target.clone(),
                &staged.path,
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "staged generation belongs to another store",
                ),
            ));
        }
        let stage_relative = PathBuf::from(format!(".stage-{}", staged.generation));
        self.checked(
            mutation,
            CommitStep::ValidateStaging,
            &stage_relative,
            target,
            &staged.path,
            || validate_real_directory(&staged.path),
        )?;
        let final_path = self.generation_path(&staged.generation);
        let exists = self.checked_value(
            mutation,
            CommitStep::ValidateStaging,
            Path::new(staged.generation.as_str()),
            target,
            &final_path,
            || path_exists(&final_path),
        )?;
        if exists {
            return Err(self.commit_error(
                CommitFailureClass::PreCommit,
                CommitStep::ValidateStaging,
                target.clone(),
                final_path,
                io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    "immutable generation already exists",
                ),
            ));
        }
        Ok(())
    }
}
