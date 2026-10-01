//! Private, versioned, single-process data-root mechanics.
//!
//! The library owns filesystem safety. A service supplies its manifest and
//! compatibility policy through [`DataRootPolicy`].

mod error;
mod paths;

use std::{
    fs::{self, File, OpenOptions},
    path::{Component, Path, PathBuf},
};

use fs2::FileExt;
use serde::{de::DeserializeOwned, Serialize};

use self::error::IoContext;
use self::paths::{
    check_not_symlink, private_directory_mode, private_file_mode, require_directory,
    require_regular_file,
};
use crate::{atomic_write, FsyncPolicy};

pub use self::error::DataRootError;
pub use self::paths::{reject_symlink, set_private_directory_mode, set_private_file_mode};

/// Service-owned manifest and compatibility hooks for a shared data root.
///
/// An implementation wraps its own failures with [`DataRootError::other`].
pub trait DataRootPolicy {
    type Manifest: Clone + Serialize + DeserializeOwned;

    fn product_name(&self) -> &'static str;

    fn manifest_file(&self) -> &'static str {
        "layout.json"
    }

    fn directories(&self) -> &'static [&'static str];

    fn legacy_markers(&self) -> &'static [&'static str] {
        &[]
    }

    fn create_manifest(&self, root: &Path) -> Result<Self::Manifest, DataRootError>;

    fn validate_manifest(&self, manifest: &Self::Manifest) -> Result<(), DataRootError>;

    fn legacy_error(&self, marker: &Path) -> DataRootError {
        DataRootError::LegacyData {
            product: self.product_name(),
            marker: marker.to_path_buf(),
        }
    }
}

/// Holds the exclusive directory lock for the life of one service process.
pub struct DataRoot<P: DataRootPolicy> {
    root: PathBuf,
    manifest_path: PathBuf,
    manifest: P::Manifest,
    policy: P,
    _root_lock: File,
}

impl<P: DataRootPolicy> DataRoot<P> {
    pub fn open(root: impl AsRef<Path>, policy: P) -> Result<Self, DataRootError> {
        let root = root.as_ref();
        check_not_symlink(root)?;
        fs::create_dir_all(root).io_context(|| {
            format!(
                "create {} data directory {}",
                policy.product_name(),
                root.display()
            )
        })?;
        require_directory(root)?;
        private_directory_mode(root)?;

        let manifest_path = root.join(policy.manifest_file());
        check_not_symlink(&manifest_path)?;
        if !manifest_path.exists() {
            refuse_legacy_root(root, &policy)?;
        } else {
            require_regular_file(&manifest_path)?;
        }

        let root_lock = OpenOptions::new().read(true).open(root).io_context(|| {
            format!(
                "open {} data root {} for locking",
                policy.product_name(),
                root.display()
            )
        })?;
        root_lock.try_lock_exclusive().io_context(|| {
            format!(
                "lock {} data root {}; another {} process may be using it",
                policy.product_name(),
                root.display(),
                policy.product_name()
            )
        })?;

        let manifest = if manifest_path.exists() {
            let bytes = fs::read(&manifest_path)
                .io_context(|| format!("read layout {}", manifest_path.display()))?;
            let manifest =
                serde_json::from_slice(&bytes).map_err(|source| DataRootError::ManifestDecode {
                    path: manifest_path.clone(),
                    source,
                })?;
            policy.validate_manifest(&manifest)?;
            private_file_mode(&manifest_path)?;
            manifest
        } else {
            let manifest = policy.create_manifest(root)?;
            policy.validate_manifest(&manifest)?;
            write_manifest(&manifest_path, &manifest)?;
            manifest
        };

        for relative in policy.directories() {
            ensure_private_relative_directory(root, relative)?;
        }

        Ok(Self {
            root: root.to_path_buf(),
            manifest_path,
            manifest,
            policy,
            _root_lock: root_lock,
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn manifest(&self) -> &P::Manifest {
        &self.manifest
    }

    pub fn manifest_path(&self) -> &Path {
        &self.manifest_path
    }

    pub fn replace_manifest(&mut self, manifest: P::Manifest) -> Result<(), DataRootError> {
        self.policy.validate_manifest(&manifest)?;
        write_manifest(&self.manifest_path, &manifest)?;
        self.manifest = manifest;
        Ok(())
    }
}

fn write_manifest<T: Serialize>(path: &Path, manifest: &T) -> Result<(), DataRootError> {
    let bytes = serde_json::to_vec_pretty(manifest)
        .map_err(|source| DataRootError::ManifestEncode { source })?;
    atomic_write(path, &bytes, FsyncPolicy::Always).map_err(DataRootError::from_atomic_write)?;
    private_file_mode(path)
}

fn refuse_legacy_root<P: DataRootPolicy>(root: &Path, policy: &P) -> Result<(), DataRootError> {
    if let Some(marker) = policy
        .legacy_markers()
        .iter()
        .map(|marker| root.join(marker))
        .find(|path| path.exists())
    {
        return Err(policy.legacy_error(&marker));
    }
    Ok(())
}

fn ensure_private_relative_directory(root: &Path, relative: &str) -> Result<(), DataRootError> {
    let relative_path = Path::new(relative);
    if relative_path.is_absolute()
        || relative_path
            .components()
            .any(|component| !matches!(component, Component::Normal(_) | Component::CurDir))
    {
        return Err(DataRootError::UnsafeDirectory {
            relative: relative.to_owned(),
        });
    }

    let mut current = root.to_path_buf();
    for component in relative_path.components() {
        if let Component::Normal(component) = component {
            current.push(component);
            check_not_symlink(&current)?;
            match fs::symlink_metadata(&current) {
                Ok(_) => require_directory(&current)?,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    fs::create_dir(&current)
                        .io_context(|| format!("create storage directory {}", current.display()))?;
                }
                Err(error) => {
                    return Err(error).io_context(|| format!("inspect {}", current.display()))
                }
            }
            private_directory_mode(&current)?;
        }
    }
    Ok(())
}
