use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::source_revision::{explicit_source_revision, short_sha};

/// Emit `cargo:rustc-env=<PREFIX>_GIT_SHA=...`, `<PREFIX>_BUILT_AT=...`, and
/// `<PREFIX>_TARGET=...`, plus the `../../.git/HEAD` rerun-if-changed hint.
///
/// `prefix` is the caller's env-var prefix, e.g. `"LUMEN"` for
/// `LUMEN_GIT_SHA` / `LUMEN_BUILT_AT` / `LUMEN_TARGET`.
pub fn stamp(prefix: &str) {
    // Re-run when HEAD moves so the stamped sha stays current. The workspace
    // `.git` lives 2 levels up from the calling crate's build.rs (e.g.
    // `projects/<name>/`); in a linked worktree `.git` is a file rather than
    // a dir, so guard the rerun hint.
    if let Some(hint) = git_head_rerun_hint(Path::new("../../.git/HEAD")) {
        println!("{hint}");
    }

    let source_revision_variable = format!("{prefix}_SOURCE_REVISION");
    println!("cargo:rerun-if-env-changed={source_revision_variable}");
    let git_sha = explicit_source_revision(&source_revision_variable)
        .or_else(short_sha)
        .unwrap_or_else(|| "unknown".to_string());
    println!("cargo:rustc-env={prefix}_GIT_SHA={git_sha}");

    let built_at = built_at_now();
    println!("cargo:rustc-env={prefix}_BUILT_AT={built_at}");

    let target = target_from_env();
    println!("cargo:rustc-env={prefix}_TARGET={target}");
}

/// Cargo's `cargo:rerun-if-changed=<head_path>` directive, only when that
/// path actually exists.
fn git_head_rerun_hint(head_path: &Path) -> Option<String> {
    head_path
        .exists()
        .then(|| format!("cargo:rerun-if-changed={}", head_path.display()))
}

/// RFC3339-ish UTC timestamp without pulling in a date crate: seconds since
/// the epoch are unambiguous and trivially formattable.
fn built_at_now() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(format_built_at)
        .unwrap_or_else(|_| "unknown".to_string())
}

fn format_built_at(d: Duration) -> String {
    format!("{}", d.as_secs())
}

/// The exact target triple cargo built for (always set for build scripts),
/// falling back to `"unknown"` if absent.
fn target_from_env() -> String {
    decode_target(std::env::var("TARGET").ok())
}

fn decode_target(v: Option<String>) -> String {
    v.unwrap_or_else(|| "unknown".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_built_at_is_epoch_seconds() {
        assert_eq!(
            format_built_at(Duration::from_secs(1_700_000_000)),
            "1700000000"
        );
    }

    #[test]
    fn decode_target_falls_back_to_unknown_when_absent() {
        assert_eq!(decode_target(None), "unknown");
    }

    #[test]
    fn decode_target_passes_through_when_present() {
        assert_eq!(
            decode_target(Some("aarch64-apple-darwin".to_string())),
            "aarch64-apple-darwin"
        );
    }

    #[test]
    fn git_head_rerun_hint_none_when_path_missing() {
        let missing = Path::new("/nonexistent-path-for-build-stamp-tests/.git/HEAD");
        assert_eq!(git_head_rerun_hint(missing), None);
    }

    #[test]
    fn git_head_rerun_hint_some_when_path_present() {
        let dir = std::env::temp_dir().join(format!("build-stamp-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let head = dir.join("HEAD");
        std::fs::write(&head, b"ref: refs/heads/main\n").unwrap();

        let hint = git_head_rerun_hint(&head).unwrap();
        assert_eq!(hint, format!("cargo:rerun-if-changed={}", head.display()));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
