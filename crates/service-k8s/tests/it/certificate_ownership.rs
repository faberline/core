//! Where certificate code is allowed to live (#3110 AC6).
//!
//! R1 says the generic lifecycle belongs in shared libraries and that Lumen
//! supplies profiles. That is an architectural claim, and architectural claims
//! decay silently: the first copy of an issuance loop into `apps/lumen` compiles,
//! passes every behavioural test, and is only visible to someone who happens to
//! read the right file. So it is checked here, mechanically, against the source
//! tree itself.
//!
//! The check is deliberately a *type* allowlist rather than a keyword denylist.
//! A denylist of "no `rcgen`, no CSR" is a guess about which name the next
//! duplicate will be spelled with; an allowlist of what Lumen's certificate
//! module may contain refuses the one nobody predicted.
//!
//! Lumen's half of this check (its profile module, its sources and its
//! Terraform) lives in faberline/lumen's `certificate_ownership` integration test.

#![cfg(feature = "certificate")]

use std::path::{Path, PathBuf};

/// The core workspace root: service-k8s sits at `crates/service-k8s`.
fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("service-k8s sits at crates/service-k8s in the core workspace")
        .to_path_buf()
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|err| panic!("read {}: {err}", path.display()))
}

/// Every `.rs` file under `dir`.
fn rust_files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(current) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&current) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                out.push(path);
            }
        }
    }
    out.sort();
    out
}

#[test]
fn the_generic_lifecycle_lives_in_the_shared_library() {
    let shared = repo_root().join("crates/service-k8s/src/certificate");
    let present: Vec<String> = rust_files(&shared)
        .iter()
        .filter_map(|path| {
            path.file_stem()
                .and_then(|stem| stem.to_str())
                .map(str::to_string)
        })
        .collect();

    // Every piece R1 names as generic, by the module that owns it.
    for expected in [
        "state",       // renewal, expiry, rotation ordering
        "issuer",      // the CSR/keypair boundary
        "cas",         // the CA Service requester
        "ephemeral",   // the replaceable in-process signer (R8)
        "projection",  // Secret layout and owner references
        "reconcile",   // retries and the write path
        "status",      // conditions and redaction
        "profile",     // bounds and validation
    ] {
        assert!(
            present.iter().any(|name| name == expected),
            "crates/service-k8s/src/certificate has no `{expected}` module; if it moved into a \
             service, this library is no longer the shared lifecycle: {present:?}"
        );
    }
}

#[test]
fn the_shared_lifecycle_does_not_know_what_lumen_is() {
    // The direction of the dependency is the property. Shared code that special-
    // cases one service is shared in name only, and the next service to adopt it
    // inherits Lumen's assumptions without being told.
    let shared = repo_root().join("crates/service-k8s/src/certificate");
    let mut offenders = Vec::new();
    for path in rust_files(&shared) {
        let source = read(&path);
        for line in source.lines() {
            let trimmed = line.trim_start();
            // Fixtures name a concrete service because a scope with no instance
            // name is not a scope. The claim is about the code that ships.
            if trimmed.starts_with("#[cfg(test)]") || trimmed.starts_with("mod tests") {
                break;
            }
            // Comments and doc comments may name Lumen: the issues that drove
            // this design are Lumen's, and erasing that provenance would make
            // the code harder to change correctly, not more generic.
            if trimmed.starts_with("//") {
                continue;
            }
            if line.to_lowercase().contains("lumen") {
                offenders.push(format!("{}: {}", path.display(), line.trim()));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "shared certificate code branches on Lumen: {offenders:#?}"
    );
}
