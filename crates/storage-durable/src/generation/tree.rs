use std::fs::OpenOptions;
use std::io;
use std::path::{Path, PathBuf};

pub(super) struct TreeEntries {
    pub(super) files: Vec<PathBuf>,
    pub(super) directories: Vec<PathBuf>,
}

pub(super) fn collect_tree(
    root: &Path,
    mut before: impl FnMut(&Path) -> io::Result<()>,
) -> Result<TreeEntries, (PathBuf, io::Error)> {
    before(Path::new(".")).map_err(|error| (root.to_path_buf(), error))?;
    validate_real_directory(root).map_err(|error| (root.to_path_buf(), error))?;
    let mut files = Vec::new();
    let mut directories = vec![PathBuf::from(".")];
    collect_directory(
        root,
        Path::new(""),
        &mut files,
        &mut directories,
        &mut before,
    )?;
    files.sort();
    directories.sort_by(|left, right| {
        directory_depth(right)
            .cmp(&directory_depth(left))
            .then_with(|| left.cmp(right))
    });
    Ok(TreeEntries { files, directories })
}

fn collect_directory(
    root: &Path,
    relative: &Path,
    files: &mut Vec<PathBuf>,
    directories: &mut Vec<PathBuf>,
    before: &mut impl FnMut(&Path) -> io::Result<()>,
) -> Result<(), (PathBuf, io::Error)> {
    let directory = if relative.as_os_str().is_empty() {
        root.to_path_buf()
    } else {
        root.join(relative)
    };
    let directory_relative = if relative.as_os_str().is_empty() {
        Path::new(".")
    } else {
        relative
    };
    before(directory_relative).map_err(|error| (directory.clone(), error))?;
    let mut entries: Vec<PathBuf> = std::fs::read_dir(&directory)
        .map_err(|error| (directory.clone(), error))?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<io::Result<_>>()
        .map_err(|error| (directory.clone(), error))?;
    entries.sort();
    for child in entries {
        let child_relative = relative.join(PathBuf::from(child.file_name().ok_or_else(|| {
            (
                child.clone(),
                io::Error::new(io::ErrorKind::InvalidData, "directory entry has no name"),
            )
        })?));
        before(&child_relative).map_err(|error| (child.clone(), error))?;
        let metadata = std::fs::symlink_metadata(&child).map_err(|error| (child.clone(), error))?;
        let file_type = metadata.file_type();
        if file_type.is_symlink() {
            return Err((
                child,
                io::Error::new(io::ErrorKind::InvalidData, "generation contains a symlink"),
            ));
        }
        if metadata.is_file() {
            files.push(child_relative);
        } else if metadata.is_dir() {
            directories.push(child_relative.clone());
            collect_directory(root, &child_relative, files, directories, before)?;
        } else {
            return Err((
                child,
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "generation contains a special filesystem entry",
                ),
            ));
        }
    }
    Ok(())
}

fn directory_depth(path: &Path) -> usize {
    if path == Path::new(".") {
        0
    } else {
        path.components().count()
    }
}

pub(super) fn validate_real_directory(path: &Path) -> io::Result<()> {
    let metadata = std::fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("expected real directory: {}", path.display()),
        ));
    }
    Ok(())
}

pub(super) fn strict_sync_directory(path: &Path) -> io::Result<()> {
    OpenOptions::new().read(true).open(path)?.sync_all()
}

pub(super) fn path_exists(path: &Path) -> io::Result<bool> {
    match std::fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}
