//! Pins the `layout.json` bytes a data root writes, and the text of every
//! `DataRoot` error a test can provoke as an `anyhow` caller sees it: `{}` and
//! the `{:#}` chain.

mod fixture;

use std::path::{Path, PathBuf};

use fixture::{GoldenManifest, GoldenPolicy};
use storage_durable::{DataRoot, DataRootPolicy};

const STORE_LAYOUT: &str = "{\n  \"version\": 1,\n  \"role\": \"store\"\n}";
const RESTORED_LAYOUT: &str = "{\n  \"version\": 1,\n  \"role\": \"restored\"\n}";

fn as_anyhow(error: impl Into<anyhow::Error>) -> anyhow::Error {
    error.into()
}

fn open_error(root: &Path, policy: GoldenPolicy) -> anyhow::Error {
    match DataRoot::open(root, policy) {
        Ok(_) => panic!("opening {} must fail", root.display()),
        Err(error) => as_anyhow(error),
    }
}

fn assert_text(error: &anyhow::Error, display: &str, chain: &str) {
    assert_eq!(format!("{error}"), display);
    assert_eq!(format!("{error:#}"), chain);
}

fn fresh_root() -> (tempfile::TempDir, PathBuf) {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("data");
    (temp, root)
}

#[test]
fn a_new_root_writes_the_pinned_layout() {
    let (_temp, root) = fresh_root();
    let mut data_root = DataRoot::open(&root, GoldenPolicy::new()).unwrap();
    assert_eq!(data_root.manifest(), &GoldenManifest::with_role("store"));
    assert_eq!(
        std::fs::read_to_string(root.join("layout.json")).unwrap(),
        STORE_LAYOUT
    );

    data_root
        .replace_manifest(GoldenManifest::with_role("restored"))
        .unwrap();
    assert_eq!(
        std::fs::read_to_string(root.join("layout.json")).unwrap(),
        RESTORED_LAYOUT
    );
}

#[test]
fn the_pinned_layout_decodes_to_its_manifest() {
    let (_temp, root) = fresh_root();
    std::fs::create_dir(&root).unwrap();
    std::fs::write(root.join("layout.json"), RESTORED_LAYOUT).unwrap();

    let data_root = DataRoot::open(&root, GoldenPolicy::new()).unwrap();
    assert_eq!(data_root.manifest(), &GoldenManifest::with_role("restored"));
    assert_eq!(
        std::fs::read_to_string(root.join("layout.json")).unwrap(),
        RESTORED_LAYOUT
    );
}

#[test]
fn the_default_legacy_error_keeps_its_text() {
    let error = GoldenPolicy::new().legacy_error(Path::new("/srv/data/legacy.data"));
    let text = "legacy golden data at /srv/data/legacy.data is not compatible";
    assert_eq!(format!("{error}"), text);
    assert_eq!(format!("{error:#}"), text);

    let (_temp, root) = fresh_root();
    std::fs::create_dir(&root).unwrap();
    std::fs::write(root.join("legacy.data"), b"keep").unwrap();
    let text = format!(
        "legacy golden data at {} is not compatible",
        root.join("legacy.data").display()
    );
    assert_text(&open_error(&root, GoldenPolicy::new()), &text, &text);
}

#[test]
fn a_root_that_is_a_file_keeps_its_create_error_text() {
    let (_temp, root) = fresh_root();
    std::fs::write(&root, b"file").unwrap();
    let context = format!("create golden data directory {}", root.display());
    assert_text(
        &open_error(&root, GoldenPolicy::new()),
        &context,
        &format!("{context}: File exists (os error 17)"),
    );
}

#[cfg(unix)]
#[test]
fn a_root_under_a_file_keeps_its_inspect_error_text() {
    let (temp, _) = fresh_root();
    std::fs::write(temp.path().join("file"), b"file").unwrap();
    let root = temp.path().join("file").join("data");
    let context = format!("inspect {}", root.display());
    assert_text(
        &open_error(&root, GoldenPolicy::new()),
        &context,
        &format!("{context}: Not a directory (os error 20)"),
    );
}

#[cfg(unix)]
#[test]
fn symlinks_keep_their_error_text() {
    use std::os::unix::fs::symlink;

    let (temp, root) = fresh_root();
    std::fs::create_dir(temp.path().join("target")).unwrap();
    symlink(temp.path().join("target"), &root).unwrap();
    let text = format!("data path must not be a symlink: {}", root.display());
    assert_text(&open_error(&root, GoldenPolicy::new()), &text, &text);

    let (temp, root) = fresh_root();
    std::fs::create_dir(&root).unwrap();
    std::fs::write(temp.path().join("layout.json"), STORE_LAYOUT).unwrap();
    symlink(temp.path().join("layout.json"), root.join("layout.json")).unwrap();
    let text = format!(
        "data path must not be a symlink: {}",
        root.join("layout.json").display()
    );
    assert_text(&open_error(&root, GoldenPolicy::new()), &text, &text);
}

