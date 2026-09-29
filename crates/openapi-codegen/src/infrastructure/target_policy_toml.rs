use anyhow::{bail, Context, Result};
use serde::Deserialize;

use crate::domain::{Lang, TargetPolicy, TargetProfile};

impl TargetPolicy {
    /// Parse a project `codegen.toml` target policy.
    ///
    /// ```toml
    /// [targets]
    /// typescript = "typescript-5.0"
    /// python = "python-3.14"
    /// rust = "rust-2024"
    /// ```
    pub fn from_toml(source: &str) -> Result<Self> {
        let raw: RawTargetPolicy = toml::from_str(source).context("parse codegen target policy")?;
        Ok(Self {
            typescript: parse_policy_target(
                raw.targets.typescript,
                Lang::Ts,
                "targets.typescript",
            )?,
            python: parse_policy_target(raw.targets.python, Lang::Py, "targets.python")?,
            rust: parse_policy_target(raw.targets.rust, Lang::Rust, "targets.rust")?,
        })
    }
}

#[derive(Deserialize)]
struct RawTargetPolicy {
    targets: RawTargets,
}

#[derive(Deserialize)]
struct RawTargets {
    typescript: String,
    python: String,
    rust: String,
}

fn parse_policy_target(value: String, lang: Lang, key: &str) -> Result<TargetProfile> {
    let target = TargetProfile::from_id(&value).with_context(|| format!("parse {key}"))?;
    if target.lang() != lang {
        bail!("{key} must select {:?}, got {}", lang, target.id());
    }
    Ok(target)
}
