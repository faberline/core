use crate::domain::{TargetProfile, TargetRequirements};

/// A single generated file, relative to the output directory.
#[derive(Debug, Clone)]
pub struct GeneratedFile {
    pub rel_path: String,
    pub contents: String,
}

/// The full in-memory generation result (so tests can assert without I/O).
#[derive(Debug, Clone)]
pub struct GeneratedOutput {
    pub files: Vec<GeneratedFile>,
    /// The explicitly selected profile used to render `files`.
    pub target: Option<TargetProfile>,
    /// The minimum requirements for consuming explicitly targeted `files`.
    pub requirements: Option<TargetRequirements>,
}

/// Sidecar filename emitted with explicitly targeted generated clients.
pub const MANIFEST_FILE: &str = ".openapi-codegen.json";

/// Stable, user-visible record of the exact generated-client contract.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct GenerationManifest {
    pub schema_version: u8,
    pub generator: String,
    pub compiler: String,
    pub target: String,
    pub language: String,
    pub minimum_version: String,
    pub language_standard: String,
    pub module_system: Option<String>,
    pub module_resolution: Option<String>,
    pub strict: Option<bool>,
    pub transport: Option<String>,
    pub runtime_dependencies: Vec<String>,
}

impl GeneratedOutput {
    pub(crate) fn legacy(files: Vec<GeneratedFile>) -> Self {
        Self {
            files,
            target: None,
            requirements: None,
        }
    }

    pub(crate) fn for_target(files: Vec<GeneratedFile>, target: TargetProfile) -> Self {
        Self {
            files,
            target: Some(target),
            requirements: Some(target.requirements()),
        }
    }

    /// Build the sidecar manifest which makes the target contract inspectable
    /// after the in-memory result has been written to disk.
    pub fn manifest(&self) -> Option<GenerationManifest> {
        let requirements = self.requirements?;
        Some(GenerationManifest {
            schema_version: 1,
            generator: "openapi-codegen".to_string(),
            compiler: requirements.compiler.to_string(),
            target: requirements.target.to_string(),
            language: requirements.language.id().to_string(),
            minimum_version: requirements.minimum_version.to_string(),
            language_standard: requirements.language_standard.to_string(),
            module_system: requirements.module_system.map(str::to_string),
            module_resolution: requirements.module_resolution.map(str::to_string),
            strict: requirements.strict,
            transport: requirements.transport.map(str::to_string),
            runtime_dependencies: requirements
                .runtime_dependencies
                .iter()
                .map(|dependency| (*dependency).to_string())
                .collect(),
        })
    }
}

impl Default for GeneratedOutput {
    fn default() -> Self {
        Self::legacy(Vec::new())
    }
}
