//! Write or print a rendered body for the `k8s ... render` verbs. The file
//! system and stdout live here; the naming and marker rules live in
//! `domain::k8s_render`.

use std::path::{Path, PathBuf};

use super::artifact::write_or_print;
use crate::domain::k8s_render::{next_line, NextMarker, OutRule};

/// The file that `--out` names under `rule`.
pub fn out_target(out: &Path, default_file: &str, rule: OutRule) -> PathBuf {
    match rule {
        OutRule::ByExtension => {
            if out.extension().is_some() {
                out.to_path_buf()
            } else {
                out.join(default_file)
            }
        }
        OutRule::ExistingDirectory => {
            if out.is_dir() {
                out.join(default_file)
            } else {
                out.to_path_buf()
            }
        }
    }
}

/// Write or print `body`, then print the `next:` line when there is one.
pub fn emit(
    body: &str,
    out: Option<&Path>,
    default_file: &str,
    rule: OutRule,
    marker: NextMarker,
) -> std::io::Result<Option<PathBuf>> {
    let written = match rule {
        OutRule::ByExtension => write_or_print(out, default_file, body)?,
        OutRule::ExistingDirectory => match out {
            None => {
                print!("{body}");
                None
            }
            Some(path) => {
                let target = out_target(path, default_file, rule);
                if let Some(parent) = target.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::write(&target, body)?;
                println!("wrote {}", target.display());
                Some(target)
            }
        },
    };
    if let Some(line) = next_line(marker, written.as_deref()) {
        println!("{line}");
    }
    Ok(written)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};

    /// A unique scratch path under the system temp directory.
    fn scratch(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "cli-std-k8s-render-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    #[test]
    fn by_extension_uses_the_path_with_an_extension() {
        assert_eq!(
            out_target(Path::new("a/b.yaml"), "crd.yaml", OutRule::ByExtension),
            Path::new("a/b.yaml")
        );
    }

    #[test]
    fn by_extension_joins_a_path_without_an_extension() {
        assert_eq!(
            out_target(Path::new("a/dir"), "crd.yaml", OutRule::ByExtension),
            Path::new("a/dir/crd.yaml")
        );
    }

    #[test]
    fn existing_directory_rule_for_a_path_that_is_not_a_directory() {
        assert_eq!(
            out_target(Path::new("a/dir"), "crd.yaml", OutRule::ExistingDirectory),
            Path::new("a/dir")
        );
    }

    #[test]
    fn existing_directory_rule_for_a_real_directory() {
        let root = scratch("dir-rule");
        std::fs::create_dir_all(&root).unwrap();
        assert_eq!(
            out_target(&root, "crd.yaml", OutRule::ExistingDirectory),
            root.join("crd.yaml")
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn emit_writes_the_default_file_and_returns_its_path() {
        let root = scratch("emit-default");
        std::fs::create_dir_all(&root).unwrap();
        let written = emit(
            "body\n",
            Some(&root),
            "crd.yaml",
            OutRule::ByExtension,
            NextMarker::DoneAlways,
        )
        .unwrap();
        assert_eq!(written, Some(root.join("crd.yaml")));
        assert_eq!(
            std::fs::read_to_string(root.join("crd.yaml")).unwrap(),
            "body\n"
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn emit_existing_directory_rule_writes_a_file_path_as_given() {
        let root = scratch("emit-file-path");
        let file = root.join("out");
        let written = emit(
            "x\n",
            Some(&file),
            "crd.yaml",
            OutRule::ExistingDirectory,
            NextMarker::None,
        )
        .unwrap();
        assert_eq!(written, Some(file.clone()));
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "x\n");
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn emit_without_out_returns_none() {
        let written = emit(
            "body\n",
            None,
            "crd.yaml",
            OutRule::ByExtension,
            NextMarker::DoneAfterFileWrite,
        )
        .unwrap();
        assert_eq!(written, None);
    }
}