#[test]
fn a_layout_that_is_a_directory_keeps_its_error_text() {
    let (_temp, root) = fresh_root();
    std::fs::create_dir_all(root.join("layout.json")).unwrap();
    let text = format!(
        "data path must be a regular file: {}",
        root.join("layout.json").display()
    );
    assert_text(&open_error(&root, GoldenPolicy::new()), &text, &text);
}

#[test]
fn a_locked_root_keeps_its_error_text() {
    let (_temp, root) = fresh_root();
    let _owner = DataRoot::open(&root, GoldenPolicy::new()).unwrap();
    let error = open_error(&root, GoldenPolicy::new());
    let context = format!(
        "lock golden data root {}; another golden process may be using it",
        root.display()
    );
    assert_eq!(format!("{error}"), context);
    #[cfg(target_os = "macos")]
    assert_eq!(
        format!("{error:#}"),
        format!("{context}: Resource temporarily unavailable (os error 35)")
    );
    #[cfg(target_os = "linux")]
    assert_eq!(
        format!("{error:#}"),
        format!("{context}: Resource temporarily unavailable (os error 11)")
    );
}

#[cfg(unix)]
#[test]
fn an_unreadable_layout_keeps_its_read_error_text() {
    use std::os::unix::fs::PermissionsExt;

    let (_temp, root) = fresh_root();
    std::fs::create_dir(&root).unwrap();
    let layout = root.join("layout.json");
    std::fs::write(&layout, STORE_LAYOUT).unwrap();
    std::fs::set_permissions(&layout, std::fs::Permissions::from_mode(0o000)).unwrap();
    if std::fs::read(&layout).is_ok() {
        // A superuser reads the file anyway, so there is no error to pin.
        return;
    }
    let context = format!("read layout {}", layout.display());
    assert_text(
        &open_error(&root, GoldenPolicy::new()),
        &context,
        &format!("{context}: Permission denied (os error 13)"),
    );
}

#[test]
fn an_undecodable_layout_keeps_its_error_text() {
    let (_temp, root) = fresh_root();
    std::fs::create_dir(&root).unwrap();
    std::fs::write(root.join("layout.json"), b"not json").unwrap();
    let context = format!("decode layout {}", root.join("layout.json").display());
    assert_text(
        &open_error(&root, GoldenPolicy::new()),
        &context,
        &format!("{context}: expected ident at line 1 column 2"),
    );
}

#[test]
fn a_rejected_layout_keeps_the_policy_text() {
    let (_temp, root) = fresh_root();
    std::fs::create_dir(&root).unwrap();
    std::fs::write(
        root.join("layout.json"),
        "{\n  \"version\": 2,\n  \"role\": \"store\"\n}",
    )
    .unwrap();
    let text = "unsupported golden format 2";
    assert_text(&open_error(&root, GoldenPolicy::new()), text, text);
}

#[test]
fn an_unsafe_directory_keeps_its_error_text() {
    let (_temp, root) = fresh_root();
    let policy = GoldenPolicy {
        directories: &["../escape"],
    };
    let text = "data-root directory must be a safe relative path: ../escape";
    assert_text(&open_error(&root, policy), text, text);
}

#[test]
fn a_directory_that_is_a_file_keeps_its_error_text() {
    let (_temp, root) = fresh_root();
    std::fs::create_dir(&root).unwrap();
    std::fs::write(root.join("wal"), b"file").unwrap();
    let text = format!(
        "data path must be a real directory: {}",
        root.join("wal").display()
    );
    assert_text(&open_error(&root, GoldenPolicy::new()), &text, &text);
}

#[test]
fn a_blocked_layout_write_keeps_its_error_text() {
    let (_temp, root) = fresh_root();
    std::fs::create_dir_all(root.join("layout.json.tmp")).unwrap();
    let context = format!("create temp {}", root.join("layout.json.tmp").display());
    assert_text(
        &open_error(&root, GoldenPolicy::new()),
        &context,
        &format!("{context}: Is a directory (os error 21)"),
    );
}

#[test]
fn replace_manifest_errors_keep_their_text() {
    let (_temp, root) = fresh_root();
    let mut data_root = DataRoot::open(&root, GoldenPolicy::new()).unwrap();

    let mut rejected = GoldenManifest::with_role("store");
    rejected.version = 3;
    let error = as_anyhow(data_root.replace_manifest(rejected).unwrap_err());
    let text = "unsupported golden format 3";
    assert_text(&error, text, text);

    let mut unencodable = GoldenManifest::with_role("store");
    unencodable.unencodable.insert(vec![1], 1);
    let error = as_anyhow(data_root.replace_manifest(unencodable).unwrap_err());
    assert_text(
        &error,
        "encode data-root layout",
        "encode data-root layout: key must be a string",
    );

    assert_eq!(data_root.manifest(), &GoldenManifest::with_role("store"));
    assert_eq!(
        std::fs::read_to_string(root.join("layout.json")).unwrap(),
        STORE_LAYOUT
    );
}
