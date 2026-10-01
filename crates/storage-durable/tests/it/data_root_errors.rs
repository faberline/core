//! What an `anyhow` caller can still downcast after the data-root API moved
//! to `DataRootError`, and how a policy error travels through it.

use std::fmt;
use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};
use storage_durable::{
    reject_symlink, set_private_directory_mode, set_private_file_mode, DataRoot, DataRootError,
    DataRootPolicy,
};

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Manifest {
    accepted: bool,
}

/// A product's own error type, raised from `validate_manifest`.
#[derive(Debug)]
struct Refused;

impl fmt::Display for Refused {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("the policy refused the layout")
    }
}

impl std::error::Error for Refused {}

struct Policy {
    accept: bool,
}

impl DataRootPolicy for Policy {
    type Manifest = Manifest;

    fn product_name(&self) -> &'static str {
        "downcast"
    }

    fn directories(&self) -> &'static [&'static str] {
        &[]
    }

    fn create_manifest(&self, _root: &Path) -> Result<Self::Manifest, DataRootError> {
        Ok(Manifest {
            accepted: self.accept,
        })
    }

    fn validate_manifest(&self, manifest: &Self::Manifest) -> Result<(), DataRootError> {
        if manifest.accepted {
            Ok(())
        } else {
            Err(DataRootError::other(Refused))
        }
    }
}

fn open_as_anyhow(root: &Path, policy: Policy) -> anyhow::Error {
    let open = || -> anyhow::Result<DataRoot<Policy>> { Ok(DataRoot::open(root, policy)?) };
    match open() {
        Ok(_) => panic!("opening {} must fail", root.display()),
        Err(error) => error,
    }
}

#[test]
fn an_io_failure_from_open_is_found_in_the_anyhow_chain() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("data");
    std::fs::write(&root, b"file").unwrap();

    let error = open_as_anyhow(&root, Policy { accept: true });
    let io = error
        .chain()
        .find_map(|cause| cause.downcast_ref::<io::Error>())
        .expect("the io::Error stays in the chain");
    assert_eq!(io.kind(), io::ErrorKind::AlreadyExists);
    assert!(error.root_cause().downcast_ref::<io::Error>().is_some());
    assert!(matches!(
        error.downcast_ref::<DataRootError>(),
        Some(DataRootError::Io { .. })
    ));
}

#[test]
fn a_policy_error_downcasts_through_other() {
    let temp = tempfile::tempdir().unwrap();
    let error = open_as_anyhow(&temp.path().join("data"), Policy { accept: false });
    assert_eq!(error.to_string(), "the policy refused the layout");

    let Some(DataRootError::Other(inner)) = error.downcast_ref::<DataRootError>() else {
        panic!("a policy error is DataRootError::Other");
    };
    assert!(inner.downcast_ref::<Refused>().is_some());
}

#[test]
fn other_keeps_the_wrapped_message_and_type() {
    let error = DataRootError::other(io::Error::other("disk is full"));
    assert_eq!(error.to_string(), "disk is full");
    let DataRootError::Other(inner) = error else {
        panic!("other() builds the Other variant");
    };
    assert!(inner.downcast_ref::<io::Error>().is_some());

    let wrapped = anyhow::Error::new(Refused).context("while checking");
    let error = DataRootError::other(wrapped);
    assert_eq!(format!("{error}"), "while checking");
    assert_eq!(
        format!("{:#}", anyhow::Error::from(error)),
        "while checking: the policy refused the layout"
    );
}

#[cfg(unix)]
#[test]
fn the_path_functions_keep_a_direct_io_downcast() {
    let temp = tempfile::tempdir().unwrap();
    let missing = temp.path().join("missing");

    for error in [
        set_private_file_mode(&missing).unwrap_err(),
        set_private_directory_mode(&missing).unwrap_err(),
    ] {
        let io = error
            .downcast_ref::<io::Error>()
            .expect("a direct downcast finds the io::Error");
        assert_eq!(io.kind(), io::ErrorKind::NotFound);
    }
    let error = set_private_file_mode(&missing).unwrap_err();
    let context = format!("set private file mode on {}", missing.display());
    assert_eq!(format!("{error}"), context);
    assert_eq!(
        format!("{error:#}"),
        format!("{context}: No such file or directory (os error 2)")
    );

    std::fs::write(temp.path().join("file"), b"file").unwrap();
    let under_file = temp.path().join("file").join("data");
    let error = reject_symlink(&under_file).unwrap_err();
    assert_eq!(
        format!("{error}"),
        format!("inspect {}", under_file.display())
    );
    assert_eq!(
        error.downcast_ref::<io::Error>().unwrap().raw_os_error(),
        Some(20)
    );

    let link = temp.path().join("link");
    std::os::unix::fs::symlink(temp.path(), &link).unwrap();
    let error = reject_symlink(&link).unwrap_err();
    let text = format!("data path must not be a symlink: {}", link.display());
    assert_eq!(format!("{error}"), text);
    assert_eq!(format!("{error:#}"), text);
}
