use anyhow::Result;

use crate::domain::emit;
use crate::domain::{FileBearerAuth, GenOptions, GeneratedOutput, Lang, TargetProfile};

/// Pure core: spec JSON text → generated files. No filesystem access. Dispatches
/// to the per-language emitter selected by [`GenOptions::lang`].
pub fn generate(spec_json: &str, opts: &GenOptions) -> Result<GeneratedOutput> {
    match opts.target() {
        Some(target) => generate_for_target(spec_json, opts, target),
        None => match opts.lang() {
            Lang::Ts => Ok(emit::ts::generate(spec_json, opts)?),
            Lang::Py => Ok(emit::py::generate(spec_json, opts)?),
            Lang::Rust => Ok(emit::rust::generate(spec_json, opts)?),
        },
    }
}

/// Generate a client that rereads one file-backed bearer token before each
/// eligible request. Calling this separate entry point is the opt-in; the
/// legacy [`generate`] output remains byte-for-byte unchanged.
pub fn generate_with_file_bearer_auth(
    spec_json: &str,
    opts: &GenOptions,
    auth: &FileBearerAuth,
) -> Result<GeneratedOutput> {
    match opts.target() {
        Some(target) => generate_for_target_with_file_bearer_auth(spec_json, opts, target, auth),
        None => match opts.lang() {
            Lang::Ts => Ok(emit::ts::generate_with_file_bearer_auth(
                spec_json, opts, auth,
            )?),
            Lang::Py => Ok(emit::py::generate_with_file_bearer_auth(
                spec_json, opts, auth,
            )?),
            Lang::Rust => Ok(emit::rust::generate_with_file_bearer_auth(
                spec_json, opts, auth,
            )?),
        },
    }
}

/// Pure core with an explicit versioned target profile. The profile must match
/// [`GenOptions::lang`], so an invalid cross-language request fails before any
/// language-specific parsing or generation occurs.
pub fn generate_for_target(
    spec_json: &str,
    opts: &GenOptions,
    target: TargetProfile,
) -> Result<GeneratedOutput> {
    validate_target(opts, target)?;
    match target {
        TargetProfile::TypeScript(target) => {
            Ok(emit::ts::generate_for_target(spec_json, opts, target)?)
        }
        TargetProfile::Python(target) => {
            Ok(emit::py::generate_for_target(spec_json, opts, target)?)
        }
        TargetProfile::Rust(target) => {
            Ok(emit::rust::generate_for_target(spec_json, opts, target)?)
        }
    }
}

fn validate_target(opts: &GenOptions, target: TargetProfile) -> Result<()> {
    if opts.lang() != target.lang() {
        anyhow::bail!(
            "target profile {} is for {:?}, not requested language {:?}",
            target.id(),
            target.lang(),
            opts.lang()
        );
    }
    if let Some(configured) = opts.target() {
        if configured != target {
            anyhow::bail!(
                "explicit target argument {} conflicts with GenOptions target {}",
                target.id(),
                configured.id()
            );
        }
    }
    Ok(())
}

/// Targeted variant of [`generate_with_file_bearer_auth`]. It preserves the
/// normal target validation and generation manifest.
pub fn generate_for_target_with_file_bearer_auth(
    spec_json: &str,
    opts: &GenOptions,
    target: TargetProfile,
    auth: &FileBearerAuth,
) -> Result<GeneratedOutput> {
    validate_target(opts, target)?;
    match target {
        TargetProfile::TypeScript(target) => Ok(
            emit::ts::generate_for_target_with_file_bearer_auth(spec_json, opts, target, auth)?,
        ),
        TargetProfile::Python(target) => Ok(emit::py::generate_for_target_with_file_bearer_auth(
            spec_json, opts, target, auth,
        )?),
        TargetProfile::Rust(target) => Ok(emit::rust::generate_for_target_with_file_bearer_auth(
            spec_json, opts, target, auth,
        )?),
    }
}
