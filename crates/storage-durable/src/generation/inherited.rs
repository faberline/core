use std::collections::{BTreeMap, BTreeSet};
use std::io;
use std::path::{Path, PathBuf};

use super::commit_error::{CommitError, CommitFailureClass};
use super::current::CurrentTarget;
#[cfg(unix)]
use super::current::{current_read_as_io, read_current_from};
use super::failure_injection::{CommitStep, Mutation};
use super::name::GenerationName;
use super::staging::StagedGeneration;
use super::store::GenerationStore;
use super::tree::validate_real_directory;

/// A private staging transaction derived from the exact current generation.
///
/// It can register only same-relative hard links to that current generation.
/// The value is single-use and is consumed by
/// [`GenerationStore::commit_from_current`]. Ordinary staging remains fully
/// synced by [`GenerationStore::commit`].
#[derive(Debug)]
pub struct CurrentGenerationStaging {
    staged: StagedGeneration,
    predecessor: GenerationName,
    root: PathBuf,
    inherited: BTreeMap<PathBuf, InheritedFile>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct InheritedFile {
    #[cfg(unix)]
    identity: FileIdentity,
}

#[cfg(unix)]
#[derive(Clone, Debug, Eq, PartialEq)]
struct FileIdentity {
    dev: u64,
    ino: u64,
    len: u64,
    mtime: i64,
    mtime_nsec: i64,
}

impl CurrentGenerationStaging {
    /// Return the generation that will become current on a successful commit.
    pub fn generation(&self) -> &GenerationName {
        self.staged.generation()
    }

    /// Return the private staging directory for this transaction.
    pub fn path(&self) -> &Path {
        self.staged.path()
    }

    /// Register one final same-relative file hard-linked from the captured
    /// current generation.
    ///
    /// This is available only on Unix. The source and staged paths must be
    /// regular, nonsymlink files with the same device and inode. Call it after
    /// all staged writes and removals, so fresh replacements stay fully synced.
    pub fn inherit_current_file(&mut self, relative: impl AsRef<Path>) -> io::Result<()> {
        #[cfg(unix)]
        {
            let relative = checked_relative_file(relative.as_ref())?;
            let source_root = self.root.join(self.predecessor.as_str());
            validate_real_relative_parent(&source_root, &relative)?;
            validate_real_relative_parent(&self.staged.path, &relative)?;
            let source = source_root.join(&relative);
            let destination = self.staged.path.join(&relative);
            let source_identity = regular_file_identity(&source)?;
            let destination_identity = regular_file_identity(&destination)?;
            if source_identity.dev != destination_identity.dev
                || source_identity.ino != destination_identity.ino
            {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "inherited staged file is not a hard link to CURRENT",
                ));
            }
            match self.inherited.get(&relative) {
                Some(existing) if existing.identity == source_identity => Ok(()),
                Some(_) => Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "inherited staged file registration changed",
                )),
                None => {
                    self.inherited.insert(
                        relative,
                        InheritedFile {
                            identity: source_identity,
                        },
                    );
                    Ok(())
                }
            }
        }
        #[cfg(not(unix))]
        {
            let _ = relative;
            Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "current-generation inherited sync is supported only on Unix",
            ))
        }
    }
}

