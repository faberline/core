use std::path::Path;
use std::process::Command;

/// Read a caller-supplied immutable Git revision for archive and image builds.
/// Local builds keep the existing best-effort short-SHA fallback.
pub(crate) fn explicit_source_revision(variable: &str) -> Option<String> {
    decode_source_revision(std::env::var(variable).ok())
}

fn decode_source_revision(value: Option<String>) -> Option<String> {
    let value = value?;
    (value.len() == 40 && value.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .then(|| value.to_ascii_lowercase())
}

/// Best-effort short SHA of HEAD. Returns `None` outside a git workspace (or
/// when the `git` binary itself is absent).
pub(crate) fn short_sha() -> Option<String> {
    short_sha_in(Path::new("."))
}

fn short_sha_in(working_directory: &Path) -> Option<String> {
    let out = git_command_in(working_directory)
        .args(["rev-parse", "--short=8", "HEAD"])
        .output()
        .ok()?;
    decode_short_sha(out.status.success(), &out.stdout)
}

fn git_command_in(working_directory: &Path) -> Command {
    let mut command = Command::new("git");
    command.current_dir(working_directory);
    for variable in [
        "GIT_DIR",
        "GIT_WORK_TREE",
        "GIT_COMMON_DIR",
        "GIT_INDEX_FILE",
        "GIT_OBJECT_DIRECTORY",
        "GIT_ALTERNATE_OBJECT_DIRECTORIES",
        "GIT_NAMESPACE",
    ] {
        command.env_remove(variable);
    }
    command
}

/// Pure decode of a `git rev-parse` invocation's outcome, split out from
/// [`short_sha`] so the fallback path is unit-testable without depending on
/// the environment's git availability.
fn decode_short_sha(success: bool, stdout: &[u8]) -> Option<String> {
    if !success {
        return None;
    }
    let sha = String::from_utf8_lossy(stdout).trim().to_string();
    (!sha.is_empty()).then_some(sha)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_short_sha_none_on_failed_command() {
        assert_eq!(decode_short_sha(false, b"deadbeef\n"), None);
    }

    #[test]
    fn decode_short_sha_none_on_empty_stdout() {
        assert_eq!(decode_short_sha(true, b""), None);
    }

    #[test]
    fn decode_short_sha_trims_trailing_newline() {
        assert_eq!(
            decode_short_sha(true, b"c3ff13cd\n"),
            Some("c3ff13cd".to_string())
        );
    }

    #[test]
    fn explicit_source_revision_accepts_and_normalizes_a_full_git_sha() {
        assert_eq!(
            decode_source_revision(Some("0123456789ABCDEF0123456789ABCDEF01234567".into())),
            Some("0123456789abcdef0123456789abcdef01234567".into())
        );
    }

    #[test]
    fn explicit_source_revision_rejects_short_or_non_hex_values() {
        assert_eq!(decode_source_revision(Some("deadbeef".into())), None);
        assert_eq!(
            decode_source_revision(Some("0123456789abcdef0123456789abcdef0123456z".into())),
            None
        );
        assert_eq!(decode_source_revision(Some(String::new())), None);
    }

    #[test]
    fn short_sha_resolves_inside_an_isolated_git_workspace() {
        let repository = tempfile::tempdir().unwrap();
        let init = git_command_in(repository.path())
            .args(["init", "--quiet"])
            .status()
            .unwrap();
        assert!(init.success());

        let commit = git_command_in(repository.path())
            .args([
                "-c",
                "user.name=build-stamp-test",
                "-c",
                "user.email=build-stamp-test@example.invalid",
                "-c",
                "commit.gpgsign=false",
                "-c",
                "core.hooksPath=/dev/null",
                "commit",
                "--quiet",
                "--allow-empty",
                "--message=initial",
            ])
            .status()
            .unwrap();
        assert!(commit.success());

        let full_sha = git_command_in(repository.path())
            .args(["rev-parse", "HEAD"])
            .output()
            .unwrap();
        assert!(full_sha.status.success());
        let full_sha = String::from_utf8(full_sha.stdout).unwrap();
        let expected = &full_sha.trim()[..8];
        let sha = short_sha_in(repository.path()).unwrap();
        assert_eq!(sha, expected);
    }
}
