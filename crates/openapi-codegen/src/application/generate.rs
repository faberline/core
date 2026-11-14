use std::borrow::Cow;

use anyhow::Result;
use sha2::{Digest, Sha256};

use crate::domain::emit;
use crate::domain::ir::select::select_operations;
use crate::domain::{
    FileBearerAuth, GenOptions, GeneratedOutput, GenerationAuth, Lang, ManagedAuth, TargetProfile,
};

fn validate_options(opts: &GenOptions) -> Result<()> {
    opts.validate()?;
    Ok(())
}

/// The spec text the emitters read. Without a selection it is the input, byte
/// for byte; with one it is the spec filtered to the selected operations.
fn prepared<'a>(spec_json: &'a str, opts: &GenOptions) -> Result<Cow<'a, str>> {
    match opts.operations() {
        None => Ok(Cow::Borrowed(spec_json)),
        Some(selection) => Ok(Cow::Owned(select_operations(spec_json, selection)?)),
    }
}

/// Records the provenance of the ORIGINAL spec text, never of the filtered one.
fn attach_provenance(output: &mut GeneratedOutput, spec_json: &str, opts: &GenOptions) {
    let hash = format!("{:x}", Sha256::digest(spec_json.as_bytes()));
    output.openapi_sha256 = Some(hash);
    output.service_compatibility = opts.service_compatibility().map(str::to_string);
    output.operations = opts.operations().map(|s| s.include().to_vec());
}

/// Pure core: spec JSON text → generated files. No filesystem access. Dispatches
/// to the per-language emitter selected by [`GenOptions::lang`].
pub fn generate(spec_json: &str, opts: &GenOptions) -> Result<GeneratedOutput> {
    validate_options(opts)?;
    let mut output = match opts.target() {
        Some(target) => generate_for_target(spec_json, opts, target)?,
        None => match opts.lang() {
            Lang::Ts => emit::ts::generate(&prepared(spec_json, opts)?, opts)?,
            Lang::Py => emit::py::generate(&prepared(spec_json, opts)?, opts)?,
            Lang::Rust => emit::rust::generate(&prepared(spec_json, opts)?, opts)?,
        },
    };
    attach_provenance(&mut output, spec_json, opts);
    Ok(output)
}

/// Generate a client that rereads one file-backed bearer token before each
/// eligible request. Calling this separate entry point is the opt-in; the
/// legacy [`generate`] output remains byte-for-byte unchanged.
pub fn generate_with_file_bearer_auth(
    spec_json: &str,
    opts: &GenOptions,
    auth: &FileBearerAuth,
) -> Result<GeneratedOutput> {
    validate_options(opts)?;
    let mut output = match opts.target() {
        Some(target) => generate_for_target_with_file_bearer_auth(spec_json, opts, target, auth)?,
        None => match opts.lang() {
            Lang::Ts => {
                emit::ts::generate_with_file_bearer_auth(&prepared(spec_json, opts)?, opts, auth)?
            }
            Lang::Py => {
                emit::py::generate_with_file_bearer_auth(&prepared(spec_json, opts)?, opts, auth)?
            }
            Lang::Rust => {
                emit::rust::generate_with_file_bearer_auth(&prepared(spec_json, opts)?, opts, auth)?
            }
        },
    };
    attach_provenance(&mut output, spec_json, opts);
    Ok(output)
}

fn generate_with_managed_auth(
    spec_json: &str,
    opts: &GenOptions,
    auth: &ManagedAuth,
) -> Result<GeneratedOutput> {
    match opts.lang() {
        Lang::Ts => {
            validate_options(opts)?;
            let spec = prepared(spec_json, opts)?;
            let mut output = match opts.target() {
                Some(TargetProfile::TypeScript(target)) => {
                    emit::ts::generate_for_target_with_managed_auth(&spec, opts, target, auth)?
                }
                _ => emit::ts::generate_with_managed_auth(&spec, opts, auth)?,
            };
            attach_provenance(&mut output, spec_json, opts);
            Ok(output)
        }
        Lang::Py => {
            validate_options(opts)?;
            let spec = prepared(spec_json, opts)?;
            let mut output = match opts.target() {
                Some(TargetProfile::Python(target)) => {
                    emit::py::generate_for_target_with_managed_auth(&spec, opts, target, auth)?
                }
                _ => emit::py::generate_with_managed_auth(&spec, opts, auth)?,
            };
            attach_provenance(&mut output, spec_json, opts);
            Ok(output)
        }
        Lang::Rust => {
            validate_options(opts)?;
            let spec = prepared(spec_json, opts)?;
            let mut output = match opts.target() {
                Some(TargetProfile::Rust(target)) => {
                    emit::rust::generate_for_target_with_managed_auth(&spec, opts, target, auth)?
                }
                _ => emit::rust::generate_with_managed_auth(&spec, opts, auth)?,
            };
            attach_provenance(&mut output, spec_json, opts);
            Ok(output)
        }
    }
}

/// Generate with an explicit auth selection. A file-bearer selection generates
/// exactly like [`generate_with_file_bearer_auth`]. A Managed selection changes
/// the runtime of every language: TypeScript, Python and Rust.
pub fn generate_with_auth(
    spec_json: &str,
    opts: &GenOptions,
    auth: &GenerationAuth,
) -> Result<GeneratedOutput> {
    match (auth.file_bearer(), auth.managed()) {
        (Some(file), _) => generate_with_file_bearer_auth(spec_json, opts, file),
        (None, Some(managed)) => generate_with_managed_auth(spec_json, opts, managed),
        (None, None) => generate(spec_json, opts),
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
    validate_options(opts)?;
    validate_target(opts, target)?;
    let spec = prepared(spec_json, opts)?;
    let mut output = match target {
        TargetProfile::TypeScript(target) => emit::ts::generate_for_target(&spec, opts, target)?,
        TargetProfile::Python(target) => emit::py::generate_for_target(&spec, opts, target)?,
        TargetProfile::Rust(target) => emit::rust::generate_for_target(&spec, opts, target)?,
    };
    attach_provenance(&mut output, spec_json, opts);
    Ok(output)
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
    validate_options(opts)?;
    validate_target(opts, target)?;
    let spec = prepared(spec_json, opts)?;
    let mut output = match target {
        TargetProfile::TypeScript(target) => {
            emit::ts::generate_for_target_with_file_bearer_auth(&spec, opts, target, auth)?
        }
        TargetProfile::Python(target) => {
            emit::py::generate_for_target_with_file_bearer_auth(&spec, opts, target, auth)?
        }
        TargetProfile::Rust(target) => {
            emit::rust::generate_for_target_with_file_bearer_auth(&spec, opts, target, auth)?
        }
    };
    attach_provenance(&mut output, spec_json, opts);
    Ok(output)
}
