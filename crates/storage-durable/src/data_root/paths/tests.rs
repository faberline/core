use super::*;
use std::fs;
use std::os::unix::fs::PermissionsExt;

use tempfile::tempdir;

/// `EPERM` on every unix: the `chmod` that a non-owner gets.
fn eperm(_: &Path) -> io::Result<()> {
    Err(io::Error::from_raw_os_error(1))
}

/// `EIO`: an error that must never be swallowed.
fn eio(_: &Path) -> io::Result<()> {
    Err(io::Error::from_raw_os_error(5))
}

fn mode_of(path: &Path) -> u32 {
    fs::metadata(path).unwrap().permissions().mode() & 0o777
}

fn with_mode(path: &Path, mode: u32) {
    fs::set_permissions(path, fs::Permissions::from_mode(mode)).unwrap();
}

#[test]
fn owner_case_sets_0700() {
    let dir = tempdir().unwrap();
    with_mode(dir.path(), 0o755);
    private_directory_mode(dir.path()).unwrap();
    assert_eq!(mode_of(dir.path()), 0o700);
}

#[test]
fn foreign_owner_with_no_other_access_is_accepted() {
    // The fsGroup case: the group can write, `other` has no access.
    let dir = tempdir().unwrap();
    with_mode(dir.path(), 0o770);
    private_directory_mode_with(dir.path(), eperm).unwrap();
    assert_eq!(mode_of(dir.path()), 0o770);
}

#[test]
fn foreign_owner_open_to_other_is_refused() {
    // `0o701` gives `other` search access only, and that is still access.
    for mode in [0o775, 0o701] {
        let dir = tempdir().unwrap();
        with_mode(dir.path(), mode);
        match private_directory_mode_with(dir.path(), eperm) {
            Err(DataRootError::Io { source, .. }) => {
                assert_eq!(source.kind(), io::ErrorKind::PermissionDenied);
            }
            other => panic!("mode {mode:o}: expected an io error, got {other:?}"),
        }
    }
}

#[test]
fn foreign_owner_unreadable_root_is_refused() {
    let dir = tempdir().unwrap();
    let path = dir.path();
    with_mode(path, 0o000);
    if fs::read_dir(path).is_ok() {
        // Running as root: every directory is readable, so this case cannot arise.
        with_mode(path, 0o700);
        return;
    }
    let result = private_directory_mode_with(path, eperm);
    with_mode(path, 0o700);
    assert!(
        matches!(result, Err(DataRootError::Io { .. })),
        "expected an io error, got {result:?}"
    );
}

#[test]
fn other_errors_are_not_swallowed() {
    let dir = tempdir().unwrap();
    with_mode(dir.path(), 0o770);
    assert!(matches!(
        private_directory_mode_with(dir.path(), eio),
        Err(DataRootError::Io { .. })
    ));
}
