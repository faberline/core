use super::*;
use jsonwebtoken::{encode, EncodingKey, Header};
use serde::Serialize;
use std::time::{SystemTime, UNIX_EPOCH};

/// A recognisable string, so a test can assert it appears nowhere in an
/// error rendering rather than asserting on the shape of the message.
const CANARY: &str = "canary-service-account-token-must-never-be-printed";

#[derive(Serialize)]
struct Claims {
    aud: Vec<String>,
    exp: i64,
    sub: String,
    /// Carries the canary into the token's own payload, so a test proves
    /// the *material* is withheld and not merely that the wrapper's
    /// `Display` was called.
    jti: String,
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock after the epoch")
        .as_secs() as i64
}

/// A syntactically real JWT. Signed with a throwaway HMAC key — the reader
/// under test never checks the signature, and building one this way keeps
/// the fixture honest about JWT framing.
fn token_for(audience: &str, exp: i64) -> String {
    encode(
        &Header::new(Algorithm::HS256),
        &Claims {
            aud: vec![audience.to_string()],
            exp,
            sub: "system:serviceaccount:ops:caller".to_string(),
            jti: CANARY.to_string(),
        },
        &EncodingKey::from_secret(b"irrelevant"),
    )
    .expect("encode fixture token")
}

struct Mount {
    dir: PathBuf,
}

impl Mount {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "service-auth-projected-{tag}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create fixture mount");
        Self { dir }
    }

    fn path(&self) -> PathBuf {
        self.dir.join("token")
    }

    fn write(&self, contents: &str) {
        std::fs::write(self.path(), contents).expect("write fixture token");
    }

    fn file(&self, audience: &str) -> ProjectedTokenFile {
        ProjectedTokenFile::new(self.path(), audience)
    }
}

impl Drop for Mount {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

#[test]
fn a_token_with_the_expected_audience_reads_back_verbatim() {
    let mount = Mount::new("happy");
    let token = token_for("callee.example.com", now() + 600);
    mount.write(&token);

    let read = mount
        .file("callee.example.com")
        .read()
        .expect("a current, correctly-scoped token is presentable");
    assert_eq!(read.expose(), token);
}

/// The rotation contract, and the reason nothing is cached: the kubelet
/// replaces the file underneath a running process, and the next request
/// must go out with the replacement.
#[test]
fn a_rotated_file_is_picked_up_by_the_next_read_without_a_restart() {
    let mount = Mount::new("rotation");
    let first = token_for("callee.example.com", now() + 600);
    mount.write(&first);
    let file = mount.file("callee.example.com");

    assert_eq!(file.read().expect("first read").expose(), first);

    // Same process, same `ProjectedTokenFile`, new material — exactly what
    // the kubelet does at ~80% of the lifetime.
    let second = token_for("callee.example.com", now() + 1200);
    assert_ne!(first, second, "the fixture must actually rotate");
    mount.write(&second);

    assert_eq!(
        file.read().expect("second read").expose(),
        second,
        "a cached token would have kept presenting the old one until it expired"
    );
}

#[test]
fn a_trailing_newline_is_not_part_of_the_credential() {
    let mount = Mount::new("newline");
    let token = token_for("callee.example.com", now() + 600);
    mount.write(&format!("{token}\n"));

    assert_eq!(
        mount
            .file("callee.example.com")
            .read()
            .expect("read")
            .expose(),
        token
    );
}

#[test]
fn a_missing_mount_is_an_actionable_error_naming_the_path() {
    let file = ProjectedTokenFile::new("/nonexistent/projected/token", "callee.example.com");
    let err = file.read().expect_err("a missing file cannot be presented");
    assert!(
        matches!(err, ProjectedTokenError::Unreadable { .. }),
        "{err:?}"
    );
    let rendered = err.to_string();
    assert!(
        rendered.contains("/nonexistent/projected/token"),
        "{rendered}"
    );
    assert!(rendered.contains("projected volume"), "{rendered}");
}

#[test]
fn an_empty_file_is_refused_rather_than_sent_as_an_empty_bearer() {
    let mount = Mount::new("empty");
    mount.write("   \n");
    let err = mount.file("callee.example.com").read().expect_err("empty");
    assert!(matches!(err, ProjectedTokenError::Empty { .. }), "{err:?}");
}

/// The case worth catching locally: the pod's *default* token. It is a
/// perfectly valid credential minted for kube-apiserver, so the callee
/// answers `401` and the operator goes looking at RBAC.
#[test]
fn a_default_pod_token_is_refused_here_rather_than_at_the_callee() {
    let mount = Mount::new("audience");
    mount.write(&token_for(
        "https://kubernetes.default.svc.cluster.local",
        now() + 3600,
    ));

    let err = mount
        .file("callee.example.com")
        .read()
        .expect_err("wrong audience");
    assert!(
        matches!(err, ProjectedTokenError::WrongAudience { .. }),
        "{err:?}"
    );
    let rendered = err.to_string();
    assert!(rendered.contains("callee.example.com"), "{rendered}");
    assert!(rendered.contains("serviceAccountToken"), "{rendered}");
}

#[test]
fn an_expired_token_is_refused_and_the_message_points_at_rotation() {
    let mount = Mount::new("expired");
    mount.write(&token_for("callee.example.com", now() - 3600));

    let err = mount
        .file("callee.example.com")
        .read()
        .expect_err("expired");
    assert!(
        matches!(err, ProjectedTokenError::Expired { .. }),
        "{err:?}"
    );
    assert!(err.to_string().contains("rotating"), "{err}");
}

#[test]
fn a_file_that_is_not_a_token_at_all_is_malformed() {
    let mount = Mount::new("garbage");
    mount.write("not-a-jwt");
    let err = mount
        .file("callee.example.com")
        .read()
        .expect_err("garbage");
    assert!(
        matches!(err, ProjectedTokenError::Malformed { .. }),
        "{err:?}"
    );
}

/// AC4's redaction requirement, asserted against the material rather than
/// against the wording: no rendering of a token or of any failure to read
/// one may contain the bytes that were on disk.
#[test]
fn no_rendering_of_a_token_or_its_failures_contains_the_material() {
    let mount = Mount::new("redaction");

    let current = token_for("callee.example.com", now() + 600);
    mount.write(&current);
    let token = mount
        .file("callee.example.com")
        .read()
        .expect("read the good token");
    for rendered in [format!("{token}"), format!("{token:?}")] {
        assert!(
            !rendered.contains(CANARY) && !rendered.contains(&current),
            "a token printed itself: {rendered}"
        );
    }

    // Every failure path, including the ones whose input is a valid token.
    let cases = [
        token_for("someone.else.example.com", now() + 600),
        token_for("callee.example.com", now() - 600),
        format!("not-a-jwt-{CANARY}"),
    ];
    for material in cases {
        mount.write(&material);
        let err = mount
            .file("callee.example.com")
            .read()
            .expect_err("each case is a refusal");
        for rendered in [err.to_string(), format!("{err:?}")] {
            assert!(
                !rendered.contains(CANARY) && !rendered.contains(&material),
                "an error echoed the credential it refused: {rendered}"
            );
        }
    }
}
