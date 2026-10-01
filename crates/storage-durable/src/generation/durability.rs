use std::collections::BTreeSet;
use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use super::commit_error::{CommitError, CommitFailureClass};
use super::current::{current_bytes, CurrentTarget, CURRENT_FILE_NAME, CURRENT_TEMP_FILE_NAME};
use super::failure_injection::{CommitStep, Mutation};
use super::store::GenerationStore;
use super::tree::{collect_tree, strict_sync_directory};

impl GenerationStore {
    pub(super) fn sync_tree(
        &self,
        tree_root: &Path,
        target: &CurrentTarget,
        mutation: &mut Mutation<'_>,
    ) -> Result<(), CommitError> {
        self.sync_tree_with_skips(tree_root, target, mutation, &BTreeSet::new())
    }

    pub(super) fn sync_tree_with_skips(
        &self,
        tree_root: &Path,
        target: &CurrentTarget,
        mutation: &mut Mutation<'_>,
        skipped: &BTreeSet<PathBuf>,
    ) -> Result<(), CommitError> {
        let entries = collect_tree(tree_root, |relative| {
            mutation.check(CommitStep::ValidateStaging, relative)
        })
        .map_err(|(path, error)| {
            self.commit_error(
                CommitFailureClass::PreCommit,
                CommitStep::ValidateStaging,
                target.clone(),
                path,
                error,
            )
        })?;

        for relative in &entries.files {
            if skipped.contains(relative) {
                continue;
            }
            let absolute = tree_root.join(relative);
            self.checked(
                mutation,
                CommitStep::SyncFile,
                relative,
                target,
                &absolute,
                || File::open(&absolute).and_then(|file| file.sync_all()),
            )?;
        }
        for relative in &entries.directories {
            let absolute = if relative == Path::new(".") {
                tree_root.to_path_buf()
            } else {
                tree_root.join(relative)
            };
            self.checked(
                mutation,
                CommitStep::SyncDirectory,
                relative,
                target,
                &absolute,
                || strict_sync_directory(&absolute),
            )?;
        }
        Ok(())
    }

    pub(super) fn commit_pointer(
        &self,
        target: &CurrentTarget,
        mutation: &mut Mutation<'_>,
    ) -> Result<(), CommitError> {
        let current = self.inner.root.join(CURRENT_FILE_NAME);
        let temp = self.inner.root.join(CURRENT_TEMP_FILE_NAME);
        mutation
            .check(
                CommitStep::RemoveStaleCurrentTemp,
                Path::new(CURRENT_TEMP_FILE_NAME),
            )
            .map_err(|error| {
                self.commit_error(
                    CommitFailureClass::PreCommit,
                    CommitStep::RemoveStaleCurrentTemp,
                    target.clone(),
                    &temp,
                    error,
                )
            })?;
        match std::fs::symlink_metadata(&temp) {
            Ok(metadata) => {
                if metadata.file_type().is_symlink() || !metadata.is_file() {
                    return Err(self.commit_error(
                        CommitFailureClass::PreCommit,
                        CommitStep::RemoveStaleCurrentTemp,
                        target.clone(),
                        &temp,
                        io::Error::new(
                            io::ErrorKind::InvalidData,
                            "CURRENT.tmp must be a regular file",
                        ),
                    ));
                }
                self.checked(
                    mutation,
                    CommitStep::RemoveStaleCurrentTemp,
                    Path::new(CURRENT_TEMP_FILE_NAME),
                    target,
                    &temp,
                    || std::fs::remove_file(&temp),
                )?;
                self.sync_root(
                    mutation,
                    CommitStep::SyncRootAfterTempCleanup,
                    target,
                    CommitFailureClass::PreCommit,
                )?;
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(self.commit_error(
                    CommitFailureClass::PreCommit,
                    CommitStep::RemoveStaleCurrentTemp,
                    target.clone(),
                    &temp,
                    error,
                ));
            }
        }

        let mut file = self.checked_value(
            mutation,
            CommitStep::CreateCurrentTemp,
            Path::new(CURRENT_TEMP_FILE_NAME),
            target,
            &temp,
            || OpenOptions::new().write(true).create_new(true).open(&temp),
        )?;
        let bytes = current_bytes(target);
        self.checked(
            mutation,
            CommitStep::WriteCurrentTemp,
            Path::new(CURRENT_TEMP_FILE_NAME),
            target,
            &temp,
            || file.write_all(&bytes),
        )?;
        self.checked(
            mutation,
            CommitStep::SyncCurrentTemp,
            Path::new(CURRENT_TEMP_FILE_NAME),
            target,
            &temp,
            || file.sync_all(),
        )?;
        drop(file);
        self.checked(
            mutation,
            CommitStep::RenameCurrent,
            Path::new(CURRENT_FILE_NAME),
            target,
            &current,
            || std::fs::rename(&temp, &current),
        )?;
        self.sync_root(
            mutation,
            CommitStep::SyncRootAfterCurrent,
            target,
            CommitFailureClass::CommitUncertain,
        )
    }

    pub(super) fn sync_root(
        &self,
        mutation: &mut Mutation<'_>,
        step: CommitStep,
        target: &CurrentTarget,
        class: CommitFailureClass,
    ) -> Result<(), CommitError> {
        let relative = Path::new(".");
        mutation
            .check(step, relative)
            .and_then(|_| strict_sync_directory(&self.inner.root))
            .map_err(|error| {
                self.commit_error(class, step, target.clone(), &self.inner.root, error)
            })
    }
}
