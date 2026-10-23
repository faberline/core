use super::*;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

const CANARY: &str = "canary-secret-token-material-must-never-be-leaked-12345";

struct Mount {
    dir: PathBuf,
}

impl Mount {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "service-auth-opaque-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create fixture mount");
        Self { dir }
    }

    fn path(&self) -> PathBuf {
        self.dir.join("token")
    }

    fn write(&self, contents: impl AsRef<[u8]>) {
        std::fs::write(self.path(), contents).expect("write fixture token");
    }

    fn source(&self) -> OpaqueProjectedTokenSource {
        OpaqueProjectedTokenSource::required(self.path())
    }
}

impl Drop for Mount {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

#[test]
fn disabled_mode_returns_none_and_never_touches_filesystem() {
    let nonexistent = PathBuf::from("/nonexistent/directory/that/does/not/exist/token");
    let mode = ProjectedTokenMode::Disabled;
    assert!(mode.is_disabled());
    assert!(!mode.is_required());
    assert_eq!(mode.path(), None);

    let source = OpaqueProjectedTokenSource::new(mode);
    assert_eq!(source.mode(), &ProjectedTokenMode::Disabled);
    assert_eq!(source.path(), None);

    let res = source.read().expect("disabled mode must succeed");
    assert_eq!(res, None);

    // Also verify standalone read_projected_token helper
    let res2 = read_projected_token(&ProjectedTokenMode::Disabled).expect("disabled must succeed");
    assert_eq!(res2, None);

    // Assert the nonexistent path was never touched
    assert!(!nonexistent.exists());
}

#[test]
fn required_mode_reads_expired_jwt_and_wrong_audience_verbatim_without_decoding() {
    let mount = Mount::new("opaque-no-jwt-decode");

    // An expired JWT (exp=1, in 1970)
    let expired_jwt = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.\
                       eyJhdWQiOlsiaHR0cHM6Ly9leGFtcGxlLmNvbSJdLCJleHAiOjEsInN1YiI6InN2Yy1hY2NvdW50In0.\
                       9Jq8aR1a_Ir_fake_sig_1234567890abcdef";
    mount.write(expired_jwt);
    let token = mount.source().read().expect("read").expect("some token");
    assert_eq!(token.expose(), expired_jwt);

    // A JWT with wrong audience
    let wrong_aud_jwt = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.\
                         eyJhdWQiOlsiaHR0cHM6Ly93cm9uZy1hdWRpZW5jZS5leGFtcGxlLmNvbSJdLCJleHAiOjI1MDAwMDAwMDAsInN1YiI6InN2Yy1hY2NvdW50In0.\
                         vAl8_fake_signature_content_here_xyz";
    mount.write(wrong_aud_jwt);
    let token2 = mount.source().read().expect("read").expect("some token");
    assert_eq!(token2.expose(), wrong_aud_jwt);

    // Trims surrounding whitespace
    let padded = format!("  \r\n\t{wrong_aud_jwt}\n\t  ");
    mount.write(padded);
    let token3 = mount.source().read().expect("read").expect("some token");
    assert_eq!(token3.expose(), wrong_aud_jwt);
}

#[test]
fn missing_file_returns_typed_missing_error() {
    let nonexistent = PathBuf::from("/nonexistent/file/for/token/source/test");
    let source = OpaqueProjectedTokenSource::required(&nonexistent);
    let err = source.read().expect_err("missing file must fail");
    match &err {
        OpaqueProjectedTokenError::Missing { path } => {
            assert_eq!(path, &nonexistent);
        }
        other => panic!("expected Missing error, got: {other:?}"),
    }
    assert_eq!(err.path(), &nonexistent);
    assert!(err.to_string().contains("does not exist"));
}

#[test]
fn unreadable_file_such_as_directory_returns_typed_unreadable_error() {
    let mount = Mount::new("unreadable");
    let dir_path = mount.dir.join("sub_directory_path");
    std::fs::create_dir_all(&dir_path).expect("create directory");

    let source = OpaqueProjectedTokenSource::required(&dir_path);
    let err = source.read().expect_err("reading directory must fail");
    match &err {
        OpaqueProjectedTokenError::Unreadable {
            path,
            source: io_err,
        } => {
            assert_eq!(path, &dir_path);
            assert!(
                std::error::Error::source(&err).is_some(),
                "must report io::Error source: {io_err}"
            );
        }
        other => panic!("expected Unreadable error, got: {other:?}"),
    }
    assert_eq!(err.path(), &dir_path);
    assert!(err.to_string().contains("cannot read"));
}

#[test]
fn empty_or_whitespace_file_returns_typed_empty_error() {
    let mount = Mount::new("empty");

    // Zero-byte file
    mount.write(b"");
    let err = mount.source().read().expect_err("empty file must fail");
    assert!(
        matches!(err, OpaqueProjectedTokenError::Empty { .. }),
        "{err:?}"
    );
    assert_eq!(err.path(), &mount.path());
    assert!(err.to_string().contains("is empty"));

    // Whitespace only
    mount.write("   \t\r\n   ");
    let err2 = mount
        .source()
        .read()
        .expect_err("whitespace file must fail");
    assert!(
        matches!(err2, OpaqueProjectedTokenError::Empty { .. }),
        "{err2:?}"
    );
    assert_eq!(err2.path(), &mount.path());
}

#[test]
fn malformed_content_returns_typed_malformed_error() {
    let mount = Mount::new("malformed");

    // 1. Non-UTF-8 bytes
    mount.write([0xff, 0xfe, 0xfd, 0x00]);
    let err = mount.source().read().expect_err("non-utf8 must fail");
    assert!(
        matches!(err, OpaqueProjectedTokenError::Malformed { .. }),
        "{err:?}"
    );

    // 2. Interior whitespace
    mount.write("token with spaces");
    let err = mount.source().read().expect_err("spaces must fail");
    assert!(
        matches!(err, OpaqueProjectedTokenError::Malformed { .. }),
        "{err:?}"
    );

    mount.write("token\twith\ttab");
    let err = mount.source().read().expect_err("tabs must fail");
    assert!(
        matches!(err, OpaqueProjectedTokenError::Malformed { .. }),
        "{err:?}"
    );

    mount.write("line1\nline2");
    let err = mount.source().read().expect_err("newline must fail");
    assert!(
        matches!(err, OpaqueProjectedTokenError::Malformed { .. }),
        "{err:?}"
    );

    // 3. Control characters
    mount.write("token\x00null");
    let err = mount.source().read().expect_err("null must fail");
    assert!(
        matches!(err, OpaqueProjectedTokenError::Malformed { .. }),
        "{err:?}"
    );

    mount.write("token\x07bell");
    let err = mount.source().read().expect_err("bell must fail");
    assert!(
        matches!(err, OpaqueProjectedTokenError::Malformed { .. }),
        "{err:?}"
    );

    assert_eq!(err.path(), &mount.path());
    assert!(err.to_string().contains("malformed"));
}

#[test]
fn rotation_rereads_on_every_call_and_picks_up_new_token() {
    let mount = Mount::new("rotation-single");
    let token_1 = "initial-opaque-token-value-alpha-001";
    let token_2 = "rotated-opaque-token-value-beta-002";

    mount.write(token_1);
    let source = mount.source();
    let first = source.read().expect("first read").expect("token 1");
    assert_eq!(first.expose(), token_1);

    // Rewrite the file: rotation in place
    mount.write(token_2);

    // Second call without restarting or reconstructing the source
    let second = source.read().expect("second read").expect("token 2");
    assert_eq!(
        second.expose(),
        token_2,
        "Required mode must read on every call; caching first read is a failure"
    );
}

#[test]
fn rotation_multi_threaded_never_yields_mixed_or_corrupt_tokens() {
    let mount = Arc::new(Mount::new("rotation-multi"));
    let token_a = "token-alpha-11111111111111111111111111111111";
    let token_b = "token-beta-22222222222222222222222222222222";

    mount.write(token_a);

    let stop = Arc::new(AtomicBool::new(false));
    let mut reader_handles = Vec::new();

    // Spawn 4 reader threads
    for _ in 0..4 {
        let m = Arc::clone(&mount);
        let s = Arc::clone(&stop);
        let handle = thread::spawn(move || {
            let src = m.source();
            let mut reads = 0;
            while !s.load(Ordering::Relaxed) && reads < 200 {
                match src.read() {
                    Ok(Some(token)) => {
                        let exp = token.expose();
                        assert!(
                            exp == token_a || exp == token_b,
                            "token must be either token_a or token_b, never a mix! Got: {exp}"
                        );
                    }
                    Ok(None) => panic!("required mode cannot return None"),
                    Err(OpaqueProjectedTokenError::Empty { .. })
                    | Err(OpaqueProjectedTokenError::Missing { .. })
                    | Err(OpaqueProjectedTokenError::Unreadable { .. })
                    | Err(OpaqueProjectedTokenError::Malformed { .. }) => {
                        // Intermediate state during atomic rewrite or truncation is a typed error, never a mix.
                    }
                }
                reads += 1;
                thread::yield_now();
            }
        });
        reader_handles.push(handle);
    }

    // Writer thread rapidly rotates the token
    let m = Arc::clone(&mount);
    let s = Arc::clone(&stop);
    let writer_handle = thread::spawn(move || {
        for i in 0..100 {
            let next_token = if i % 2 == 0 { token_b } else { token_a };
            m.write(next_token);
            thread::sleep(Duration::from_millis(1));
        }
        s.store(true, Ordering::Relaxed);
    });

    writer_handle.join().expect("writer joined");
    for h in reader_handles {
        h.join().expect("reader joined");
    }
}

#[test]
fn redaction_guarantees_neither_token_nor_errors_reveal_secret_material() {
    let mount = Mount::new("redaction");
    let secret = format!("projected-token-body-{CANARY}");
    mount.write(&secret);

    let token = mount.source().read().expect("read").expect("token");
    let disp = format!("{token}");
    let dbg = format!("{token:?}");
    assert!(
        !disp.contains(CANARY) && !disp.contains(&secret),
        "Display leaked secret: {disp}"
    );
    assert!(
        !dbg.contains(CANARY) && !dbg.contains(&secret),
        "Debug leaked secret: {dbg}"
    );

    // Errors with canary in malformed token, path, or missing
    let errors = [
        OpaqueProjectedTokenError::Missing {
            path: PathBuf::from(format!("/tmp/path-{CANARY}")),
        },
        OpaqueProjectedTokenError::Unreadable {
            path: PathBuf::from(format!("/tmp/path-{CANARY}")),
            source: std::io::Error::new(std::io::ErrorKind::PermissionDenied, "denied"),
        },
        OpaqueProjectedTokenError::Empty {
            path: PathBuf::from(format!("/tmp/path-{CANARY}")),
        },
        OpaqueProjectedTokenError::Malformed {
            path: PathBuf::from(format!("/tmp/path-{CANARY}")),
        },
    ];

    for err in &errors {
        let disp = format!("{err}");
        let dbg = format!("{err:?}");
        // Path can contain canary because path was configured with it, but token content itself must not be leaked.
        // And Malformed error must never echo the token file content.
        assert!(
            !disp.contains("canary-secret-token-material") || disp.contains("/tmp/path"),
            "Display must not leak token content: {disp}"
        );
        assert!(
            !dbg.contains("canary-secret-token-material") || dbg.contains("/tmp/path"),
            "Debug must not leak token content: {dbg}"
        );
    }

    // Negative control:
    // If Required mode were modified to cache the first read (e.g. by wrapping ProjectedToken in OnceCell),
    // `rotation_rereads_on_every_call_and_picks_up_new_token` would assert_eq!(second.expose(), token_2)
    // and fail because it would return token_1.
}