impl GenerationStore {
    /// Begin a private stage only when CURRENT has a process-owned durable
    /// publication proof.
    ///
    /// `Ok(None)` means CURRENT is a valid named generation but this process has
    /// not completed its full publication. Callers must choose ordinary staging
    /// explicitly in that case. On non-Unix this returns `Unsupported`.
    #[cfg(unix)]
    pub fn begin_from_current_if_durable(
        &self,
        generation: GenerationName,
    ) -> io::Result<Option<CurrentGenerationStaging>> {
        let guard = self.lock_commit()?;
        let predecessor =
            match read_current_from(&self.inner.root, |_| Ok(())).map_err(current_read_as_io)? {
                CurrentTarget::Generation(name) => name,
                CurrentTarget::Empty => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "CURRENT is empty and has no inheritable generation",
                    ));
                }
            };
        if guard.durable_current.as_ref() != Some(&predecessor) {
            return Ok(None);
        }
        let staged = self.begin_locked(generation)?;
        Ok(Some(CurrentGenerationStaging {
            staged,
            predecessor,
            root: self.inner.root.clone(),
            inherited: BTreeMap::new(),
        }))
    }

    #[cfg(not(unix))]
    pub fn begin_from_current_if_durable(
        &self,
        _generation: GenerationName,
    ) -> io::Result<Option<CurrentGenerationStaging>> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "current-generation inherited sync is supported only on Unix",
        ))
    }

    /// Durably activate a stage derived from the exact current generation.
    ///
    /// Only registered, still-identical Unix hard links skip `SyncFile`.
    /// Every unregistered file and every directory and pointer durability step
    /// uses the ordinary commit policy.
    pub fn commit_from_current(
        &self,
        staged: CurrentGenerationStaging,
    ) -> Result<GenerationName, CommitError> {
        self.commit_from_current_with_publication_guard(staged, || Ok(()))
    }

    /// Commit an inherited-current stage while holding a caller domain guard
    /// through CURRENT publication.
    pub fn commit_from_current_with_publication_guard<G>(
        &self,
        staged: CurrentGenerationStaging,
        acquire: impl FnOnce() -> io::Result<G>,
    ) -> Result<GenerationName, CommitError> {
        let CurrentGenerationStaging {
            staged,
            predecessor,
            root,
            inherited,
        } = staged;
        if root != self.inner.root {
            let target = CurrentTarget::Generation(staged.generation.clone());
            return Err(self.commit_error(
                CommitFailureClass::PreCommit,
                CommitStep::ValidateStaging,
                target,
                &staged.path,
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "current-derived staging belongs to another store",
                ),
            ));
        }
        self.commit_staged_with_publication_guard(staged, Some(predecessor), inherited, acquire)
    }

    pub(super) fn verify_inherited_files(
        &self,
        staged: &StagedGeneration,
        predecessor: Option<&GenerationName>,
        inherited: &BTreeMap<PathBuf, InheritedFile>,
        target: &CurrentTarget,
        mutation: &mut Mutation<'_>,
    ) -> Result<BTreeSet<PathBuf>, CommitError> {
        if inherited.is_empty() {
            return Ok(BTreeSet::new());
        }
        let Some(predecessor) = predecessor else {
            return Err(self.commit_error(
                CommitFailureClass::PreCommit,
                CommitStep::ValidateStaging,
                target.clone(),
                &staged.path,
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "ordinary staging cannot claim inherited files",
                ),
            ));
        };
        #[cfg(unix)]
        {
            let mut skipped = BTreeSet::new();
            for (relative, registered) in inherited {
                let source_root = self.generation_path(predecessor);
                self.checked(
                    mutation,
                    CommitStep::ValidateStaging,
                    relative,
                    target,
                    &source_root,
                    || validate_real_relative_parent(&source_root, relative),
                )?;
                self.checked(
                    mutation,
                    CommitStep::ValidateStaging,
                    relative,
                    target,
                    &staged.path,
                    || validate_real_relative_parent(&staged.path, relative),
                )?;
                let source = source_root.join(relative);
                let destination = staged.path.join(relative);
                let source_identity = self.checked_value(
                    mutation,
                    CommitStep::ValidateStaging,
                    relative,
                    target,
                    &source,
                    || regular_file_identity(&source),
                )?;
                let destination_identity = self.checked_value(
                    mutation,
                    CommitStep::ValidateStaging,
                    relative,
                    target,
                    &destination,
                    || regular_file_identity(&destination),
                )?;
                if source_identity != registered.identity
                    || destination_identity != registered.identity
                {
                    return Err(self.commit_error(
                        CommitFailureClass::PreCommit,
                        CommitStep::ValidateStaging,
                        target.clone(),
                        &destination,
                        io::Error::new(
                            io::ErrorKind::InvalidData,
                            "inherited staged file changed after registration",
                        ),
                    ));
                }
                skipped.insert(relative.clone());
            }
            Ok(skipped)
        }
        #[cfg(not(unix))]
        {
            let _ = predecessor;
            let _ = mutation;
            Err(self.commit_error(
                CommitFailureClass::PreCommit,
                CommitStep::ValidateStaging,
                target.clone(),
                &staged.path,
                io::Error::new(
                    io::ErrorKind::Unsupported,
                    "current-generation inherited sync is supported only on Unix",
                ),
            ))
        }
    }
}

fn checked_relative_file(relative: &Path) -> io::Result<PathBuf> {
    if relative.as_os_str().is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "inherited file path is empty",
        ));
    }
    let mut checked = PathBuf::new();
    for component in relative.components() {
        match component {
            std::path::Component::Normal(component) => checked.push(component),
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "inherited file path is not a safe relative path",
                ));
            }
        }
    }
    if checked.as_os_str() != relative.as_os_str() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "inherited file path is not canonical",
        ));
    }
    Ok(checked)
}

fn validate_real_relative_parent(root: &Path, relative: &Path) -> io::Result<()> {
    validate_real_directory(root)?;
    let mut current = root.to_path_buf();
    let mut components = relative.components().peekable();
    while let Some(component) = components.next() {
        let std::path::Component::Normal(component) = component else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "inherited file path is not a safe relative path",
            ));
        };
        if components.peek().is_some() {
            current.push(component);
            validate_real_directory(&current)?;
        }
    }
    Ok(())
}

#[cfg(unix)]
fn regular_file_identity(path: &Path) -> io::Result<FileIdentity> {
    use std::os::unix::fs::MetadataExt;

    let metadata = std::fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "inherited file must be a regular nonsymlink file",
        ));
    }
    Ok(FileIdentity {
        dev: metadata.dev(),
        ino: metadata.ino(),
        len: metadata.len(),
        mtime: metadata.mtime(),
        mtime_nsec: metadata.mtime_nsec(),
    })
}
