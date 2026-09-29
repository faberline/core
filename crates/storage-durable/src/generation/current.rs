use std::fmt;
use std::fs::File;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use super::name::GenerationName;

/// The durable pointer file.
pub const CURRENT_FILE_NAME: &str = "CURRENT";
/// The sibling used to prepare a new durable pointer.
pub const CURRENT_TEMP_FILE_NAME: &str = "CURRENT.tmp";
/// The exact pointer bytes for an initialized store with no generation.
pub const EMPTY_CURRENT_BYTES: &[u8] = b"empty\n";

const CURRENT_PREFIX: &[u8] = b"generation:";
const MAX_CURRENT_BYTES: usize = 256;

/// The target selected by `CURRENT`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CurrentTarget {
    Empty,
    Generation(GenerationName),
}

/// Stable classification for a rejected `CURRENT` pointer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CurrentReadErrorKind {
    Missing,
    PointerNotRegular,
    PointerTooLarge,
    Malformed,
    UnsafeTarget,
    TargetMissing,
    TargetNotDirectory,
    TargetSymlink,
    Io,
}

/// A `CURRENT` read failure. It never triggers generation auto-selection.
#[derive(Debug)]
pub struct CurrentReadError {
    pub kind: CurrentReadErrorKind,
    pub path: PathBuf,
    pub source: Option<io::Error>,
}

impl fmt::Display for CurrentReadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "read durable current pointer {}: {:?}",
            self.path.display(),
            self.kind
        )
    }
}

impl std::error::Error for CurrentReadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.source
            .as_ref()
            .map(|source| source as &(dyn std::error::Error + 'static))
    }
}

pub(super) fn current_bytes(target: &CurrentTarget) -> Vec<u8> {
    match target {
        CurrentTarget::Empty => EMPTY_CURRENT_BYTES.to_vec(),
        CurrentTarget::Generation(generation) => {
            format!("generation:{}\n", generation.as_str()).into_bytes()
        }
    }
}

pub(super) fn read_current_from(
    root: &Path,
    mut before: impl FnMut(&Path) -> io::Result<()>,
) -> Result<CurrentTarget, CurrentReadError> {
    let pointer = root.join(CURRENT_FILE_NAME);
    before(Path::new(CURRENT_FILE_NAME)).map_err(|error| CurrentReadError {
        kind: CurrentReadErrorKind::Io,
        path: pointer.clone(),
        source: Some(error),
    })?;
    let metadata = match std::fs::symlink_metadata(&pointer) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Err(CurrentReadError {
                kind: CurrentReadErrorKind::Missing,
                path: pointer,
                source: Some(error),
            });
        }
        Err(error) => {
            return Err(CurrentReadError {
                kind: CurrentReadErrorKind::Io,
                path: pointer,
                source: Some(error),
            });
        }
    };
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(CurrentReadError {
            kind: CurrentReadErrorKind::PointerNotRegular,
            path: pointer,
            source: None,
        });
    }
    if metadata.len() > MAX_CURRENT_BYTES as u64 {
        return Err(CurrentReadError {
            kind: CurrentReadErrorKind::PointerTooLarge,
            path: pointer,
            source: None,
        });
    }

    before(Path::new(CURRENT_FILE_NAME)).map_err(|error| CurrentReadError {
        kind: CurrentReadErrorKind::Io,
        path: pointer.clone(),
        source: Some(error),
    })?;
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    let read = File::open(&pointer).and_then(|file| {
        file.take((MAX_CURRENT_BYTES + 1) as u64)
            .read_to_end(&mut bytes)
    });
    if let Err(error) = read {
        return Err(CurrentReadError {
            kind: CurrentReadErrorKind::Io,
            path: pointer,
            source: Some(error),
        });
    }
    if bytes.len() > MAX_CURRENT_BYTES {
        return Err(CurrentReadError {
            kind: CurrentReadErrorKind::PointerTooLarge,
            path: pointer,
            source: None,
        });
    }
    if bytes == EMPTY_CURRENT_BYTES {
        return Ok(CurrentTarget::Empty);
    }

    let Some(raw_name) = bytes
        .strip_prefix(CURRENT_PREFIX)
        .and_then(|value| value.strip_suffix(b"\n"))
    else {
        return Err(CurrentReadError {
            kind: CurrentReadErrorKind::Malformed,
            path: pointer,
            source: None,
        });
    };
    let raw_name = std::str::from_utf8(raw_name).map_err(|_| CurrentReadError {
        kind: CurrentReadErrorKind::Malformed,
        path: pointer.clone(),
        source: None,
    })?;
    let generation = GenerationName::parse(raw_name.to_owned()).map_err(|_| CurrentReadError {
        kind: CurrentReadErrorKind::UnsafeTarget,
        path: pointer.clone(),
        source: None,
    })?;
    let target = root.join(generation.as_str());
    before(Path::new(generation.as_str())).map_err(|error| CurrentReadError {
        kind: CurrentReadErrorKind::Io,
        path: target.clone(),
        source: Some(error),
    })?;
    let target_metadata = match std::fs::symlink_metadata(&target) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Err(CurrentReadError {
                kind: CurrentReadErrorKind::TargetMissing,
                path: target,
                source: Some(error),
            });
        }
        Err(error) => {
            return Err(CurrentReadError {
                kind: CurrentReadErrorKind::Io,
                path: target,
                source: Some(error),
            });
        }
    };
    if target_metadata.file_type().is_symlink() {
        return Err(CurrentReadError {
            kind: CurrentReadErrorKind::TargetSymlink,
            path: target,
            source: None,
        });
    }
    if !target_metadata.is_dir() {
        return Err(CurrentReadError {
            kind: CurrentReadErrorKind::TargetNotDirectory,
            path: target,
            source: None,
        });
    }
    Ok(CurrentTarget::Generation(generation))
}

pub(super) fn current_read_as_io(error: CurrentReadError) -> io::Error {
    let kind = match error.kind {
        CurrentReadErrorKind::Missing | CurrentReadErrorKind::TargetMissing => {
            io::ErrorKind::NotFound
        }
        CurrentReadErrorKind::PointerNotRegular
        | CurrentReadErrorKind::PointerTooLarge
        | CurrentReadErrorKind::Malformed
        | CurrentReadErrorKind::UnsafeTarget
        | CurrentReadErrorKind::TargetNotDirectory
        | CurrentReadErrorKind::TargetSymlink => io::ErrorKind::InvalidData,
        CurrentReadErrorKind::Io => error
            .source
            .as_ref()
            .map(io::Error::kind)
            .unwrap_or(io::ErrorKind::Other),
    };
    io::Error::new(kind, error)
}
