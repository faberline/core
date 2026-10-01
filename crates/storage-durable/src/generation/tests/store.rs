use super::*;

#[test]
fn generation_name_accepts_safe_legacy_and_unique_names() {
    for candidate in ["gen-42", "restore_2026.08.27-1", "A0", "0"] {
        assert_eq!(name(candidate).as_str(), candidate);
    }
}

#[test]
fn generation_name_rejects_reserved_traversal_and_non_ascii() {
    let cases = [
        ("", GenerationNameErrorKind::Empty),
        ("CURRENT", GenerationNameErrorKind::Reserved),
        ("CURRENT.tmp", GenerationNameErrorKind::Reserved),
        (".", GenerationNameErrorKind::InvalidCharacter),
        ("..", GenerationNameErrorKind::InvalidCharacter),
        ("../gen", GenerationNameErrorKind::InvalidCharacter),
        ("a/b", GenerationNameErrorKind::InvalidCharacter),
        ("a\\b", GenerationNameErrorKind::InvalidCharacter),
        ("é", GenerationNameErrorKind::InvalidCharacter),
    ];
    for (candidate, expected) in cases {
        assert_eq!(GenerationName::parse(candidate).unwrap_err().kind, expected);
    }
    assert_eq!(
        GenerationName::parse("a".repeat(129)).unwrap_err().kind,
        GenerationNameErrorKind::TooLong
    );
}

#[test]
fn begin_creates_one_unique_direct_child_stage() {
    let directory = tempfile::tempdir().unwrap();
    let store = GenerationStore::open(directory.path()).unwrap();
    let staged = store.begin(name("gen-1")).unwrap();
    assert_eq!(
        staged.path(),
        std::fs::canonicalize(directory.path())
            .unwrap()
            .join(".stage-gen-1")
    );
    assert!(staged.path().is_dir());
    assert_eq!(
        store.begin(name("gen-1")).unwrap_err().kind(),
        io::ErrorKind::AlreadyExists
    );
    std::fs::create_dir(directory.path().join("gen-2")).unwrap();
    assert_eq!(
        store.begin(name("gen-2")).unwrap_err().kind(),
        io::ErrorKind::AlreadyExists
    );
}

#[test]
fn independent_opens_share_one_process_root_lock() {
    let directory = tempfile::tempdir().unwrap();
    let first = GenerationStore::open(directory.path()).unwrap();
    let second = GenerationStore::open(directory.path()).unwrap();
    assert!(Arc::ptr_eq(&first.inner.commit, &second.inner.commit));
}
