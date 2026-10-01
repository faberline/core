//! Golden pins for the text of every error the domain returns, taken before
//! the DDD P2 changes: the Display and the `{:#}` chain a CLI prints.

use super::*;

fn chain(err: impl Into<anyhow::Error>) -> (String, String) {
    let err = err.into();
    (err.to_string(), format!("{err:#}"))
}

fn pair(display: &str, alternate: &str) -> (String, String) {
    (display.to_string(), alternate.to_string())
}

const PARSE_FAILED: &str = "failed to parse OpenAPI spec";
const PARSE_FAILED_CHAIN: &str =
    "failed to parse OpenAPI spec: key must be a string at line 1 column 3";
const EXPECTED_PROFILES: &str = "expected one of: python-3.11, python-3.12, python-3.13, \
                                 python-3.14, typescript-5.0, rust-2021, rust-2024";

#[test]
fn emitter_spec_parse_error_is_pinned() {
    let opts = full_opts();
    let expected = pair(PARSE_FAILED, PARSE_FAILED_CHAIN);
    assert_eq!(
        chain(emit::ts::generate("{ not json", &opts).unwrap_err()),
        expected
    );
    assert_eq!(
        chain(emit::py::generate("{ not json", &opts).unwrap_err()),
        expected
    );
    assert_eq!(
        chain(emit::rust::generate("{ not json", &opts).unwrap_err()),
        expected
    );
    assert_eq!(chain(generate("{ not json", &opts).unwrap_err()), expected);
}

#[test]
fn file_bearer_auth_errors_are_pinned() {
    let https = [FileBearerScheme::Https];
    for (path, suffix, schemes, message) in [
        (
            "",
            ".example.internal",
            &https[..],
            "bearer token path must not be empty",
        ),
        (
            "/run/token",
            "example.internal",
            &https[..],
            "bearer hostname suffix must start with one dot",
        ),
        (
            "/run/token",
            ".",
            &https[..],
            "bearer hostname suffix must start with one dot",
        ),
        (
            "/run/token",
            ".example.internal.",
            &https[..],
            "bearer hostname suffix must start with one dot",
        ),
        (
            "/run/token",
            ".Example.internal",
            &https[..],
            "bearer hostname suffix must be lowercase DNS labels",
        ),
        (
            "/run/token",
            ".example.internal",
            &[][..],
            "bearer auth needs at least one HTTP scheme",
        ),
    ] {
        let error = FileBearerAuth::new(path, suffix, schemes.iter().copied()).unwrap_err();
        assert_eq!(chain(error), pair(message, message), "{path:?} {suffix:?}");
    }
}

#[cfg(unix)]
#[test]
fn file_bearer_auth_non_utf8_path_error_is_pinned() {
    use std::os::unix::ffi::OsStrExt;
    let path = PathBuf::from(std::ffi::OsStr::from_bytes(b"/run/\xff"));
    let error =
        FileBearerAuth::new(path, ".example.internal", [FileBearerScheme::Https]).unwrap_err();
    let message = "bearer token path must be valid UTF-8";
    assert_eq!(chain(error), pair(message, message));
}

#[test]
fn unknown_target_profile_error_is_pinned() {
    let message = format!("unknown target profile \"python-2.7\"; {EXPECTED_PROFILES}");
    assert_eq!(
        chain(TargetProfile::from_id("python-2.7").unwrap_err()),
        pair(&message, &message)
    );
    assert_eq!(
        chain("python-2.7".parse::<TargetProfile>().unwrap_err()),
        pair(&message, &message)
    );
}

#[test]
fn target_policy_errors_are_pinned() {
    let policy = TargetPolicy::from_toml(
        "[targets]\ntypescript = \"typescript-5.0\"\npython = \"python-3.14\"\nrust = \"rust-2024\"\n",
    )
    .unwrap();
    assert_eq!(
        chain(policy.resolve(Lang::Ts, Some("python-9")).unwrap_err()),
        pair(
            "parse explicit target profile \"python-9\"",
            &format!(
                "parse explicit target profile \"python-9\": unknown target profile \
                 \"python-9\"; {EXPECTED_PROFILES}"
            ),
        )
    );
    let message = "target profile python-3.12 is for Py, not requested language Ts";
    assert_eq!(
        chain(policy.resolve(Lang::Ts, Some("python-3.12")).unwrap_err()),
        pair(message, message)
    );

    let unknown = TargetPolicy::from_toml(
        "[targets]\ntypescript = \"typescript-5.0\"\npython = \"python-9\"\nrust = \"rust-2024\"\n",
    )
    .unwrap_err();
    assert_eq!(
        chain(unknown),
        pair(
            "parse targets.python",
            &format!(
                "parse targets.python: unknown target profile \"python-9\"; {EXPECTED_PROFILES}"
            ),
        )
    );
    let mismatch = TargetPolicy::from_toml(
        "[targets]\ntypescript = \"typescript-5.0\"\npython = \"typescript-5.0\"\nrust = \"rust-2024\"\n",
    )
    .unwrap_err();
    let message = "targets.python must select Py, got typescript-5.0";
    assert_eq!(chain(mismatch), pair(message, message));
}
