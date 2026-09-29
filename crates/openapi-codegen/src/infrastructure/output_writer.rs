use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::domain::{GeneratedOutput, MANIFEST_FILE};

impl GeneratedOutput {
    /// Materialize generated files and, for explicit targets, their contract manifest.
    ///
    /// The output is intentionally written through this one method so every
    /// embedding CLI records the same contract rather than duplicating file
    /// loops and silently dropping target metadata.
    pub fn write_to_dir(&self, out_dir: &Path) -> Result<()> {
        let paths: Vec<PathBuf> = self
            .files
            .iter()
            .map(|file| safe_output_path(out_dir, &file.rel_path))
            .collect::<Result<_>>()?;
        std::fs::create_dir_all(out_dir)?;
        for (file, path) in self.files.iter().zip(paths) {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(path, &file.contents)?;
        }
        if let Some(manifest) = self.manifest() {
            let manifest = serde_json::to_string_pretty(&manifest)?;
            std::fs::write(out_dir.join(MANIFEST_FILE), format!("{manifest}\n"))?;
        }
        Ok(())
    }
}

fn safe_output_path(out_dir: &Path, rel_path: &str) -> Result<PathBuf> {
    use std::path::Component;

    let rel = Path::new(rel_path);
    if rel.is_absolute()
        || rel
            .components()
            .any(|component| matches!(component, Component::ParentDir | Component::RootDir))
    {
        anyhow::bail!("generated file path must stay under output directory: {rel_path:?}");
    }
    Ok(out_dir.join(rel))
}
